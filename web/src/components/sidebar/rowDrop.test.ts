import { describe, expect, it } from "vitest";
import type { SidebarGroup } from "../../lib/sidebarGroups";
import type { Workspace } from "../../lib/types";
import { rowDrop } from "./rowDrop";

const group = (path: string, ids: string[]): SidebarGroup =>
  ({
    id: path,
    groupPath: path,
    workspaces: ids.map((id) => ({ key: `${path}::${id}`, workspace: { id } as Workspace })),
  }) as SidebarGroup;

describe("rowDrop", () => {
  const groups = [group("payto", ["a", "b", "c"]), group("releases", ["d", "e"])];

  it.each<[string, string, string, string[], string | null]>([
    ["moves down within its Group", "payto::a", "payto::c", ["b", "c", "a", "d", "e"], null],
    ["moves up within its Group", "releases::e", "releases::d", ["a", "b", "c", "e", "d"], null],
    [
      "lands at the target row's place in another Group",
      "payto::b",
      "releases::e",
      ["a", "c", "d", "b", "e"],
      "releases",
    ],
  ])("%s", (_name, active, over, order, target) => {
    const drop = rowDrop(groups, active, over)!;
    expect(drop.order).toEqual(order);
    expect(drop.target?.groupPath ?? null).toBe(target);
    expect(drop.moved.id).toBe(active.split("::")[1]);
  });

  it("keeps a workspace split across Groups at its first place, and ignores unknown rows", () => {
    const split = [group("payto", ["a", "w"]), group("releases", ["w", "d"])];
    expect(rowDrop(split, "releases::d", "payto::a")!.order).toEqual(["d", "a", "w"]);
    expect(rowDrop(groups, "payto::zz", "payto::a")).toBeNull();
  });
});
