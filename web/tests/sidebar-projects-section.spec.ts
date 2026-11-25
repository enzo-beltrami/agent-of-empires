import { test, expect } from "./helpers/mockedTest";
import { Page } from "@playwright/test";

// The sidebar's Projects section lists every registered Project, whether or not it has a Session or was
// pinned by an older build, since Groups replace repository headers.

interface MockSession {
  id: string;
  title: string;
  project_path: string;
}

interface MockProject {
  name: string;
  path: string;
  scope: "global" | "profile";
  pinned: boolean;
}

async function mockApis(page: Page, sessions: MockSession[], projects: MockProject[]) {
  await page.route("**/api/login/status", (r) => r.fulfill({ json: { required: false, authenticated: true } }));
  await page.route(/\/api\/sessions(\?.*)?$/, (r) => {
    if (r.request().method() !== "GET") return r.fulfill({ status: 400 });
    return r.fulfill({
      headers: { "aoe-runtime-epoch": "mock-runtime", "aoe-runtime-revision": "1" },
      json: {
        sessions: sessions.map((s) => ({
          id: s.id,
          title: s.title,
          project_path: s.project_path,
          group_path: s.project_path,
          tool: "claude",
          status: "Idle",
          yolo_mode: false,
          created_at: new Date().toISOString(),
          last_accessed_at: null,
          last_error: null,
          branch: null,
          main_repo_path: null,
          is_sandboxed: false,
          has_terminal: true,
          profile: "default",
          workspace_repos: [],
        })),
        workspace_ordering: [],
      },
    });
  });
  await page.route("**/api/projects*", (r) => {
    const request = r.request();
    if (request.method() === "GET") {
      if (new URL(request.url()).searchParams.get("profile") !== "default") return r.fulfill({ status: 400 });
      return r.fulfill({ json: projects });
    }
    if (request.method() === "POST") {
      const body = request.postDataJSON() as { path: string; pinned?: boolean; profile: string; scope: string };
      if (body.profile !== "default" || body.scope !== "global") return r.fulfill({ status: 400 });
      if (projects.some((project) => project.path === body.path)) return r.fulfill({ status: 409 });
      const project: MockProject = {
        name: body.path.split("/").pop()!,
        path: body.path,
        scope: "global",
        pinned: body.pinned ?? false,
      };
      projects.push(project);
      return r.fulfill({ status: 201, json: project });
    }
    return r.fulfill({ status: 400 });
  });
  await page.route("**/api/projects/*", (r) => {
    if (r.request().method() !== "PATCH") return r.fulfill({ status: 400 });
    const url = new URL(r.request().url());
    const path = decodeURIComponent(url.pathname.slice("/api/projects/".length));
    const project = projects.find(
      (project) => project.path === path && project.scope === url.searchParams.get("scope"),
    );
    if (!project) return r.fulfill({ status: 404 });
    project.pinned = r.request().postDataJSON().pinned;
    return r.fulfill({ json: project });
  });
  for (const path of ["settings", "themes", "agents", "profiles", "groups", "devices", "docker/status", "about"]) {
    await page.route(`**/api/${path}`, (r) =>
      r.fulfill({
        json:
          path === "about"
            ? { profile: "default" }
            : path === "profiles"
              ? [{ name: "default", is_default: true }]
              : path === "docker/status"
                ? {}
                : [],
      }),
    );
  }
}

test("the Projects section lists every registered project, with or without sessions", async ({ page }) => {
  await mockApis(
    page,
    [{ id: "s-1", title: "Mongols", project_path: "/tmp/repo-a" }],
    [
      { name: "repo-a", path: "/tmp/repo-a", scope: "global", pinned: false },
      { name: "repo-b", path: "/tmp/repo-b", scope: "global", pinned: true },
    ],
  );
  await page.goto("/");
  const toggle = page.getByTestId("sidebar-projects-toggle");
  await expect(toggle).toContainText("Projects");
  await expect(toggle).toContainText("(2)");
  if ((await toggle.getAttribute("aria-expanded")) === "false") await toggle.click();
  const rows = page.getByTestId("sidebar-project-row");
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText("repo-a");
  await expect(rows.nth(1)).toContainText("repo-b");
});
