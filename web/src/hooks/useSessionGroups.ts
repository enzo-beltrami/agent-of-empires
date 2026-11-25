import { useCallback, useMemo, useState } from "react";
import type { Workspace } from "../lib/types";
import { buildCoordinatorGroup, buildSessionGroups, type SidebarGroup } from "../lib/sidebarGroups";
import type { SidebarSortMode } from "../lib/sidebarSort";
import { useIdleDecayWindowMs } from "../lib/idleDecay";
import { loadGroupOrder, persistGroupOrder } from "../lib/groupOrder";
import { loadGroupColors, persistGroupColors, withGroupColor, type GroupColor } from "../lib/groupColor";
import { useCollapsedKeys } from "./useCollapsedKeys";

export function useSessionGroups(
  workspaces: Workspace[],
  sortMode: SidebarSortMode,
  workspaceOrdering: readonly string[] = [],
): {
  groups: SidebarGroup[];
  toggleGroupCollapsed: (groupId: string) => void;
  reorderGroups: (orderedGroupIds: string[]) => void;
  setGroupColor: (groupId: string, color: GroupColor | null) => void;
  /** Moves a renamed Group's color and place to its new id. */
  carryGroupPrefs: (fromId: string, toId: string) => void;
} {
  const idleDecayWindowMs = useIdleDecayWindowMs();
  const { isCollapsed, toggle } = useCollapsedKeys("aoe-group-collapsed-");
  const [groupOrder, setGroupOrder] = useState<string[]>(loadGroupOrder);
  const [colors, setColors] = useState(loadGroupColors);

  const groups = useMemo(
    () =>
      buildSessionGroups(workspaces, {
        idleDecayWindowMs,
        sortMode,
        isCollapsed,
        workspaceOrdering,
        groupOrder,
        colors,
      }),
    [workspaces, idleDecayWindowMs, sortMode, isCollapsed, workspaceOrdering, groupOrder, colors],
  );

  const reorderGroups = useCallback((orderedGroupIds: string[]) => {
    setGroupOrder(orderedGroupIds);
    persistGroupOrder(orderedGroupIds);
  }, []);

  const setGroupColor = useCallback((groupId: string, color: GroupColor | null) => {
    setColors((prev) => {
      const next = withGroupColor(prev, groupId, color);
      persistGroupColors(next);
      return next;
    });
  }, []);

  const carryGroupPrefs = useCallback((fromId: string, toId: string) => {
    setGroupOrder((prev) => {
      const next = prev.map((id) => (id === fromId ? toId : id));
      persistGroupOrder(next);
      return next;
    });
    setColors((prev) => {
      const next = withGroupColor(withGroupColor(prev, toId, prev[fromId] ?? prev[toId] ?? null), fromId, null);
      persistGroupColors(next);
      return next;
    });
  }, []);

  return { groups, toggleGroupCollapsed: toggle, reorderGroups, setGroupColor, carryGroupPrefs };
}

export function useCoordinatorGroup(workspaces: Workspace[]): SidebarGroup | null {
  const idleDecayWindowMs = useIdleDecayWindowMs();
  return useMemo(() => buildCoordinatorGroup(workspaces, idleDecayWindowMs), [workspaces, idleDecayWindowMs]);
}
