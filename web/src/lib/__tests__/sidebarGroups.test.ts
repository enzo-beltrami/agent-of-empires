// @vitest-environment node

import { describe, expect, it } from "vitest";

import {
  archivableWorkspaces,
  buildSessionGroups,
  buildStatusGroups,
  sidebarGroupHasLiveWorkspace,
  statusGroupId,
  UNGROUPED_GROUP_ID,
  type SidebarGroup,
} from "../sidebarGroups";
import type { SidebarSortMode } from "../sidebarSort";
import { IDLE_DECAY_WINDOW_MS } from "../session";
import type { SessionResponse, SessionStatus, Workspace } from "../types";

function session(over: Partial<SessionResponse> = {}): SessionResponse {
  return {
    id: "s1",
    title: "t",
    project_path: "/repo-a",
    group_path: "",
    tool: "claude",
    status: "Idle",
    yolo_mode: false,
    lifecycle_generation: 0,
    created_at: "2025-01-01T00:00:00Z",
    last_accessed_at: null,
    idle_entered_at: null,
    last_error: null,
    branch: null,
    main_repo_path: null,
    is_sandboxed: false,
    favorited: false,
    has_managed_worktree: false,
    has_terminal: true,
    profile: "default",
    cleanup_defaults: { delete_worktree: false, delete_branch: false, delete_sandbox: false },
    remote_owner: null,
    notify_on_waiting: null,
    notify_on_idle: null,
    notify_on_error: null,
    claude_fullscreen: false,
    workspace_repos: [],
    scratch: false,
    ...over,
  };
}

function workspace(id: string, sessions: SessionResponse[]): Workspace {
  return {
    id,
    branch: null,
    projectPath: "/repo-a",
    displayName: id,
    agents: ["claude"],
    primaryAgent: "claude",
    status: "idle",
    sessions,
  };
}

/** A one-session workspace in `group_path`. */
const ws = (id: string, group_path: string, over: Partial<SessionResponse> = {}) =>
  workspace(id, [session({ id: `${id}-s`, group_path, ...over })]);

const archived = { archived_at: "2025-01-02T00:00:00Z" };
const wsIds = (group: SidebarGroup) => group.workspaces.map((v) => v.workspace.id);

const build = (
  workspaces: Workspace[],
  isCollapsed: (id: string) => boolean = () => false,
  sortMode: SidebarSortMode = "lastActivity",
  extra: Pick<Parameters<typeof buildSessionGroups>[1], "workspaceOrdering" | "groupOrder" | "colors"> = {},
) => buildSessionGroups(workspaces, { idleDecayWindowMs: IDLE_DECAY_WINDOW_MS, sortMode, isCollapsed, ...extra });

describe("buildSessionGroups", () => {
  it("orders rows by attention when sortMode is attention (#1640)", () => {
    const groups = build(
      [
        ws("w-running", "feature", { status: "Running" }),
        ws("w-waiting", "feature", { status: "Waiting" }),
        ws("w-idle", "feature", { status: "Idle" }),
      ],
      () => false,
      "attention",
    );
    expect(wsIds(groups[0]!)).toEqual(["w-waiting", "w-idle", "w-running"]);
  });

  it("buckets by group_path alphabetically with Ungrouped last", () => {
    const groups = build([ws("w1", "refactor"), ws("w0", ""), ws("w2", "feature"), ws("w3", "   ")]);
    expect(groups.map((g) => g.id)).toEqual(["feature", "refactor", UNGROUPED_GROUP_ID]);
    expect(groups.every((g) => g.kind === "sessionGroup")).toBe(true);
    expect(groups[0]!.groupPath).toBe("feature");
    expect(groups[2]).toMatchObject({ displayName: "Ungrouped", groupPath: "" });
    expect(wsIds(groups[2]!)).toEqual(["w0", "w3"]);
  });

  it("splits a workspace whose sessions span groups, keeping its real id", () => {
    const [feature, fix] = build([
      workspace("w1", [session({ id: "a", group_path: "feature" }), session({ id: "b", group_path: "fix" })]),
    ]);
    expect([feature!.id, fix!.id]).toEqual(["feature", "fix"]);
    expect([wsIds(feature!), wsIds(fix!)]).toEqual([["w1"], ["w1"]]);
    expect(feature!.workspaces[0]!.key).not.toBe(fix!.workspaces[0]!.key);
    expect(feature!.workspaces[0]!.workspace.sessions.map((s) => s.id)).toEqual(["a"]);
    expect(fix!.workspaces[0]!.workspace.sessions.map((s) => s.id)).toEqual(["b"]);
  });

  it("buckets paths differing only by surrounding slashes together", () => {
    const groups = build([ws("w1", "feature"), ws("w2", "feature/"), ws("w3", "/feature")]);
    expect(groups.map((g) => g.id)).toEqual(["feature"]);
    expect(wsIds(groups[0]!)).toEqual(["w1", "w2", "w3"]);
  });

  it("shows the full nested path so sibling leaves stay distinct", () => {
    const groups = build([ws("w1", "pushforward/PRs"), ws("w2", "chargeunpacker/PRs")]);
    expect(groups.map((g) => [g.id, g.displayName])).toEqual([
      ["chargeunpacker/PRs", "chargeunpacker / PRs"],
      ["pushforward/PRs", "pushforward / PRs"],
    ]);
  });

  it("orders rows by the persisted ordering in manual mode, pinned first", () => {
    const rows = [ws("w-a", "feature"), ws("w-b", "feature"), ws("w-c", "feature"), ws("w-pin", "feature")];
    rows[3]!.sessions[0]!.pinned_at = "2025-01-01T00:00:00Z";
    const [group] = build(rows, () => false, "manual", { workspaceOrdering: ["w-c", "w-a"] });
    expect(wsIds(group!)).toEqual(["w-pin", "w-c", "w-a", "w-b"]);
  });

  it("orders Groups by the dragged order, then alphabetically, with Ungrouped always last", () => {
    const groups = build(
      [ws("w0", ""), ws("w1", "alpha"), ws("w2", "beta"), ws("w3", "gamma")],
      () => false,
      "manual",
      {
        groupOrder: [UNGROUPED_GROUP_ID, "gamma", "alpha"],
      },
    );
    expect(groups.map((g) => g.id)).toEqual(["gamma", "alpha", "beta", UNGROUPED_GROUP_ID]);
  });

  it("reflects collapse state and each Group's color", () => {
    const [feature, fix] = build([ws("w1", "feature"), ws("w2", "fix")], (id) => id === "feature", "lastActivity", {
      colors: { feature: "teal" },
    });
    expect([feature!.collapsed, feature!.color]).toEqual([true, "teal"]);
    expect([fix!.collapsed, fix!.color]).toEqual([false, null]);
  });

  it("reports liveness", () => {
    expect(sidebarGroupHasLiveWorkspace(build([ws("w1", "feature", archived)])[0]!)).toBe(false);
    expect(sidebarGroupHasLiveWorkspace(build([ws("w1", "feature")])[0]!)).toBe(true);
  });
});

