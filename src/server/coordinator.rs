//! Reports watched sessions' status changes to the coordinator as an automated turn, so it
//! can keep work moving and escalate decisions without the user asking first.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast::error::RecvError;

use super::push::StatusChange;
use super::session_service::SessionCaller;
use super::state::AppState;
use crate::server::api::{coordinator_session, submit_prompt, PromptSubmission};
use crate::session::Status;

/// Changes arriving this soon after the first one go out in the same turn.
const BATCH: Duration = Duration::from_secs(5);

/// Marks the turn as AoE's, not the user's.
const ORIGIN: &str = "aoe.coordinator";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Update {
    Waiting,
    Finished,
    Failed,
}

fn update_for(old: Status, new: Status) -> Option<Update> {
    match (old, new) {
        (old, Status::Waiting) if old != Status::Waiting => Some(Update::Waiting),
        (Status::Running, Status::Idle) => Some(Update::Finished),
        (old, Status::Error) if old != Status::Error => Some(Update::Failed),
        _ => None,
    }
}

struct Pending {
    id: String,
    title: String,
    update: Update,
}

/// One line per session, keeping only its latest update.
fn message(pending: &[Pending]) -> String {
    let mut latest: Vec<&Pending> = Vec::new();
    for p in pending {
        match latest.iter_mut().find(|seen| seen.id == p.id) {
            Some(seen) => *seen = p,
            None => latest.push(p),
        }
    }
    let mut text = String::from("[aoe update] Watched sessions changed:\n");
    for p in latest {
        let what = match p.update {
            Update::Waiting => "is waiting for input",
            Update::Finished => "finished its turn",
            Update::Failed => "failed",
        };
        text.push_str(&format!("- \"{}\" (`{}`) {what}\n", p.title, p.id));
    }
    text.push_str("Check their output and act on them as your workflow says.");
    text
}

async fn pending_for(state: &AppState, change: StatusChange) -> Option<Pending> {
    let update = update_for(change.old, change.new)?;
    if coordinator_session(state).await.as_deref() == Some(change.instance_id.as_str()) {
        return None;
    }
    let id = change.instance_id.clone();
    let watched =
        tokio::task::spawn_blocking(move || crate::session::coordinator::watchlist().watches(&id))
            .await
            .unwrap_or(false);
    watched.then_some(Pending {
        id: change.instance_id,
        title: change.instance_title,
        update,
    })
}

async fn deliver(state: &Arc<AppState>, pending: &[Pending]) {
    if let Err(reason) = tell_coordinator(state, &message(pending)).await {
        tracing::warn!(target: "server.coordinator", reason, "status update not delivered");
    }
}

/// Sends the coordinator session an AoE-marked turn, returning its id, or why it could not.
pub(crate) async fn tell_coordinator(
    state: &Arc<AppState>,
    text: &str,
) -> Result<String, &'static str> {
    let coordinator = coordinator_session(state).await.ok_or("no_coordinator")?;
    let submission = PromptSubmission {
        text,
        uploads: &[],
        prompt_id: None,
        queued_prompt_id: None,
        no_revive: false,
    };
    let caller = SessionCaller::PluginMessage {
        plugin_id: ORIGIN.into(),
    };
    use crate::server::api::PromptSubmitError as E;
    match submit_prompt(state, &caller, &coordinator, submission).await {
        Ok(_) => Ok(coordinator),
        Err(E::SessionNotFound) => Err("no_coordinator"),
        Err(E::RevivalRefused) => Err("revival_refused"),
        Err(E::WorkerNotReady) => Err("worker_not_ready"),
        Err(E::Blocked(_)) => Err("blocked"),
        Err(E::Enqueue(..)) => Err("queue_refused"),
        Err(E::Turn(_)) => Err("turn_failed"),
        Err(_) => Err("refused"),
    }
}

pub(super) fn spawn(state: Arc<AppState>) {
    crate::task_util::spawn_supervised(
        "server.coordinator",
        crate::task_util::PanicPolicy::Log,
        async move {
            let mut rx = state.status_tx.subscribe();
            loop {
                let first = loop {
                    match rx.recv().await {
                        Ok(change) => {
                            if let Some(p) = pending_for(&state, change).await {
                                break p;
                            }
                        }
                        Err(RecvError::Lagged(_)) => continue,
                        Err(RecvError::Closed) => return,
                    }
                };
                let mut pending = vec![first];
                let deadline = tokio::time::Instant::now() + BATCH;
                loop {
                    match tokio::time::timeout_at(deadline, rx.recv()).await {
                        Ok(Ok(change)) => pending.extend(pending_for(&state, change).await),
                        Ok(Err(RecvError::Lagged(_))) => {}
                        Ok(Err(RecvError::Closed)) => return,
                        Err(_) => break,
                    }
                }
                deliver(&state, &pending).await;
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_waiting_finished_and_failed_turns_only() {
        use Status::*;
        for (old, new, expected) in [
            (Running, Waiting, Some(Update::Waiting)),
            (Idle, Waiting, Some(Update::Waiting)),
            (Waiting, Waiting, None),
            (Running, Idle, Some(Update::Finished)),
            (Starting, Idle, None),
            (Waiting, Running, None),
            (Running, Error, Some(Update::Failed)),
            (Running, Stopped, None),
        ] {
            assert_eq!(update_for(old, new), expected, "{old:?} -> {new:?}");
        }
    }

    #[test]
    fn message_keeps_each_sessions_latest_update() {
        let p = |id: &str, update| Pending {
            id: id.into(),
            title: format!("t-{id}"),
            update,
        };
        let text = message(&[
            p("a", Update::Finished),
            p("b", Update::Waiting),
            p("a", Update::Waiting),
        ]);
        assert!(text.starts_with("[aoe update]"));
        assert!(text.contains("\"t-a\" (`a`) is waiting for input"));
        assert!(!text.contains("finished its turn"));
        assert_eq!(text.matches("(`").count(), 2);
    }
}
