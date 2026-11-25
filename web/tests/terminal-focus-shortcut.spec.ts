import { test, expect, observeFor } from "./helpers/mockedTest";
import type { Page } from "@playwright/test";
import { clickSidebarSession } from "./helpers/sidebar";
import { mockTerminalApis } from "./helpers/terminal-mocks";

async function openSession(page: Page) {
  await clickSidebarSession(page, "pinch-test");
  await expect(page.locator('[data-term="agent"]')).toHaveCount(1);
  await expect(
    page.locator('[data-term="agent"] .xterm, [data-term="agent"] [data-live-terminal]').first(),
  ).toBeVisible();
}

async function focusedKind(page: Page): Promise<"agent" | "paired" | null> {
  return page.evaluate(() => {
    const active = document.activeElement;
    if (!active) return null;
    if (document.querySelector('[data-term="agent"]')?.contains(active)) {
      return "agent";
    }
    const paired = document.querySelectorAll('[data-term="paired"]');
    for (const p of paired) {
      if (p.contains(active)) return "paired";
    }
    return null;
  });
}

async function focusAgent(page: Page) {
  await page.locator('[data-term="agent"]').first().locator("textarea").focus();
}

async function blurAll(page: Page) {
  await page.evaluate(() => {
    const a = document.activeElement as HTMLElement | null;
    a?.blur?.();
    document.body.focus();
  });
}

// ────────────────────────────────────────────────────────────────────
//  Desktop scenarios
// ────────────────────────────────────────────────────────────────────
test.describe("Cmd/Ctrl+` desktop", () => {
  test.use({ viewport: { width: 1280, height: 800 }, hasTouch: false });

  test("focuses the agent terminal from anywhere and never opens a paired shell", async ({ page }) => {
    await mockTerminalApis(page);
    await page.goto("/");
    await openSession(page);
    await blurAll(page);
    await expect.poll(() => focusedKind(page)).toBe(null);

    // Desktop has no paired shell, so every press lands on the agent terminal.
    for (let i = 0; i < 3; i++) {
      await page.keyboard.press("ControlOrMeta+`");
      await expect.poll(() => focusedKind(page)).toBe("agent");
    }
    await expect(page.locator('[data-term="paired"]')).toHaveCount(0);
  });

  test("Ctrl+Q returns focus from the terminal to the active sidebar row", async ({ page }) => {
    await mockTerminalApis(page);
    await page.goto("/");
    await openSession(page);
    await focusAgent(page);

    const sidebar = page.getByRole("navigation", { name: "Sessions sidebar" });
    const activeRow = sidebar.locator("[aria-current=page]");
    await page.keyboard.press("Control+q");
    await expect(activeRow).toBeFocused();

    await activeRow.evaluate((row) => ((row as HTMLElement).style.display = "none"));
    await focusAgent(page);
    await page.keyboard.press("Control+q");
    await expect(sidebar).toBeFocused();
  });
  // (The xterm-only `term-focused` panel CSS ring was removed with the xterm
  // renderer; focus correctness is covered by the focusedKind() assertions
  // above.)
});

// ────────────────────────────────────────────────────────────────────
//  Mobile scenario — proves the data-term fix
// ────────────────────────────────────────────────────────────────────
test.describe("Cmd/Ctrl+` mobile", () => {
  test.use({ viewport: { width: 390, height: 844 }, hasTouch: true });

  test("toggle works correctly on a mobile viewport", async ({ page }) => {
    await mockTerminalApis(page);
    await page.goto("/");
    await page.getByRole("button", { name: "Toggle sidebar" }).click();
    await openSession(page);

    // #1452: one full-viewport pane; the chord promotes a single paired instance.
    await page.keyboard.press("ControlOrMeta+`");
    await expect(page.locator('[data-term="paired"]')).toHaveCount(1);
    await expect.poll(() => focusedKind(page)).toBe("paired");

    await page.keyboard.press("ControlOrMeta+`");
    await expect.poll(() => focusedKind(page)).toBe("agent");

    await page.keyboard.press("ControlOrMeta+`");
    await expect.poll(() => focusedKind(page)).toBe("paired");
  });
  test("Ctrl+Q returns from the mobile keyboard proxy without sending a terminal control byte", async ({ page }) => {
    const terminal = await mockTerminalApis(page);
    await page.goto("/");
    await page.getByRole("button", { name: "Toggle sidebar" }).click();
    await openSession(page);
    await terminal.waitForLiveReady();

    const proxy = page.locator("[data-keyboard-proxy]");
    await expect(proxy).toHaveAttribute("data-session-input", "");
    await proxy.focus();
    await expect(proxy).toBeFocused();
    await page.keyboard.press("Control+a");
    await expect.poll(() => terminal.liveInput.some((input) => input.equals(Buffer.from([0x01])))).toBe(true);

    await page.keyboard.press("Control+q");
    const sidebar = page.getByRole("navigation", { name: "Sessions sidebar" });
    await expect(sidebar.locator("[data-active-session-row]")).toBeFocused();
    await observeFor(page, 250, async () => {
      expect(terminal.liveInput.some((input) => input.includes(0x11))).toBe(false);
    });
  });
});