describe("buildStatusGroups", () => {
  const buildStatus = (workspaces: Workspace[], isCollapsed: (groupId: string) => boolean = () => false) =>
    buildStatusGroups(workspaces, {
      idleDecayWindowMs: IDLE_DECAY_WINDOW_MS,
      sortMode: "lastActivity",
      isCollapsed,
    });

  it("buckets by status in the Attention sort's order", () => {
    const groups = buildStatus([
      ws("w-idle", "", { status: "Idle" }),
      ws("w-waiting", "", { status: "Waiting" }),
      ws("w-running", "", { status: "Running" }),
      ws("w-error", "", { status: "Error" }),
    ]);
    expect(groups.map((g) => [g.id, g.displayName, g.kind])).toEqual([
      ["status:Waiting", "Waiting", "status"],
      ["status:Error", "Error", "status"],
      ["status:Idle", "Idle", "status"],
      ["status:Running", "Running", "status"],
    ]);
    expect(groups.map(wsIds)).toEqual([["w-waiting"], ["w-error"], ["w-idle"], ["w-running"]]);
  });

  it("ranks an unrecognised status with Unknown and transients last", () => {
    const fromNewerServer = workspace("w-new", [session({ id: "n", status: "Paused" as SessionStatus })]);
    const groups = buildStatus([
      ws("w-starting", "", { status: "Starting" }),
      fromNewerServer,
      ws("w-idle", "", { status: "Idle" }),
    ]);
    expect(groups.map((g) => g.displayName)).toEqual(["Idle", "Paused", "Starting"]);
  });

  it("splits a workspace whose sessions span statuses, keeping its real id", () => {
    const [waiting, running] = buildStatus([
      workspace("w1", [session({ id: "a", status: "Waiting" }), session({ id: "b", status: "Running" })]),
    ]);
    expect([waiting!.id, running!.id]).toEqual(["status:Waiting", "status:Running"]);
    expect([wsIds(waiting!), wsIds(running!)]).toEqual([["w1"], ["w1"]]);
    expect(waiting!.workspaces[0]!.key).not.toBe(running!.workspaces[0]!.key);
    expect(waiting!.workspaces[0]!.workspace.sessions.map((s) => s.id)).toEqual(["a"]);
    expect(running!.workspaces[0]!.workspace.sessions.map((s) => s.id)).toEqual(["b"]);
  });

  it("reflects collapse state and carries no Group path or color", () => {
    const [group] = buildStatus([ws("w1", "", { status: "Waiting" })], (id) => id === statusGroupId("Waiting"));
    expect(group).toMatchObject({ collapsed: true, color: null });
    expect(group!.groupPath).toBeUndefined();
  });
});

describe("archivableWorkspaces", () => {
  it.each<[string, Workspace[], string[]]>([
    ["skips archived primaries", [ws("w-live", "feature"), ws("w-archived", "feature", archived)], ["w-live"]],
    [
      "includes snoozed members",
      [ws("w-snoozed", "feature", { snoozed_until: "2999-01-01T00:00:00Z" })],
      ["w-snoozed"],
    ],
    [
      "keys off the primary session",
      [
        workspace("w1", [
          session({ id: "a", group_path: "feature" }),
          session({ id: "b", group_path: "feature", ...archived }),
        ]),
      ],
      ["w1"],
    ],
  ])("%s", (_name, workspaces, expected) => {
    expect(archivableWorkspaces(build(workspaces)[0]!).map((w) => w.id)).toEqual(expected);
  });
});
