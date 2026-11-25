import type { GroupColor } from "./groupColor";
import type { SessionResponse, SessionStatus, Workspace, WorkspaceStatus } from "./types";
import { isSessionActive } from "./session";
import {
  compareWorkspacesForComputedSortMode,
  type SidebarSortMode,
  statusAttentionRank,
  workspaceIsSunk,
  workspaceTriageTier,
} from "./sidebarSort";

// Bucket for sessions with no `group_path`; also a localStorage collapse key.
export const UNGROUPED_GROUP_ID = "__ungrouped__";

export const COORDINATOR_GROUP_ID = "__coordinator__";

// In the group axis `sessions` is a per-group slice, so use `workspace.id` for actions and `key` only for render/DnD identity.
export interface SidebarWorkspaceView {
  key: string;
  workspace: Workspace;
}

export interface SidebarGroup {
  id: string;
  /**
   * A user Group reorders, renames, takes a color and accepts rows dropped from another; a status bucket does
   * none; the coordinator section is pinned above both and never collapses.
   */
  kind: "sessionGroup" | "status" | "coordinator";
  displayName: string;
  color: GroupColor | null;
  workspaces: SidebarWorkspaceView[];
  status: WorkspaceStatus;
  collapsed: boolean;
  /** Set when `kind === "sessionGroup"`. Empty string for Ungrouped. */
  groupPath?: string;
}

function normalizeGroupPath(path: string | null | undefined): string {
  const trimmed = (path ?? "").trim();
  if (trimmed === "") return "";
  // "feature" and "feature/" are the same group.
  return trimmed.replace(/^\/+|\/+$/g, "");
}

function groupDisplayName(path: string): string {
  if (path === "") return "Ungrouped";
  // Flat rendering shows the full path, since sibling groups can share a leaf name.
  return path.split("/").join(" / ");
}

/** Rows in a Group: by the persisted ordering in `manual` mode, else by the computed sort; pinned rows first, sunk last. */
function workspaceComparator(sortMode: SidebarSortMode, ordering: readonly string[]) {
  if (sortMode !== "manual") return compareWorkspacesForComputedSortMode(sortMode);
  const rank = new Map(ordering.map((id, i) => [id, i] as const));
  return (a: Workspace, b: Workspace) => {
    const tier = workspaceTriageTier(a) - workspaceTriageTier(b);
    if (tier !== 0) return tier;
    // `Infinity - Infinity` is NaN, so compare explicitly.
    const ar = rank.get(a.id) ?? Infinity;
    const br = rank.get(b.id) ?? Infinity;
    if (ar !== br) return ar < br ? -1 : 1;
    return a.id.localeCompare(b.id);
  };
}

// Split workspaces by per-session `group_path`; a workspace spanning groups appears once per group.
export function buildSessionGroups(
  workspaces: Workspace[],
  opts: {
    idleDecayWindowMs: number;
    sortMode: SidebarSortMode;
    isCollapsed: (groupId: string) => boolean;
    /** Persisted workspace ids, the row order in `manual` mode. */
    workspaceOrdering?: readonly string[];
    /** Group ids in their dragged order; unlisted Groups follow alphabetically and Ungrouped is always last. */
    groupOrder?: readonly string[];
    colors?: Readonly<Record<string, GroupColor>>;
  },
): SidebarGroup[] {
  const compareWorkspace = workspaceComparator(opts.sortMode, opts.workspaceOrdering ?? []);
  const byGroup = new Map<string, SidebarWorkspaceView[]>();

  for (const ws of workspaces) {
    const sessionsByGroup = new Map<string, SessionResponse[]>();
    for (const session of ws.sessions) {
      const gp = normalizeGroupPath(session.group_path);
      const existing = sessionsByGroup.get(gp);
      if (existing) existing.push(session);
      else sessionsByGroup.set(gp, [session]);
    }

    for (const [gp, sessions] of sessionsByGroup) {
      const sliced: Workspace = {
        ...ws,
        sessions,
        status: sessions.some((s) => isSessionActive(s, opts.idleDecayWindowMs)) ? "active" : "idle",
      };
      const view: SidebarWorkspaceView = { key: `${gp}::${ws.id}`, workspace: sliced };
      const bucket = byGroup.get(gp);
      if (bucket) bucket.push(view);
      else byGroup.set(gp, [view]);
    }
  }

  const groups: SidebarGroup[] = [...byGroup].map(([gp, views]) => {
    views.sort((a, b) => compareWorkspace(a.workspace, b.workspace));
    const id = gp === "" ? UNGROUPED_GROUP_ID : gp;
    return {
      id,
      kind: "sessionGroup",
      displayName: groupDisplayName(gp),
      color: opts.colors?.[id] ?? null,
      workspaces: views,
      status: views.some((v) => v.workspace.status === "active") ? "active" : "idle",
      collapsed: opts.isCollapsed(id),
      groupPath: gp,
    };
  });

  const groupRank = new Map((opts.groupOrder ?? []).map((id, i) => [id, i] as const));
  groups.sort((a, b) => {
    if (a.id === UNGROUPED_GROUP_ID) return 1;
    if (b.id === UNGROUPED_GROUP_ID) return -1;
    const ar = groupRank.get(a.id) ?? Infinity;
    const br = groupRank.get(b.id) ?? Infinity;
    if (ar !== br) return ar < br ? -1 : 1;
    return a.displayName.localeCompare(b.displayName);
  });

  return groups;
}

