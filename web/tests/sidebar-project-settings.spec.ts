import { test, expect } from "./helpers/mockedTest";
import { Page } from "@playwright/test";

// Project settings open from a Projects-section row, and Scratch settings from its Scratch row, since
// the sidebar groups Sessions by Group rather than by repository.

interface MockSession {
  id: string;
  title: string;
  project_path: string;
  scratch?: boolean;
}

interface MockProject {
  name: string;
  path: string;
  scope: "global" | "profile";
  pinned: boolean;
  default_base_branch?: string;
}

async function mockApis(page: Page, sessions: MockSession[], projects: MockProject[]) {
  await page.route("**/api/login/status*", (r) => r.fulfill({ json: { required: false, authenticated: true } }));
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
          scratch: s.scratch ?? false,
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
  // GET lists the registry; POST registers (returns the created project).
  await page.route(/\/api\/projects(?:\?.*)?$/, (r) => {
    const method = r.request().method();
    if (method === "GET") return r.fulfill({ json: projects });
    if (method === "POST") {
      const body = r.request().postDataJSON() as { path: string };
      return r.fulfill({
        status: 201,
        json: { name: body.path.split("/").pop(), path: body.path, scope: "global", pinned: false },
      });
    }
    return r.fulfill({ status: 400 });
  });
  for (const path of ["settings", "themes", "agents", "groups", "devices", "docker/status", "about"]) {
    await page.route(`**/api/${path}`, (r) =>
      r.fulfill({ json: path === "about" ? { profile: "default" } : path === "docker/status" ? {} : [] }),
    );
  }
  await page.route("**/api/profiles", (r) =>
    r.fulfill({ json: [{ name: "default", description: null, is_default: true }] }),
  );
}

test.describe("Sidebar project and scratch settings (#4036)", () => {
  test("Project settings on a registered project's row opens the editor directly, no re-registration", async ({
    page,
  }) => {
    await mockApis(
      page,
      [{ id: "s-1", title: "Mongols", project_path: "/tmp/repo-a" }],
      [{ name: "repo-a", path: "/tmp/repo-a", scope: "global", pinned: false, default_base_branch: "develop" }],
    );
    await page.goto("/");
    await expect(page.locator("header")).toBeVisible();

    const row = page.getByTestId("sidebar-project-row").filter({ hasText: "repo-a" });
    await expect(row).toBeVisible();
    await row.click({ button: "right" });

    let posted = false;
    page.on("request", (req) => {
      if (req.url().endsWith("/api/projects") && req.method() === "POST") posted = true;
    });
    await page.getByTestId("sidebar-project-context-menu-edit").click();

    await expect(page.getByTestId("project-form-modal")).toBeVisible();
    await expect(page.getByText("Edit project 'repo-a'")).toBeVisible();
    expect(posted).toBe(false);
  });

  test("Scratch settings on the Projects section's Scratch row opens its own dialog, not the project form", async ({
    page,
  }) => {
    await mockApis(page, [{ id: "s-1", title: "Scratch chat", project_path: "/tmp/scratch/s-1", scratch: true }], []);
    // Overrides mockApis' generic `settings -> []` fallback: the modal reads `session` off an
    // object, matching the real `GET /api/settings` shape.
    await page.route("**/api/settings", (r) => {
      if (r.request().method() !== "GET") return r.fulfill({ status: 400 });
      return r.fulfill({ json: { session: {} } });
    });
    await page.goto("/");
    await expect(page.locator("header")).toBeVisible();

    const row = page.getByTestId("sidebar-scratch-row");
    await expect(row).toBeVisible();
    await row.click({ button: "right" });
    await page.getByTestId("sidebar-scratch-context-menu-settings").click();

    await expect(page.getByTestId("scratch-overrides-modal")).toBeVisible();
    await expect(page.getByText("Scratch session settings")).toBeVisible();
    await expect(page.getByTestId("project-form-modal")).toHaveCount(0);
  });
});
