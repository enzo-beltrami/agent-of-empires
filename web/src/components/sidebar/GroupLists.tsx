import type { ComponentProps, ReactNode } from "react";
import { DndContext, MouseSensor, TouchSensor, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, arrayMove, verticalListSortingStrategy } from "@dnd-kit/sortable";
import type { GroupColor } from "../../lib/groupColor";
import { sidebarGroupHasLiveWorkspace, type SidebarGroup, type SidebarWorkspaceView } from "../../lib/sidebarGroups";
import type { Workspace } from "../../lib/types";
import { workspaceIsPinned, workspaceIsSunk } from "../../lib/sidebarSort";
import { dragEnded, dragStarted, typedClosestCenter, useReleaseDragOnUnmount, type DragClickGuard } from "./dnd";
import { rowDrop } from "./rowDrop";
import { SessionRow } from "./SessionRow";
import { SidebarGroupHeader } from "./SidebarGroupHeader";
import { ROWS, rowsId } from "./keyboardNav";
import { SortableGroup, SortableSessionRow } from "./Sortable";
import type { DragHandleProps } from "./types";

type RowProps = Omit<ComponentProps<typeof SessionRow>, "indented">;

export interface ListContext {
  hasFilter: boolean;
  readOnly?: boolean;
  offline: boolean;
  displayedActiveId: string | null;
  displayedActiveSessionId: string | null;
  /** Opens the wizard with `groupPath` prefilled; "" for Ungrouped. */
  onNewInGroup: (groupPath: string) => void;
  /** New session next to a row's own repo, from its context menu. */
  onCreateSession: (repoPath: string) => void;
  onRenameGroup: (group: SidebarGroup, to: string) => void;
  onSetGroupColor: (groupId: string, color: GroupColor | null) => void;
  onArchiveGroup: (group: SidebarGroup) => void;
  rowProps: (v: SidebarWorkspaceView) => RowProps;
}

const liveViews = (group: SidebarGroup) => group.workspaces.filter((v) => !workspaceIsSunk(v.workspace));
// A status slice holds one session, so the workspace id alone would mark every slice.
const containsActive = (views: SidebarWorkspaceView[], id: string | null, sessionId: string | null) =>
  views.some(
    (v) => v.workspace.id === id && (sessionId == null || v.workspace.sessions.some((s) => s.id === sessionId)),
  );

/** Wraps one Group's rows, collapsed or not, so its header's `aria-controls` always resolves. */
function GroupRows({ id, children }: { id: string; children: ReactNode }) {
  return (
    <div id={id} {...{ [ROWS]: "" }}>
      {children}
    </div>
  );
}

/** `full` is the unfiltered group so "Archive all" covers hidden members. */
function GroupHeader({
  ctx,
  full,
  visible,
  onToggle,
  dragHandle,
  rows,
}: {
  ctx: ListContext;
  full: SidebarGroup;
  visible: SidebarWorkspaceView[];
  onToggle: () => void;
  dragHandle?: DragHandleProps;
  /** Id of the `GroupRows` this header controls. */
  rows: string;
}) {
  const expanded = ctx.hasFilter || !full.collapsed;
  const locked = ctx.readOnly || ctx.offline;
  const userGroup = full.kind === "sessionGroup";
  return (
    <SidebarGroupHeader
      group={{ ...full, collapsed: !expanded }}
      hasActiveChild={!expanded && containsActive(visible, ctx.displayedActiveId, ctx.displayedActiveSessionId)}
      onClick={() => !ctx.hasFilter && onToggle()}
      onSetColor={userGroup ? (color) => ctx.onSetGroupColor(full.id, color) : undefined}
      onRename={!locked && userGroup && full.groupPath ? (to) => ctx.onRenameGroup(full, to) : undefined}
      onArchiveAll={locked ? undefined : () => ctx.onArchiveGroup(full)}
      onNewSession={() => ctx.onNewInGroup(full.groupPath ?? "")}
      offline={ctx.offline}
      dragHandle={dragHandle}
      rowsId={rows}
    />
  );
}

