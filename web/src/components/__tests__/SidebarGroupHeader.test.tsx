// @vitest-environment jsdom
// Drag-release click suppression is timing dependent in a browser, so it is pinned here via `isDragging`.

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

import { SidebarGroupHeader } from "../sidebar/SidebarGroupHeader";
import type { SidebarGroup } from "../../lib/sidebarGroups";
import type { SessionStatus, Workspace } from "../../lib/types";
import { makeSession, makeWorkspace } from "./fixtures";

type Sunk = "archived" | "snoozed";
const SUNK = { archived: { archived_at: "2025-01-02T00:00:00Z" }, snoozed: { snoozed_until: "2099-01-01T00:00:00Z" } };

function workspace(id: string, statuses: SessionStatus[], sunk?: Sunk): Workspace {
  return makeWorkspace(
    id,
    statuses.map((status, i) => makeSession({ id: `${id}-s${i}`, title: id, status, ...(sunk && SUNK[sunk]) })),
  );
}
const idle = (n: number) => Array<SessionStatus>(n).fill("Idle");
const views = (...wss: Workspace[]) => wss.map((w) => ({ key: w.id, workspace: w }));

function group(over: Partial<SidebarGroup> = {}): SidebarGroup {
  return {
    id: "work/payto",
    kind: "sessionGroup",
    displayName: "work / payto",
    color: null,
    workspaces: views(workspace("w1", idle(3))),
    status: "idle",
    collapsed: false,
    groupPath: "work/payto",
    ...over,
  };
}

const dragHandle = () => ({
  setActivatorNodeRef: () => {},
  attributes: {},
  listeners: {},
});

function renderHeader(props: Partial<Parameters<typeof SidebarGroupHeader>[0]> = {}) {
  const onClick = vi.fn();
  const onNewSession = vi.fn();
  const ui = (p: typeof props) => (
    <SidebarGroupHeader
      group={group()}
      hasActiveChild={false}
      onClick={onClick}
      onNewSession={onNewSession}
      onSetColor={() => {}}
      onRename={() => {}}
      offline={false}
      rowsId="sidebar-rows-test"
      {...p}
    />
  );
  const { rerender } = render(ui(props));
  return { onClick, onNewSession, rerender: (p: typeof props) => rerender(ui(p)) };
}

const text = (id: string) => screen.queryByTestId(id)?.textContent;

afterEach(cleanup);

