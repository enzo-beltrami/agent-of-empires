import { describe, expect, it } from "vitest";
import { deployLabel, isCoordinatorPath } from "./coordinator";

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
