import { describe, expect, it } from "vitest";
import { isAssistantPath } from "./assistant";

describe("isAssistantPath", () => {
  it.each([
    ["/home/u/.config/agent-of-empires/assistant", true],
    ["/home/u/.config/agent-of-empires-dev/assistant/", true],
    ["/Users/u/.agent-of-empires/assistant", true],
    ["/home/u/code/assistant", false],
    ["/home/u/.config/agent-of-empires/assistant/sub", false],
    [undefined, false],
  ])("%s -> %s", (path, expected) => {
    expect(isAssistantPath(path)).toBe(expected);
  });
});
