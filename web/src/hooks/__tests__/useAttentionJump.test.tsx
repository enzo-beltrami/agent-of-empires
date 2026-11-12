// @vitest-environment jsdom

import { renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { repoGroupToSidebarGroup } from "../../lib/sidebarGroups";
import { nextAttentionSessionId } from "../../lib/sidebarSort";
import { repoGroup, session, workspace } from "./fixtures";
import { useAttentionJump } from "../useAttentionJump";

const ws = (id: string, over = {}) => workspace(`w-${id}`, "/repo", [session({ id, ...over })]);
const groupsOf = (...workspaces: ReturnType<typeof ws>[]) => [repoGroupToSidebarGroup(repoGroup({ workspaces }))];

describe("useAttentionJump (#4446)", () => {
  it("lists sessions in display order and leaves calm sessions out of the attention set", () => {
    const { orderedIds, attention } = renderHook(() => useAttentionJump(groupsOf(ws("a"), ws("b"), ws("c")))).result
      .current;
    expect(orderedIds).toEqual(["a", "b", "c"]);
    expect(attention.size).toBe(0);
    expect(nextAttentionSessionId(orderedIds, attention, "a")).toBeNull();
  });

  it("jumps to a Waiting or Error session, skipping sunk ones", () => {
    const groups = groupsOf(
      ws("a"),
      ws("b", { status: "Error" }),
      ws("c", { status: "Waiting", snoozed_until: "2099-01-01T00:00:00Z" }),
    );
    const { orderedIds, attention } = renderHook(() => useAttentionJump(groups)).result.current;
    expect([...attention]).toEqual(["b"]);
    expect(nextAttentionSessionId(orderedIds, attention, "a")).toBe("b");
  });

  it("keeps the result across renders with the same groups", () => {
    const groups = groupsOf(ws("a", { status: "Waiting" }));
    const { result, rerender } = renderHook(() => useAttentionJump(groups));
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
  });
});
