import { safeGetItem, safeRemoveItem, safeSetItem } from "./safeStorage";

// Manual order of the sidebar's Groups, per browser: the server orders sessions, not Groups.
const STORAGE_KEY = "aoe-group-order-v1";

export function loadGroupOrder(): string[] {
  const raw = safeGetItem(STORAGE_KEY);
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((id): id is string => typeof id === "string");
  } catch {
    return [];
  }
}

export function persistGroupOrder(order: readonly string[]): void {
  if (order.length === 0) {
    safeRemoveItem(STORAGE_KEY);
    return;
  }
  safeSetItem(STORAGE_KEY, JSON.stringify(order));
}
