import { safeGetItem, safeSetItem } from "./safeStorage";

/** Sidebar grouping axis, persisted per browser. */
export type SidebarAxis = "group" | "status";

export const SIDEBAR_AXIS_KEY = "aoe-sidebar-axis";

export function loadSidebarAxis(): SidebarAxis {
  return safeGetItem(SIDEBAR_AXIS_KEY) === "status" ? "status" : "group";
}

export function saveSidebarAxis(axis: SidebarAxis): void {
  safeSetItem(SIDEBAR_AXIS_KEY, axis);
}