describe("SidebarGroupHeader", () => {
  it.each([
    ["two live workspaces", views(workspace("a", idle(2)), workspace("b", idle(3))), "(2)"],
    // Sunk rows render in the footer, so they are not counted (#2372).
    [
      "live plus sunk",
      views(workspace("live", idle(1)), workspace("arch", idle(1), "archived"), workspace("zz", idle(1), "snoozed")),
      "(1)",
    ],
  ])("counts %s as %s", (_n, workspaces, count) => {
    renderHeader({ group: group({ workspaces }) });
    expect(text("sidebar-group-session-count")).toBe(count);
  });

  it.each([
    ["Waiting and Error sessions", views(workspace("a", ["Waiting", "Running"]), workspace("b", ["Error"])), "2"],
    ["nothing needing attention", views(workspace("w1", idle(3))), undefined],
    // Sunk wins over Waiting.
    ["waiting but archived sessions", views(workspace("arch", ["Waiting", "Waiting"], "archived")), undefined],
  ])("attention badge for %s is %s", (_n, workspaces, badge) => {
    renderHeader({ group: group({ workspaces }) });
    expect(text("sidebar-group-attention-badge")).toBe(badge);
  });

  it("shows a Group icon for a user Group and an activity icon for a status bucket", () => {
    const icon = () => screen.getByTestId("sidebar-group-icon").querySelector("svg")!.getAttribute("class");
    renderHeader();
    expect(icon()).toContain("lucide-layers");
    cleanup();
    renderHeader({
      group: group({ kind: "status", groupPath: undefined }),
      onSetColor: undefined,
      onRename: undefined,
    });
    expect(icon()).toContain("lucide-activity");
  });

  it("is one disclosure button that controls its rows and names its menu shortcut", () => {
    const { rerender } = renderHeader();
    const header = screen.getByTestId("sidebar-group-header");
    const button = header.querySelector("button[aria-expanded]")!;
    expect(header.hasAttribute("tabindex")).toBe(false);
    expect(header.getAttribute("aria-haspopup")).toBeNull();
    expect(button.getAttribute("aria-expanded")).toBe("true");
    expect(button.getAttribute("aria-controls")).toBe("sidebar-rows-test");
    expect(button.getAttribute("aria-keyshortcuts")).toBe("Shift+F10");
    rerender({ group: group({ collapsed: true }) });
    expect(button.getAttribute("aria-expanded")).toBe("false");
  });

  it("returns focus to the header after a menu action that keeps it, but not after Rename", () => {
    // (menu item, props, focus back on the header button)
    const cases: [string, Partial<Parameters<typeof SidebarGroupHeader>[0]>, boolean][] = [
      ["sidebar-group-context-menu-archive-all", { onArchiveAll: vi.fn() }, true],
      ["sidebar-group-context-menu-rename", {}, false],
    ];
    for (const [item, props, toHeader] of cases) {
      renderHeader(props);
      const button = screen.getByTestId("sidebar-group-header").querySelector<HTMLElement>("button[aria-expanded]")!;
      button.focus();
      fireEvent.keyDown(button, { key: "F10", shiftKey: true });
      const action = screen.queryByTestId(item) ?? screen.getByText(item);
      // Keyboard activation of a menu button is a click on the focused item.
      action.focus();
      fireEvent.click(action);
      expect(document.activeElement === button, item).toBe(toHeader);
      cleanup();
    }
  });

  it("returns focus to the header when a rename ends with Enter or Escape", async () => {
    for (const key of ["Enter", "Escape"]) {
      renderHeader();
      fireEvent.contextMenu(screen.getByTestId("sidebar-group-header"));
      fireEvent.click(screen.getByTestId("sidebar-group-context-menu-rename"));
      const input = screen.getByTestId("sidebar-group-rename-input");
      input.focus();
      fireEvent.keyDown(input, { key });
      await new Promise((resolve) => requestAnimationFrame(resolve));
      const button = screen.getByTestId("sidebar-group-header").querySelector("button[aria-expanded]");
      expect(document.activeElement, key).toBe(button);
      cleanup();
    }
  });

  it("renames to the trimmed full path, and ignores a blank or unchanged one", () => {
    // (typed value, how the edit ends, the rename sent)
    const cases: [string, string, string | null][] = [
      ["  work/payments  ", "Enter", "work/payments"],
      ["work/payments", "Escape", null],
      ["   ", "Enter", null],
      ["work/payto", "Enter", null],
    ];
    for (const [value, key, sent] of cases) {
      const onRename = vi.fn();
      renderHeader({ onRename });
      fireEvent.contextMenu(screen.getByTestId("sidebar-group-header"));
      fireEvent.click(screen.getByTestId("sidebar-group-context-menu-rename"));
      const input = screen.getByTestId<HTMLInputElement>("sidebar-group-rename-input");
      expect(input.value).toBe("work/payto");
      fireEvent.change(input, { target: { value } });
      fireEvent.keyDown(input, { key });
      expect(onRename.mock.calls, `${value} ${key}`).toEqual(sent ? [[sent]] : []);
      cleanup();
    }
  });

  it("picks and clears a color, and offers only the menu items its handlers allow", () => {
    const onSetColor = vi.fn();
    renderHeader({ group: group({ color: "teal" }), onSetColor });
    fireEvent.contextMenu(screen.getByTestId("sidebar-group-header"));
    expect(screen.getByTestId("sidebar-group-color-teal").getAttribute("aria-pressed")).toBe("true");
    fireEvent.click(screen.getByTestId("sidebar-group-color-rose"));
    fireEvent.click(screen.getByTestId("sidebar-group-color-clear"));
    expect(onSetColor.mock.calls).toEqual([["rose"], [null]]);
    cleanup();

    // Ungrouped and read-only headers cannot rename; a status bucket has no menu at all.
    renderHeader({ group: group({ id: "__ungrouped__", groupPath: "" }), onRename: undefined });
    fireEvent.contextMenu(screen.getByTestId("sidebar-group-header"));
    expect(screen.queryByTestId("sidebar-group-context-menu-rename")).toBeNull();
    expect(screen.getByTestId("sidebar-group-color-amber")).toBeTruthy();
    cleanup();
    renderHeader({ group: group({ kind: "status" }), onSetColor: undefined, onRename: undefined });
    fireEvent.contextMenu(screen.getByTestId("sidebar-group-header"));
    expect(screen.queryByTestId("sidebar-group-context-menu")).toBeNull();
  });

  it("marks the header draggable only with a drag handle", () => {
    const { rerender } = renderHeader({ dragHandle: dragHandle() });
    expect(screen.getByTestId("sidebar-group-header").getAttribute("data-draggable")).toBe("true");
    rerender({});
    expect(screen.getByTestId("sidebar-group-header").getAttribute("data-draggable")).toBeNull();
  });
});
