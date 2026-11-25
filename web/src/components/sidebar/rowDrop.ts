import type { SidebarGroup } from "../../lib/sidebarGroups";
import type { Workspace } from "../../lib/types";

/**
 * Where a row dropped on another row lands. `order` is the full flat workspace order to persist, so every
 * client agrees; a workspace split across Groups keeps its first place. `target` is set when the row
 * crossed into another Group.
 */
export function rowDrop(
  groups: SidebarGroup[],
  activeKey: string,
  overKey: string,
): { moved: Workspace; order: string[]; target: SidebarGroup | null } | null {
  const groupIndexOf = (key: string) => groups.findIndex((g) => g.workspaces.some((v) => v.key === key));
  const fromGroup = groupIndexOf(activeKey);
  const toGroup = groupIndexOf(overKey);
  if (fromGroup < 0 || toGroup < 0) return null;
  const lists = groups.map((g) => [...g.workspaces]);
  const from = lists[fromGroup]!.findIndex((v) => v.key === activeKey);
  const to = lists[toGroup]!.findIndex((v) => v.key === overKey);
  const [moved] = lists[fromGroup]!.splice(from, 1);
  if (!moved) return null;
  lists[toGroup]!.splice(to, 0, moved);
  return {
    moved: moved.workspace,
    order: [...new Set(lists.flatMap((list) => list.map((v) => v.workspace.id)))],
    target: fromGroup === toGroup ? null : groups[toGroup]!,
  };
}
