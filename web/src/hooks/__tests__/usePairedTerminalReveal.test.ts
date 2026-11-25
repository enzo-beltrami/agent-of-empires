// @vitest-environment jsdom

import { renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { useOpenPairedTerminal, usePairedTerminalReveal } from "../usePairedTerminalReveal";

const focus = vi.hoisted(() => ({ dispatchFocusTerminal: vi.fn(), setPendingTerminalFocus: vi.fn() }));
vi.mock("../../lib/terminalFocus", () => focus);

afterEach(() => vi.clearAllMocks());

describe("usePairedTerminalReveal", () => {
  it("promotes the paired view, latches focus for its terminal, and focuses it next frame", () => {
    const raf = vi.spyOn(window, "requestAnimationFrame").mockImplementation((cb) => {
      cb(0);
      return 0;
    });
    const setRightPanelView = vi.fn();
    const prepare = vi.fn(() => true);
    const { result } = renderHook(() => usePairedTerminalReveal(setRightPanelView));

    expect(result.current(prepare)).toBe(0);
    expect(prepare.mock.calls).toEqual([[0]]);
    expect(setRightPanelView.mock.calls).toEqual([["paired"]]);
    expect(focus.setPendingTerminalFocus.mock.calls).toEqual([["paired", 0]]);
    expect(focus.dispatchFocusTerminal.mock.calls).toEqual([["paired", 0]]);
    raf.mockRestore();
  });
});

describe("useOpenPairedTerminal", () => {
  it("sets the chosen mode on the shown terminal, only for the active Session, and says if it remounts", () => {
    const remounts: boolean[] = [];
    const reveal = vi.fn((prepare?: (terminalIndex: number) => boolean) => {
      remounts.push(prepare!(2));
      return 2;
    });
    const shownMode = vi.fn(() => "host" as const);
    const setMode = vi.fn();
    const { result } = renderHook(() => useOpenPairedTerminal("active", reveal, shownMode, setMode));

    result.current("other", "container");
    expect(reveal).not.toHaveBeenCalled();
    expect(setMode).not.toHaveBeenCalled();

    result.current("active", "container");
    result.current("active", "host");
    expect(setMode.mock.calls).toEqual([
      ["active", 2, "container"],
      ["active", 2, "host"],
    ]);
    expect(shownMode).toHaveBeenCalledWith("active", 2);
    expect(remounts).toEqual([true, false]);
  });
});
