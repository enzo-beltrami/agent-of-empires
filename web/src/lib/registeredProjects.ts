import type { ProjectInfo, SavedProject } from "./types";

// Trims trailing slashes without lowercasing, which would fold distinct repos on case-sensitive filesystems.
export function normalizeProjectPathKey(path: string): string {
  return path.trim().replace(/[\\/]+$/g, "");
}

/** The registered project's per-project overrides for `path`, if it is registered. */
export function overridesForPath(projects: ProjectInfo[], path: string): ProjectInfo["overrides"] {
  const key = normalizeProjectPathKey(path);
  return projects.find((p) => normalizeProjectPathKey(p.path) === key)?.overrides;
}

/** The registered Projects the sidebar lists, one per path across scopes, by name. */
export function savedProjects(projects: readonly ProjectInfo[]): SavedProject[] {
  const byKey = new Map<string, ProjectInfo[]>();
  for (const project of projects) {
    const key = normalizeProjectPathKey(project.path);
    if (!key) continue;
    const list = byKey.get(key);
    if (list) list.push(project);
    else byKey.set(key, [project]);
  }
  return [...byKey.values()]
    .map((registrations) => {
      const path = registrations[0]!.path;
      return { path, displayName: path.split("/").pop() || path, registrations };
    })
    .sort((a, b) => a.displayName.localeCompare(b.displayName));
}
