import { lazy, Suspense } from "react";

import { StructuredSessionStack } from "./StructuredSessionStack";
import { TerminalSessionStack } from "./TerminalSessionStack";
import { PairedShellPane, type ShellMode } from "./PairedTerminal";
import { hasStartedBefore } from "./acp/startupWait";
import type { RightPanelView } from "../lib/rightPanelView";
import type { SessionResponse } from "../lib/types";
import type { WebSettings } from "../hooks/useWebSettings";
import type { ServerAbout } from "../lib/api";
import { autoResumeSettingsHref } from "../lib/autoResumeSettingsHref";

const StructuredView = lazy(() => import("./acp/StructuredView").then((m) => ({ default: m.StructuredView })));

interface Props {
  /** Start a Session, for a queued row's "Start and send". */
  onStartSession?: (sessionId: string) => void;
  readOnly: boolean;
  view: RightPanelView;
  onBackToAgent: () => void;
  /** Opens the switch-view confirmation for that Session; omitted where it can't switch. */
  onSwitchToTerminal?: (sessionId: string) => void;
  /** Shows that Session's paired shell in `mode`; omitted where the terminal can't be used. */
  onOpenTerminal?: (sessionId: string, mode: ShellMode) => void;
  /** Whether this server lets the dashboard restart an Agent. */
  canRestartAgent: boolean;
  /** The active Session's mode for its primary paired shell. */
  shellMode: ShellMode;
  onShellModeChange: (mode: ShellMode) => void;
  pairedMounted: boolean;
  activeSession: SessionResponse | null;
  activeSessionId: string | null;
  sessions: SessionResponse[];
  webSettings: Pick<
    WebSettings,
    "persistentTerminals" | "maxPersistentTerminals" | "persistentStructuredViews" | "maxPersistentStructuredViews"
  >;
  /** Decides whether the usage-limit banner links to Settings. */
  serverAbout: ServerAbout | null;
}

function layerClass(active: boolean): string {
  const base = "absolute inset-0 flex flex-col min-h-0 overflow-hidden";
  return active ? base : `${base} invisible pointer-events-none`;
}

/** Keep terminal geometry and scrollback across switches; only the visible
 *  surface owns keyboard input. */
export function MobileMainPane({
  onStartSession,
  readOnly,
  view,
  onBackToAgent,
  onSwitchToTerminal,
  onOpenTerminal,
  canRestartAgent,
  shellMode,
  onShellModeChange,
  pairedMounted,
  activeSession,
  activeSessionId,
  sessions,
  webSettings,
  serverAbout,
}: Props) {
  return (
    <div className="flex-1 flex flex-col min-h-0">
      {view !== "agent" && (
        <div className="flex items-center gap-2 h-9 px-2 border-b border-surface-700/60 bg-surface-900 shrink-0">
          <button
            onClick={onBackToAgent}
            data-testid="mobile-back-to-agent"
            className="flex items-center gap-1 px-2 py-1 rounded-md text-xs text-text-secondary hover:text-text-primary hover:bg-surface-800 cursor-pointer transition-colors"
          >
            <span aria-hidden>&larr;</span> Agent
          </button>
          <span className="text-xs text-text-dim">Paired terminal</span>
        </div>
      )}
      <div className="relative flex-1 flex flex-col min-h-0 overflow-hidden">
        <div className={layerClass(view === "agent")} inert={view !== "agent"}>
          {activeSession?.view === "structured" ? (
            <StructuredSessionStack
              active={view === "agent"}
              activeSessionId={activeSessionId!}
              sessions={sessions.filter((session) => session.view === "structured")}
              persistent={webSettings.persistentStructuredViews}
              maxPersistentViews={webSettings.maxPersistentStructuredViews}
              renderSession={(session, active, selected) => (
                <Suspense fallback={null}>
                  <StructuredView
                    active={active}
                    selected={selected}
                    sessionId={session.id}
                    acpWorkerState={session.acp_worker_state ?? "absent"}
                    rateLimitAutoResume={session.rate_limit_auto_resume}
                    settingsHref={autoResumeSettingsHref(session.profile, serverAbout)}
                    tool={session.tool}
                    acpAgent={session.acp_agent ?? null}
                    startedBefore={hasStartedBefore(session)}
                    acpProvider={session.acp_provider ?? null}
                    clearAliases={session.clear_aliases}
                    compactAliases={session.compact_aliases}
                    onStartSession={onStartSession && (() => onStartSession(session.id))}
                    archivedAt={session.archived_at ?? null}
                    snoozedUntil={session.snoozed_until ?? null}
                    trashedAt={session.trashed_at ?? null}
                    fileRefSession={session}
                    isSandboxed={session.is_sandboxed}
                    onSwitchToTerminal={onSwitchToTerminal ? () => onSwitchToTerminal(session.id) : undefined}
                    onOpenTerminal={onOpenTerminal ? (mode) => onOpenTerminal(session.id, mode) : undefined}
                    canRestartAgent={canRestartAgent}
                  />
                </Suspense>
              )}
            />
          ) : (
            // Clear the home indicator without changing the keyboard-open lift.
            <div className="home-indicator-clearance flex-1 flex flex-col min-h-0 overflow-hidden">
              <TerminalSessionStack
                readOnly={readOnly}
                active={view === "agent"}
                activeSessionId={activeSessionId!}
                sessions={sessions.filter((session) => session.view !== "structured")}
                persistent={webSettings.persistentTerminals}
                maxPersistentTerminals={webSettings.maxPersistentTerminals}
              />
            </div>
          )}
        </div>

        {pairedMounted && (
          // Match the agent terminal’s home-indicator clearance.
          <div
            className={`home-indicator-clearance ${layerClass(view === "paired")}`}
            inert={view !== "paired"}
            data-testid="mobile-paired-layer"
          >
            <PairedShellPane
              session={activeSession}
              sessionId={activeSessionId}
              readOnly={readOnly}
              active={view === "paired"}
              mode={shellMode}
              onModeChange={onShellModeChange}
            />
          </div>
        )}
      </div>
    </div>
  );
}
