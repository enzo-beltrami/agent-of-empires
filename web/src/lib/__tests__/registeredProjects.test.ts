// @vitest-environment node

import { expect, it } from "vitest";

import { normalizeProjectPathKey, savedProjects } from "../registeredProjects";
import type { ProjectInfo } from "../types";

function project(path: string, over: Partial<ProjectInfo> = {}): ProjectInfo {
  return { name: path.split("/").pop() ?? path, path, scope: "global", pinned: false, ...over };
}

it("normalizeProjectPathKey trims and strips trailing slashes without lowercasing", () => {
  expect(["/work/foo/", "/work/foo", " /work/Foo "].map(normalizeProjectPathKey)).toEqual([
    "/work/foo",
    "/work/foo",
    "/work/Foo",
  ]);
});

it("savedProjects lists one entry per path across scopes, by name, skipping blank paths", () => {
  const listed = savedProjects([
    project("/work/zeta"),
    project("/work/alpha"),
    project("/work/alpha/", { scope: "profile" }),
    project("   "),
  ]);
  expect(listed.map((p) => [p.displayName, p.path, p.registrations.map((r) => r.scope)])).toEqual([
    ["alpha", "/work/alpha", ["global", "profile"]],
    ["zeta", "/work/zeta", ["global"]],
  ]);
});
