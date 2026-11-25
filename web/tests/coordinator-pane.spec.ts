// The coordinator's Sessions / Deploys / Notes panel sits beside every session on desktop and only collapses or opens.

import { test, expect } from "./helpers/mockedTest";
import { installSidebarMocks } from "./helpers/sidebarMocks";

test("the Coordinator panel beside any session collapses and opens from the header, and stays as left", async ({
  page,
}) => {
  await installSidebarMocks(page, {
    sessions: [{ id: "s-work", title: "work", project_path: "/tmp/project", branch: "feat/work" }],
  });
  await page.route("**/api/coordinator/**", (route) =>
    route.fulfill({
      json: route.request().url().endsWith("/summary")
        ? { wip: [], needs_action: [], idle: [] }
        : route.request().url().endsWith("/decisions")
          ? { session_id: null, decisions: [] }
          : [],
    }),
  );
  await page.setViewportSize({ width: 1440, height: 900 });
  await page.goto("/session/s-work");

  const toggle = page.getByTestId("coordinator-panel-toggle");
  const panel = page.getByTestId("coordinator-panel");
  // No other pane toggles and no tab strip: the panel is the only right-hand surface.
  await expect(page.locator("[data-testid^='pane-toggle-']")).toHaveCount(0);
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(panel.getByRole("tab", { name: /Decisions/ })).toHaveAttribute("aria-selected", "true");

  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(panel).toHaveCount(0);
  await page.reload();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(panel).toHaveCount(0);

  await toggle.click();
  await expect(panel.getByRole("tab", { name: /Deploys/ })).toBeVisible();
});
