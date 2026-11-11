import { useCallback, useEffect, useState, type ReactNode } from "react";

import { ensureAssistant } from "../lib/api";
import type { SessionResponse } from "../lib/types";

interface Props {
  sessions: SessionResponse[];
  activeSessionId: string | null;
  renderSession: (session: SessionResponse) => ReactNode;
}

type State = { kind: "loading" } | { kind: "ready"; sessionId: string } | { kind: "error"; message: string };

/** The dashboard assistant: a structured session that can read and drive the others. */
export function AssistantPane({ sessions, activeSessionId, renderSession }: Props) {
  const [state, setState] = useState<State>({ kind: "loading" });

  const request = useCallback(() => {
    void ensureAssistant().then((result) =>
      setState(result.kind === "ok" ? { kind: "ready", sessionId: result.sessionId } : result),
    );
  }, []);
  const ensure = () => {
    setState({ kind: "loading" });
    request();
  };

  useEffect(request, [request]);

  if (state.kind === "error") {
    return (
      <Notice>
        <p>{state.message}</p>
        <button
          type="button"
          onClick={ensure}
          className="mt-2 cursor-pointer rounded-md px-2 py-1 text-text-secondary hover:bg-surface-700/50"
        >
          Try again
        </button>
      </Notice>
    );
  }
  const session = state.kind === "ready" ? sessions.find((s) => s.id === state.sessionId) : undefined;
  if (!session) {
    return (
      <Notice>
        <p>Starting the assistant…</p>
        {state.kind === "ready" && (
          <button
            type="button"
            onClick={ensure}
            className="mt-2 cursor-pointer rounded-md px-2 py-1 text-text-secondary hover:bg-surface-700/50"
          >
            Start a new assistant
          </button>
        )}
      </Notice>
    );
  }
  if (session.id === activeSessionId) {
    return <Notice>The assistant is open in the main view.</Notice>;
  }
  return <div className="flex h-full min-h-0 flex-col">{renderSession(session)}</div>;
}

function Notice({ children }: { children: ReactNode }) {
  return (
    <div
      data-testid="assistant-pane-notice"
      className="flex h-full flex-col items-center justify-center p-4 text-center text-sm text-text-dim"
    >
      {children}
    </div>
  );
}
