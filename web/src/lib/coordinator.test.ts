import { describe, expect, it } from "vitest";
import { deployGroups, deployLabel, isCoordinatorPath, ticketLabel } from "./coordinator";

describe("isCoordinatorPath", () => {
  it.each([
    ["/home/u/.config/agent-of-empires/coordinator", true],
    ["/home/u/.config/agent-of-empires-dev/coordinator/", true],
    ["/Users/u/.agent-of-empires/coordinator", true],
    ["/home/u/code/coordinator", false],
    ["/home/u/.config/agent-of-empires/coordinator/sub", false],
    [undefined, false],
  ])("%s -> %s", (path, expected) => {
    expect(isCoordinatorPath(path)).toBe(expected);
  });
});

describe("deployLabel", () => {
  it.each([
    [{ url: "https://github.com/o/streambox/pull/42" }, "streambox#42"],
    [{ url: "https://gitlab.com/g/api/-/merge_requests/7" }, "api#7"],
    [{ url: "https://example.com/x" }, "https://example.com/x"],
    [{ url: "https://github.com/o/r/pull/1", title: "Fix login" }, "Fix login"],
  ])("%o -> %s", (deploy, expected) => {
    expect(deployLabel(deploy)).toBe(expected);
  });
});

describe("ticketLabel", () => {
  it.each([
    ["PI-1921", { text: "PI-1921" }],
    [
      "https://acme.atlassian.net/browse/PI-1921?focus=1",
      { text: "PI-1921", href: "https://acme.atlassian.net/browse/PI-1921?focus=1" },
    ],
  ])("%s", (ticket, expected) => {
    expect(ticketLabel(ticket)).toEqual(expected);
  });
});

describe("deployGroups", () => {
  const d = (id: string, added_at: string, deployed_at?: string) => ({ id, url: `u${id}`, added_at, deployed_at });
  it("puts pending first, then deployed by local day, newest first", () => {
    const now = new Date(2026, 10, 11, 15, 0);
    const local = (day: number, hour: number) => new Date(2026, 10, day, hour).toISOString();
    const groups = deployGroups(
      [
        d("old", local(1, 9), local(9, 10)),
        d("p1", local(10, 9)),
        d("y", local(9, 9), local(10, 12)),
        d("t1", local(10, 9), local(11, 9)),
        d("p2", local(11, 9)),
        d("t2", local(10, 9), local(11, 14)),
      ],
      now,
    );
    expect(
      groups.map((g) => [g.label.startsWith("Deployed Mon") ? "older" : g.label, g.deploys.map((x) => x.id)]),
    ).toEqual([
      ["Pending", ["p2", "p1"]],
      ["Deployed today", ["t2", "t1"]],
      ["Deployed yesterday", ["y"]],
      ["older", ["old"]],
    ]);
  });
});
