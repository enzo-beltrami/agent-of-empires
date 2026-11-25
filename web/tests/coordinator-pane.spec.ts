// The coordinator's Sessions / Deploys / Notes panel is a dockable pane offered only on the coordinator session.

import { test, expect } from "./helpers/mockedTest";
import { installSidebarMocks } from "./helpers/sidebarMocks";

test("the coordinator pane is a header toggle on the coordinator session only", async ({ page }) => {
  await installSidebarMocks(page, {
    sessions: [
      {
        id: "s-coord",
        title: "Coordinator",
        project_path: "/home/me/.config/agent-of-empires/coordinator",
        branch: null,
      },
      { id: "s-work", title: "work", project_path: "/tmp/project", branch: "feat/work" },
    ],
  });
  await page.route("**/api/coordinator/**", (route) =>
    route.fulfill({
      json: route.request().url().endsWith("/summary") ? { wip: [], needs_action: [], idle: [] } : [],
    }),
  );
  await page.setViewportSize({ width: 1440, height: 900 });

  await page.goto("/session/s-work");
  await expect(page.getByTestId("activity-bar")).toBeVisible();
  await expect(page.getByTestId("pane-toggle-coordinator")).toHaveCount(0);

  await page.goto("/session/s-coord");
  const toggle = page.getByTestId("pane-toggle-coordinator");
  const panel = page.getByTestId("coordinator-panel");
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(panel).toHaveCount(0);
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByRole("tab", { name: /Sessions/ })).toBeVisible();
  await toggle.click();
  await expect(panel).toHaveCount(0);
});
