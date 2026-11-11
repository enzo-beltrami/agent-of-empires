import { describe, expect, it } from "vitest";
import { isCoordinatorPath } from "./coordinator";

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
