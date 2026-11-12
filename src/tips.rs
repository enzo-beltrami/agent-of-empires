//! Tips catalog for the web dashboard.

/// Called only after a creation is committed, never for a restart or a placeholder.
pub(crate) fn record_session_creations(count: usize) {
    if count == 0 {
        return;
    }
    if let Err(error) = crate::session::config::update_app_state(|state| {
        state.sessions_created = state.sessions_created.saturating_add(count as u64);
    }) {
        tracing::warn!(%error, "Failed to persist session creation count for tips");
    }
}

pub struct Tip {
    /// Persistence key in `tips_seen`: never reuse or renumber.
    pub id: &'static str,
    pub title: &'static str,
    pub body: &'static str,
}

pub fn catalog() -> &'static [Tip] {
    CATALOG
}

static CATALOG: &[Tip] = &[
    Tip {
        id: "install-dashboard-pwa",
        title: "Install the dashboard as an app",
        body: "You can install the dashboard as an app for quick access. In your browser, \
               use the install option (Install Agent of Empires in Chrome, or Add to Home \
               Screen on iOS) to keep it one tap away and keep notifications working.",
    },
    Tip {
        id: "pin-sessions",
        title: "Keep important sessions on top",
        body: "Right-click a session in the sidebar (long-press on touch) and choose Pin to \
               float it to the top of every sort. Unpin it the same way.",
    },
    Tip {
        id: "archive-sessions",
        title: "Tuck finished sessions away",
        body: "Right-click a session and choose Archive to stop it and tuck it into the \
               \"Snoozed & archived\" footer at the bottom of the sidebar. Sending it a \
               message brings it right back.",
    },
    Tip {
        id: "snooze-sessions",
        title: "Snooze a session for later",
        body: "Right-click a session, choose Snooze, and pick a duration from 1 hour up to \
               1 week. It stays hidden until the timer runs out or you send it a message.",
    },
    Tip {
        id: "group-sessions",
        title: "Organize sessions into groups",
        body: "The grouping picker offers Repository, Organization, User group, Repository \
               then group, and Session status. Right-click a session and choose Edit group to \
               file it under any name you like.",
    },
    Tip {
        id: "sort-sidebar",
        title: "Sort the sidebar your way",
        body: "The sort picker offers Manual, where you drag the rows into any order \
               yourself, Recent activity, and Attention, which floats the sessions that need \
               you to the top.",
    },
    Tip {
        id: "scratch-sessions",
        title: "Spin up a scratch session",
        body: "Need a throwaway? Toggle \"Skip project folder\" in the new-session wizard \
               (or press Ctrl+Shift+N) to start a session with no repo. AoE makes a temp \
               directory and cleans it up when you delete the session.",
    },
    Tip {
        id: "multi-repo-sessions",
        title: "Drive several repos at once",
        body: "Save your repos as projects, then multi-select them in the new-session wizard \
               to give one agent a worktree in every repo on a shared branch.",
    },
];

pub fn id_in_catalog(id: &str) -> bool {
    catalog().iter().any(|tip| tip.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[serial_test::serial]
    fn creation_count_merges_concurrent_writers_and_preserves_seen_state() {
        use crate::session::config::{update_app_state, AppStateConfig};
        let temp = tempfile::tempdir().unwrap();
        let _guard = crate::session::test_support::isolate_app_dir_at(temp.path());
        update_app_state(|state| state.tips_seen.push("other-tip".into())).unwrap();
        let barrier = std::sync::Barrier::new(4);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    barrier.wait();
                    for _ in 0..8 {
                        record_session_creations(1);
                    }
                });
            }
        });
        record_session_creations(0);
        let state = AppStateConfig::load().unwrap();
        assert_eq!(state.sessions_created, 32);
        assert_eq!(state.tips_seen, ["other-tip"]);
    }

    #[test]
    fn catalog_entries_are_complete_and_unique() {
        let ids: Vec<&str> = catalog().iter().map(|t| t.id).collect();
        assert!(!ids.is_empty());
        for tip in catalog() {
            assert!(!tip.id.is_empty(), "every tip needs an id");
            assert!(!tip.title.is_empty(), "every tip needs a title");
            assert!(!tip.body.is_empty(), "every tip needs a body");
            // Bodies render as-is, so a `{...}` placeholder would leak into the dashboard.
            assert!(!tip.body.contains('{'), "{} has a placeholder", tip.id);
            assert!(id_in_catalog(tip.id));
        }
        assert!(!id_in_catalog("nope"));
        assert!(!id_in_catalog(""));
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "tip ids must be unique");
    }
}
