// Keyboard shortcuts through useKeyboardShortcuts: Cmd/Ctrl+B toggles the
// sidebar and Cmd/Ctrl+Alt+B the Coordinator panel. The chords bind on
// `e.code === "KeyB"` so Mac layouts emitting "∫" still match.

import { test, expect } from "./helpers/mockedTest";
import { installSidebarMocks, threeSessionsInOneRepo } from "./helpers/sidebarMocks";
import { mockTerminalApis } from "./helpers/terminal-mocks";

const SESSION = "pinch-test";

test("Cmd/Ctrl+B toggles the workspace sidebar", async ({ page }) => {
  await installSidebarMocks(page, { sessions: threeSessionsInOneRepo() });
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto("/");

  const sessionRow = page.locator('[data-testid="sidebar-session-row"]').first();
  await expect(sessionRow).toBeVisible();

  // Global chord: fires regardless of focus.
  await page.keyboard.press("ControlOrMeta+b");
  await expect(sessionRow).toBeHidden();

  await page.keyboard.press("ControlOrMeta+b");
  await expect(sessionRow).toBeVisible();
});

test("Cmd/Ctrl+Alt+B toggles the Coordinator panel on a session view", async ({ page }) => {
  await mockTerminalApis(page);
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto(`/session/${SESSION}`);

  const panel = page.getByTestId("coordinator-panel");
  const toggle = page.getByTestId("coordinator-panel-toggle");
  await expect(panel).toBeVisible();

  // Global chord: fires with the terminal focused too.
  await page.keyboard.press("ControlOrMeta+Alt+b");
  await expect(panel).toHaveCount(0);
  await expect(toggle).toHaveAttribute("aria-pressed", "false");

  await page.keyboard.press("ControlOrMeta+Alt+b");
  await expect(panel).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
});
