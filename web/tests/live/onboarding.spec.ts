// First-run surface: the tutorial.

import type { Page } from "@playwright/test";
import { test, expect } from "../helpers/liveTest";

/** Auto-launching onboarding is suppressed for automated browsers; present as a real one. */
const presentAsRealBrowser = (page: Page) =>
  page.addInitScript(() => Object.defineProperty(navigator, "webdriver", { get: () => false }));

type ApiJson = {
  app_state?: { has_seen_web_tour?: boolean };
} | null;
const fetchJson = (page: Page, path: string): Promise<ApiJson> =>
  page.evaluate(async (p) => {
    const res = await fetch(p, { cache: "no-store" });
    return res.ok ? res.json() : null;
  }, path);

const tourSeen = async (page: Page) => (await fetchJson(page, "/api/settings"))?.app_state?.has_seen_web_tour === true;
/** The intro folds the Daemon holds in synced web UI state. */
const storedIntros = (page: Page) =>
  page.evaluate(async () => {
    const res = await fetch("/api/app-state/web-ui-state", { cache: "no-store" });
    const state: Record<string, string> = res.ok ? await res.json() : {};
    return Object.fromEntries(Object.entries(state).filter(([k]) => k.startsWith("aoe-settings-intro-")));
  });

async function openTopBarMenuItem(page: Page, name: string) {
  await page.getByRole("button", { name: "More options" }).click();
  await page.getByRole("menuitem", { name }).click();
}

test("first-run tutorial: auto-launch, skip, persist, re-trigger, and the Settings intros it resets", async ({
  page,
  serve,
}) => {
  // #1513; the seen flag is server-side since #1832.
  const tourStep = (title: string) => page.getByRole("alertdialog", { name: title, exact: true });
  const firstStep = tourStep("Search and commands");
  await presentAsRealBrowser(page);

  await page.goto(serve.baseUrl);
  // #1834: the theme welcome shows first.
  await expect(page.getByRole("heading", { name: "Welcome to Agent of Empires" })).toBeVisible({ timeout: 10_000 });
  await page.getByRole("button", { name: "Continue" }).click();

  await expect(firstStep).toBeVisible({ timeout: 10_000 });
  const skip = page.getByRole("button", { name: "Skip tour", exact: true });
  await expect(skip).toBeVisible();
  const postSeen = page.waitForResponse(
    (r) => r.url().includes("/api/app-state/web-tour-seen") && r.request().method() === "POST",
    { timeout: 10_000 },
  );
  await skip.click();
  expect((await postSeen).status()).toBe(200);
  await expect(firstStep).toBeHidden();
  // #2819: a stranded scrim blocks the whole page.
  await expect(page.locator(".react-joyride__overlay")).toHaveCount(0);
  await expect.poll(() => tourSeen(page), { timeout: 20_000 }).toBe(true);

  await page.reload();
  await expect(page.getByRole("button", { name: "Go to dashboard" })).toBeVisible();
  await expect(firstStep).toBeHidden();
  await expect(page.getByRole("heading", { name: "Welcome to Agent of Empires" })).toBeHidden();

  // #4594: a Settings tab intro folds into "About" and stays folded across a reload.
  const intro = page.getByRole("region", { name: "About this tab" });
  const about = page.getByRole("button", { name: "About this tab" });
  await page.goto(`${serve.baseUrl}/settings/worktree`);
  await expect(intro).toBeVisible({ timeout: 10_000 });
  const details = intro.getByRole("button", { name: "Details" });
  await details.focus();
  await page.keyboard.press("Enter");
  await expect(details).toHaveAttribute("aria-expanded", "true");
  await expect(intro.getByText(/path template expands/)).toBeVisible();
  await intro.getByRole("button", { name: "Got it" }).click();
  await expect(about).toBeVisible();
  await expect.poll(() => storedIntros(page)).toEqual({ "aoe-settings-intro-worktree": "folded" });
  // Drop this browser's copy so only the Daemon's can fold it again, as on another Device.
  await page.evaluate(() => localStorage.removeItem("aoe-settings-intro-worktree"));
  await page.reload();
  await expect(about).toBeVisible({ timeout: 10_000 });
  await expect(intro).toBeHidden();

  await page.goto(serve.baseUrl);
  await openTopBarMenuItem(page, "Show tutorial");
  await expect(firstStep).toBeVisible({ timeout: 10_000 });
  await expect
    .poll(() => storedIntros(page))
    .toEqual({
      "aoe-settings-intro-worktree": "unfolded",
      "aoe-settings-intro-plugins": "unfolded",
      "aoe-settings-intro-structured-view": "unfolded",
    });

  // #4594: the tour stays on the dashboard.
  const next = page.getByRole("button", { name: /^Next/ });
  for (const heading of ["Your sessions", "Start a session", "Settings", "Replay this tour any time"]) {
    await next.click();
    await expect(tourStep(heading)).toBeVisible({ timeout: 10_000 });
    expect(new URL(page.url()).pathname).toBe("/");
  }
  await page.getByRole("button", { name: /^Done/ }).click();
  await expect(tourStep("Replay this tour any time")).toBeHidden();
  // #2819: the last step must end the tour itself.
  await expect(page.locator(".react-joyride__overlay")).toHaveCount(0);
  expect(await tourSeen(page)).toBe(true);

  // Replaying unfolded the intro; it reads at 375px without sideways scroll.
  await page.setViewportSize({ width: 375, height: 800 });
  await page.goto(`${serve.baseUrl}/settings/worktree`);
  await expect(intro).toBeVisible({ timeout: 10_000 });
  const box = await intro.boundingBox();
  expect(box && box.x >= 0 && box.x + box.width <= 375).toBe(true);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(375);
});

test("a read-only dashboard keeps a Settings intro fold on the device across a reload", async ({
  serveReadOnly,
  page,
}) => {
  const intro = page.getByRole("region", { name: "About this tab" });
  const about = page.getByRole("button", { name: "About this tab" });
  await page.goto(`${serveReadOnly.baseUrl}/settings/worktree`);
  await expect(intro).toBeVisible({ timeout: 10_000 });
  await intro.getByRole("button", { name: "Got it" }).click();
  await expect(about).toBeVisible();
  // The Daemon refuses the write, so the fold lives only in this browser's device key.
  expect(await storedIntros(page)).toEqual({});
  expect(await page.evaluate(() => localStorage.getItem("aoe-local-settings-intro-worktree"))).toBe("folded");
  await page.reload();
  await expect(about).toBeVisible({ timeout: 10_000 });
  await expect(intro).toBeHidden();
});
