// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from "vitest";
import { renderHook, act } from "@testing-library/react";

import { useSessionGroups } from "../useSessionGroups";
import { session, workspace } from "./fixtures";

const groupedWorkspace = (group_path: string, over = {}) =>
  workspace("w1", "/repo", [session({ group_path, branch: "feat", main_repo_path: "/repo", ...over })], {
    branch: "feat",
    displayName: "feat",
  });

beforeEach(() => localStorage.clear());

describe("sidebar group collapse persistence", () => {
  it("useSessionGroups builds a group per group_path and persists its collapse", () => {
    const render = () => renderHook(() => useSessionGroups([groupedWorkspace("team-a")], "lastActivity"));
    const { result } = render();
    expect(result.current.groups).toHaveLength(1);
    const id = result.current.groups[0]!.id;
    const key = `aoe-group-collapsed-${id}`;

    act(() => result.current.toggleGroupCollapsed(id));
    expect(result.current.groups[0]!.collapsed).toBe(true);
    expect(localStorage.getItem(key)).toBe("1");
    expect(render().result.current.groups[0]!.collapsed).toBe(true);

    act(() => result.current.toggleGroupCollapsed(id));
    expect(result.current.groups[0]!.collapsed).toBe(false);
    expect(localStorage.getItem(key)).toBeNull();
  });

  it("useSessionGroups persists the dragged order and colors, and carries both through a rename", () => {
    const workspaces = [
      workspace("w1", "/repo", [session({ id: "a", group_path: "alpha" })]),
      workspace("w2", "/repo", [session({ id: "b", group_path: "beta" })]),
    ];
    const render = () => renderHook(({ ws }) => useSessionGroups(ws, "manual"), { initialProps: { ws: workspaces } });
    const { result, rerender } = render();
    const ids = () => result.current.groups.map((g) => g.id);
    expect(ids()).toEqual(["alpha", "beta"]);

    act(() => result.current.reorderGroups(["beta", "alpha"]));
    act(() => result.current.setGroupColor("alpha", "rose"));
    expect(ids()).toEqual(["beta", "alpha"]);
    expect(render().result.current.groups.map((g) => [g.id, g.color])).toEqual([
      ["beta", null],
      ["alpha", "rose"],
    ]);

    act(() => result.current.carryGroupPrefs("alpha", "omega"));
    rerender({ ws: [workspace("w1", "/repo", [session({ id: "a", group_path: "omega" })]), workspaces[1]!] });
    expect(result.current.groups.map((g) => [g.id, g.color])).toEqual([
      ["beta", null],
      ["omega", "rose"],
    ]);
  });
});
