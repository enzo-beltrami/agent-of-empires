import { useMemo } from "react";
import type { Workspace } from "../lib/types";
import { buildSessionGroups, type SidebarGroup } from "../lib/sidebarGroups";
import type { SidebarSortMode } from "../lib/sidebarSort";
import { useIdleDecayWindowMs } from "../lib/idleDecay";
import { useCollapsedKeys } from "./useCollapsedKeys";

export function useSessionGroups(
  workspaces: Workspace[],
  sortMode: SidebarSortMode,
): {
  groups: SidebarGroup[];
  toggleGroupCollapsed: (groupId: string) => void;
} {
  const idleDecayWindowMs = useIdleDecayWindowMs();
  const { isCollapsed, toggle } = useCollapsedKeys("aoe-group-collapsed-");

  const groups = useMemo(
    () => buildSessionGroups(workspaces, { idleDecayWindowMs, sortMode, isCollapsed }),
    [workspaces, idleDecayWindowMs, sortMode, isCollapsed],
  );

  return { groups, toggleGroupCollapsed: toggle };
}
