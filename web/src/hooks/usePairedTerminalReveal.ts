import { useCallback } from "react";

import type { ShellMode } from "../components/PairedTerminal";
import type { RightPanelView } from "../lib/rightPanelView";
import { dispatchFocusTerminal, setPendingTerminalFocus } from "../lib/terminalFocus";

/**
 * Shows and focuses the mobile paired shell, never toggling back to the Agent; returns the terminal it shows.
 * `prepare` runs first with that terminal's index and returns whether it will remount the shell.
 */
export type RevealPairedTerminal = (prepare?: (terminalIndex: number) => boolean) => number;

export function usePairedTerminalReveal(setRightPanelView: (view: RightPanelView) => void): RevealPairedTerminal {
  return useCallback(
    (prepare) => {
      prepare?.(0);
      // One full-viewport pane: promote the paired view, then dispatch focus on the next frame, since
      // the inactive layer is inert until React commits the switch. The paired shell mounts lazily, so
      // its PTY may not be ready when the dispatch fires; latch the intent too.
      setRightPanelView("paired");
      setPendingTerminalFocus("paired", 0);
      requestAnimationFrame(() => dispatchFocusTerminal("paired", 0));
      return 0;
    },
    [setRightPanelView],
  );
}

/** Open Terminal for `sessionId`: only the active Session's, so it never switches another Session's shell. */
export function useOpenPairedTerminal(
  activeSessionId: string | null,
  reveal: RevealPairedTerminal,
  /** The mode the Session's shell shows now; a different one remounts it. */
  shownMode: (sessionId: string, terminalIndex: number) => ShellMode,
  setMode: (sessionId: string, terminalIndex: number, mode: ShellMode) => void,
): (sessionId: string, mode: ShellMode) => void {
  return useCallback(
    (sessionId: string, mode: ShellMode) => {
      if (sessionId !== activeSessionId) return;
      reveal((terminalIndex) => {
        const remounts = shownMode(sessionId, terminalIndex) !== mode;
        setMode(sessionId, terminalIndex, mode);
        return remounts;
      });
    },
    [activeSessionId, reveal, shownMode, setMode],
  );
}
