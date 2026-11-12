// #4627: the document capture shortcut must not unmount Settings before a
// dialog's own Escape handler can run in the browser.
import type { Page } from "@playwright/test";
import { test, expect, publishedRequests } from "./helpers/mockedTest";
import { mockSettingsApis, mockStaticApis } from "./helpers/apiMocks";

const WEB = {
  id: "aoe.web",
  name: "Web dashboard",
  version: "1.0.0",
  description: "The aoe serve web dashboard.",
  enabled: true,
};

test.beforeEach(async ({ page }) => {
  await mockStaticApis(page);
  await mockSettingsApis(page);
  await page.route("**/api/plugins", (route) => route.fulfill({ json: { plugins: [WEB] } }));
  await page.goto("/settings/plugins");
  await expect(page.getByTestId("plugin-aoe.web")).toBeVisible();
});

const openWebOff = async (page: Page) => {
  await page.getByRole("switch", { name: "Enable Web dashboard", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Turn off the Web dashboard?", exact: true });
  await expect(dialog).toBeVisible();
  return dialog;
};
const noToggle = async (page: Page) =>
  expect(await publishedRequests(page, "/api/plugins/aoe.web/enabled", "POST")).toEqual([]);

test("Escape cancels disabling the Web dashboard and keeps it enabled", async ({ page }) => {
  const toggle = page.getByRole("switch", { name: "Enable Web dashboard", exact: true });
  await toggle.click();
  const dialog = page.getByRole("dialog", { name: "Turn off the Web dashboard?", exact: true });
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId("web-dashboard-off-confirm")).toBeFocused();

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(page).toHaveURL(/\/settings\/plugins$/);
  await expect(toggle).toBeChecked();
  expect(await publishedRequests(page, "/api/plugins/aoe.web/enabled", "POST")).toEqual([]);
});

test("Escape still closes the global About dialog without leaving Settings", async ({ page }) => {
  await page.getByRole("button", { name: "More options", exact: true }).click();
  await page.getByRole("menuitem", { name: "About", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Agent of Empires", exact: true });
  await expect(dialog).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(page).toHaveURL(/\/settings\/plugins$/);
});

test("Escape closes the palette before the underlying confirmation", async ({ page }) => {
  const confirm = await openWebOff(page);
  await page.keyboard.press("ControlOrMeta+k");
  const palette = page.getByRole("dialog", { name: "Command palette", exact: true });
  await expect(palette).toBeVisible();
  await expect(palette.getByRole("combobox")).toBeFocused();

  await page.keyboard.press("Escape");
  await expect(palette).toBeHidden();
  await expect(confirm).toBeVisible();
  await expect(page).toHaveURL(/\/settings\/plugins$/);

  await page.keyboard.press("Escape");
  await expect(confirm).toBeHidden();
  await expect(page).toHaveURL(/\/settings\/plugins$/);
  await noToggle(page);

  await page.keyboard.press("Escape");
  await expect(page).toHaveURL(/\/$/);
});

test("About above a confirmation keeps Tab focus inside and first Escape closes only About", async ({ page }) => {
  const confirm = await openWebOff(page);
  await page.keyboard.press("ControlOrMeta+k");
  const palette = page.getByRole("dialog", { name: "Command palette", exact: true });
  await expect(palette.getByRole("combobox")).toBeFocused();
  await palette.getByRole("combobox").fill("About");
  await page.keyboard.press("Enter");
  const about = page.getByRole("dialog", { name: "Agent of Empires", exact: true });
  await expect(about).toBeVisible();
  const close = about.getByRole("button", { name: "Close", exact: true });
  const feedback = about.getByRole("link", { name: "Send feedback", exact: true });
  await expect(close).toBeFocused();

  await page.keyboard.press("Shift+Tab");
  await expect(feedback).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(close).toBeFocused();

  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Escape");
  await expect(about).toBeHidden();
  await expect(confirm).toBeVisible();
  await expect(page).toHaveURL(/\/settings\/plugins$/);
  await noToggle(page);
});
