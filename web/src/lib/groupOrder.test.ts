// @vitest-environment jsdom

import { beforeEach, expect, it } from "vitest";

import { loadGroupOrder, persistGroupOrder } from "./groupOrder";

const ORDER_KEY = "aoe-group-order-v1";

beforeEach(() => {
  window.localStorage.clear();
});

it("round-trips an order and removes the key when empty", () => {
  expect(loadGroupOrder()).toEqual([]);
  persistGroupOrder(["releases", "payto"]);
  expect(window.localStorage.getItem(ORDER_KEY)).toBe(JSON.stringify(["releases", "payto"]));
  expect(loadGroupOrder()).toEqual(["releases", "payto"]);
  persistGroupOrder([]);
  expect(window.localStorage.getItem(ORDER_KEY)).toBeNull();
});

it("loads only the string entries of a stored array", () => {
  const cases: [string, string[]][] = [
    ["{not json", []],
    [JSON.stringify({ a: 1 }), []],
    [JSON.stringify(["releases", 42, null, "payto"]), ["releases", "payto"]],
  ];
  for (const [stored, expected] of cases) {
    window.localStorage.setItem(ORDER_KEY, stored);
    expect(loadGroupOrder(), stored).toEqual(expected);
  }
});
