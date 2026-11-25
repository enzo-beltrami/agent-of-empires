// @vitest-environment jsdom

import { beforeEach, expect, it } from "vitest";

import { loadGroupColors, persistGroupColors, withGroupColor } from "./groupColor";

const STORAGE_KEY = "aoe-group-color-v1";

beforeEach(() => {
  window.localStorage.clear();
});

it("withGroupColor sets, clears, and ignores unknown colors without mutating its input", () => {
  const current = { payto: "amber" } as const;
  const snapshot = JSON.stringify(current);
  expect(withGroupColor(current, "releases", "teal")).toEqual({ payto: "amber", releases: "teal" });
  expect(withGroupColor(current, "payto", null)).toEqual({});
  expect(withGroupColor(current, "payto", "bogus" as never)).toEqual({});
  expect(JSON.stringify(current)).toBe(snapshot);
});

it("round-trips colors, drops unknown ones, and removes the key when empty", () => {
  persistGroupColors({ payto: "amber", releases: "violet" });
  expect(loadGroupColors()).toEqual({ payto: "amber", releases: "violet" });
  window.localStorage.setItem(STORAGE_KEY, JSON.stringify({ payto: "amber", releases: "rainbow" }));
  expect(loadGroupColors()).toEqual({ payto: "amber" });
  window.localStorage.setItem(STORAGE_KEY, "{not json");
  expect(loadGroupColors()).toEqual({});
  persistGroupColors({});
  expect(window.localStorage.getItem(STORAGE_KEY)).toBeNull();
});