export const statusGroupId = (status: string) => `status:${status}`;

// Split workspaces by the status of each session; a workspace spanning statuses appears once per status.
export function buildStatusGroups(
  workspaces: Workspace[],
  opts: {
    idleDecayWindowMs: number;
    sortMode: SidebarSortMode;
    isCollapsed: (groupId: string) => boolean;
  },
): SidebarGroup[] {
  const compareWorkspace = compareWorkspacesForComputedSortMode(opts.sortMode);
  const byStatus = new Map<SessionStatus, SidebarWorkspaceView[]>();

  for (const ws of workspaces) {
    const sessionsByStatus = new Map<SessionStatus, SessionResponse[]>();
    for (const session of ws.sessions) {
      const existing = sessionsByStatus.get(session.status);
      if (existing) existing.push(session);
      else sessionsByStatus.set(session.status, [session]);
    }

    for (const [status, sessions] of sessionsByStatus) {
      const sliced: Workspace = {
        ...ws,
        sessions,
        status: sessions.some((s) => isSessionActive(s, opts.idleDecayWindowMs)) ? "active" : "idle",
      };
      const view: SidebarWorkspaceView = { key: `${status}::${ws.id}`, workspace: sliced };
      const bucket = byStatus.get(status);
      if (bucket) bucket.push(view);
      else byStatus.set(status, [view]);
    }
  }

  // The Attention sort's order, shared with the TUI and the Board's cards; ties fall
  // back to the status spelling so unknown statuses still get a stable place.
  const buckets = [...byStatus.entries()].sort(
    ([a], [b]) => statusAttentionRank(a) - statusAttentionRank(b) || a.localeCompare(b),
  );

  return buckets.map(([status, views]) => {
    views.sort((a, b) => compareWorkspace(a.workspace, b.workspace));
    const id = statusGroupId(status);
    return {
      id,
      kind: "status",
      displayName: status,
      color: null,
      workspaces: views,
      status: views.some((v) => v.workspace.status === "active") ? "active" : "idle",
      collapsed: opts.isCollapsed(id),
    };
  });
}

/** The coordinator's rows, kept out of every axis; `null` when there are none. */
export function buildCoordinatorGroup(workspaces: Workspace[], idleDecayWindowMs: number): SidebarGroup | null {
  if (workspaces.length === 0) return null;
  const views = workspaces.map((ws) => ({
    key: `${COORDINATOR_GROUP_ID}::${ws.id}`,
    workspace: {
      ...ws,
      status: ws.sessions.some((s) => isSessionActive(s, idleDecayWindowMs)) ? "active" : "idle",
    } satisfies Workspace,
  }));
  return {
    id: COORDINATOR_GROUP_ID,
    kind: "coordinator",
    displayName: "Coordinator",
    color: null,
    workspaces: views,
    status: views.some((v) => v.workspace.status === "active") ? "active" : "idle",
    collapsed: false,
  };
}

export function sidebarGroupHasLiveWorkspace(group: SidebarGroup): boolean {
  return group.workspaces.some((v) => !workspaceIsSunk(v.workspace));
}

// Workspaces whose primary session (`sessions[0]`, the triage target) is not archived.
export function archivableWorkspaces(group: SidebarGroup): Workspace[] {
  return group.workspaces
    .map((v) => v.workspace)
    .filter((ws) => {
      const primary = ws.sessions[0];
      return primary != null && primary.archived_at == null;
    });
}