/**
 * Every Group with its rows. In `manual` order a header drags to reorder Groups, and a row drags to
 * reorder within its Group or onto a row of another Group to move its sessions there.
 */
export function GroupList({
  ctx,
  groups,
  fullGroups,
  reorderDisabled,
  groupDragDisabled,
  onToggleGroup,
  onReorderGroups,
  onReorderWorkspaces,
  onMoveToGroup,
  dragClickGuard,
}: {
  ctx: ListContext;
  groups: SidebarGroup[];
  fullGroups: SidebarGroup[];
  reorderDisabled: boolean;
  groupDragDisabled: boolean;
  onToggleGroup: (groupId: string) => void;
  onReorderGroups: (orderedGroupIds: string[]) => void;
  onReorderWorkspaces: (newOrder: string[]) => void;
  /** Moves a row's sessions to `groupPath`, then persists `newOrder`. */
  onMoveToGroup: (workspace: Workspace, groupPath: string, newOrder: string[]) => void;
  dragClickGuard: React.MutableRefObject<DragClickGuard>;
}) {
  useReleaseDragOnUnmount(dragClickGuard);
  // Desktop activates on distance so a still click navigates; touch needs a hold so flicks scroll.
  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 8 } }),
    useSensor(TouchSensor, { activationConstraint: { delay: 150, tolerance: 8 } }),
  );
  const byId = new Map(fullGroups.map((g) => [g.id, g]));

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    if (active.data.current?.type === "group") {
      if (over.data.current?.type !== "group") return;
      const ids = fullGroups.map((g) => g.id);
      const from = ids.indexOf(String(active.id));
      const to = ids.indexOf(String(over.id));
      if (from >= 0 && to >= 0) onReorderGroups(arrayMove(ids, from, to));
      return;
    }
    const drop = rowDrop(fullGroups, String(active.id), String(over.id));
    if (!drop) return;
    if (drop.target) onMoveToGroup(drop.moved, drop.target.groupPath ?? "", drop.order);
    else onReorderWorkspaces(drop.order);
  };

  const liveGroups = groups.filter(sidebarGroupHasLiveWorkspace);
  const body = (group: SidebarGroup, dragHandle?: DragHandleProps) => {
    const live = liveViews(group);
    return (
      <>
        <GroupHeader
          ctx={ctx}
          full={{ ...(byId.get(group.id) ?? group), collapsed: group.collapsed }}
          visible={group.workspaces}
          onToggle={() => onToggleGroup(group.id)}
          dragHandle={dragHandle}
          rows={rowsId(group.id)}
        />
        <GroupRows id={rowsId(group.id)}>
          {(ctx.hasFilter || !group.collapsed) && (
            <SortableContext items={live.map((v) => v.key)} strategy={verticalListSortingStrategy}>
              {live.map((v) => (
                <SortableSessionRow
                  key={v.key}
                  {...ctx.rowProps(v)}
                  onCreateSession={ctx.onCreateSession}
                  // Pinned rows float to the top of their group, so dragging them is meaningless.
                  dragDisabled={reorderDisabled || workspaceIsPinned(v.workspace)}
                />
              ))}
            </SortableContext>
          )}
        </GroupRows>
      </>
    );
  };

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={typedClosestCenter}
      onDragStart={() => dragStarted(dragClickGuard.current)}
      onDragEnd={(event) => {
        dragEnded(dragClickGuard.current);
        if (!reorderDisabled) handleDragEnd(event);
      }}
      onDragCancel={() => dragEnded(dragClickGuard.current)}
    >
      <SortableContext items={liveGroups.map((g) => g.id)} strategy={verticalListSortingStrategy}>
        {liveGroups.map((group) => (
          <SortableGroup key={group.id} groupId={group.id} disabled={groupDragDisabled}>
            {(handle) => body(group, groupDragDisabled ? undefined : handle)}
          </SortableGroup>
        ))}
      </SortableContext>
    </DndContext>
  );
}
