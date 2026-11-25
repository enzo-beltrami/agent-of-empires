import type { CSSProperties } from "react";
import { safeGetItem, safeRemoveItem, safeSetItem } from "./safeStorage";

// A Group's header tint, per browser, keyed by Group id.
const STORAGE_KEY = "aoe-group-color-v1";

export type GroupColor = "amber" | "teal" | "sky" | "violet" | "rose" | "slate";

export const GROUP_COLOR_OPTIONS: Array<{ id: GroupColor; label: string }> = [
  { id: "amber", label: "Amber" },
  { id: "teal", label: "Teal" },
  { id: "sky", label: "Sky" },
  { id: "violet", label: "Violet" },
  { id: "rose", label: "Rose" },
  { id: "slate", label: "Slate" },
];

// Fixed hues: these are user labels, and theme tokens can share one value.
const GROUP_COLOR_HEX: Record<GroupColor, string> = {
  amber: "#f59e0b",
  teal: "#14b8a6",
  sky: "#0ea5e9",
  violet: "#8b5cf6",
  rose: "#f43f5e",
  slate: "#64748b",
};

// Faint tinted background for a Group header carrying a color.
export function groupColorStyle(color: GroupColor | null): CSSProperties | undefined {
  if (!color) return undefined;
  return { backgroundColor: `color-mix(in srgb, ${GROUP_COLOR_HEX[color]} 14%, transparent)` };
}

// Solid swatch for the color picker.
export function groupSwatchStyle(color: GroupColor): CSSProperties {
  return { backgroundColor: GROUP_COLOR_HEX[color] };
}

const validColors = new Set<string>(GROUP_COLOR_OPTIONS.map((option) => option.id));

export function loadGroupColors(): Record<string, GroupColor> {
  const raw = safeGetItem(STORAGE_KEY);
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return {};
    return Object.fromEntries(
      Object.entries(parsed).filter((entry): entry is [string, GroupColor] => validColors.has(entry[1] as string)),
    );
  } catch {
    return {};
  }
}

/** `color` null clears the entry; the input map is not mutated. */
export function withGroupColor(
  current: Record<string, GroupColor>,
  groupId: string,
  color: GroupColor | null,
): Record<string, GroupColor> {
  const next = { ...current };
  if (color && validColors.has(color)) next[groupId] = color;
  else delete next[groupId];
  return next;
}

export function persistGroupColors(map: Record<string, GroupColor>): void {
  if (Object.keys(map).length === 0) {
    safeRemoveItem(STORAGE_KEY);
    return;
  }
  safeSetItem(STORAGE_KEY, JSON.stringify(map));
}
