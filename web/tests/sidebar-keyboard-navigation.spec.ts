import { test, expect } from "./helpers/mockedTest";
import type { Locator, Page } from "@playwright/test";
import { installSidebarMocks, threeSessionsInOneRepo, type MockSessionInput } from "./helpers/sidebarMocks";

test("arrow keys move focus through the session list and Enter opens the focused session", async ({ page }) => {
  await installSidebarMocks(page, {
    sessions: threeSessionsInOneRepo(),
  });

  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto("/");

  const rows = page.getByTestId("sidebar-session-row");
  await expect(rows).toHaveCount(3);
  await rows.nth(0).focus();
  // Headers are in the list too, so Up from the first row reaches its header.
  await page.keyboard.press("ArrowUp");
  await expect(page.locator("[data-testid='sidebar-group-header'] button[aria-expanded]")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(rows.nth(0)).toBeFocused();

  await page.keyboard.press("ArrowDown");
  await expect(rows.nth(1)).toBeFocused();
  await expect(page).toHaveURL("/");

  await page.keyboard.press("ArrowDown");
  await expect(rows.nth(2)).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(rows.nth(2)).toBeFocused();

  await page.keyboard.press("ArrowUp");
  await expect(rows.nth(1)).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/session\/s-b$/, { timeout: 5_000 });
});

test("arrow keys skip deleting session rows", async ({ page }) => {
  const sessions = threeSessionsInOneRepo().map((session, index) =>
    index === 1 ? { ...session, fields: { status: "Deleting" } } : session,
  );
  await installSidebarMocks(page, { sessions });

  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto("/");

  const rows = page.getByTestId("sidebar-session-row");
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(1)).toHaveAttribute("aria-disabled", "true");
  await rows.nth(0).focus();
  await page.keyboard.press("ArrowDown");
  await expect(rows.nth(2)).toBeFocused();
  await expect(page).toHaveURL("/");
});

// Two Groups of two rows each.
const twoByTwo: MockSessionInput[] = [
  { id: "s-a1", title: "a-one", project_path: "/tmp/repo-a", branch: "a/one", group: "g-a" },
  { id: "s-a2", title: "a-two", project_path: "/tmp/repo-a", branch: "a/two", group: "g-a" },
  { id: "s-b1", title: "b-one", project_path: "/tmp/repo-b", branch: "b/one", group: "g-b" },
  { id: "s-b2", title: "b-two", project_path: "/tmp/repo-b", branch: "b/two", group: "g-b" },
];

for (const compact of [false, true]) {
  test(`headers are one tab stop each and share the arrow keys with rows${compact ? " (compact)" : ""}`, async ({
    page,
  }) => {
    await installSidebarMocks(page, { sessions: twoByTwo });
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.goto("/");
    if (compact) {
      await page.getByRole("button", { name: "Compact sidebar" }).click();
      await expect(page.getByRole("button", { name: "Expand sidebar" })).toBeVisible();
    }
    const headers = page.getByTestId("sidebar-group-header");
    await expect(headers).toHaveCount(2);
    const toggle = (i: number): Locator => headers.nth(i).locator("button[aria-expanded]");
    const press = async (key: string, focused: Locator) => {
      await page.keyboard.press(key);
      await expect(focused, key).toBeFocused();
    };

    // One stop per header: Shift+Tab from its first row goes "+" (absent in compact), then the header button, then out.
    const controlled = await toggle(0).getAttribute("aria-controls");
    const firstRow = page.locator(`[id="${controlled}"] [data-testid="sidebar-session-row"]`).first();
    await expect(firstRow).toBeVisible();
    await firstRow.focus();
    if (!compact) await press("Shift+Tab", headers.nth(0).getByRole("button", { name: /New session in/ }));
    await press("Shift+Tab", toggle(0));
    await page.keyboard.press("Shift+Tab");
    expect(await headers.nth(0).evaluate((h) => h.contains(document.activeElement))).toBe(false);

    // Enter and Space toggle.
    await toggle(0).focus();
    await page.keyboard.press("Enter");
    await expect(toggle(0)).toHaveAttribute("aria-expanded", "false");
    await page.keyboard.press(" ");
    await expect(toggle(0)).toHaveAttribute("aria-expanded", "true");

    // A modifier leaves the arrows to the browser and other shortcuts. (Not Left or Right: Alt+Left is Back.)
    for (const modifier of ["Alt", "Control", "Meta", "Shift"]) await press(`${modifier}+ArrowDown`, toggle(0));

    // Down walks header, row, row, next header; Left on a row returns to its header.
    const rowsOf = async (i: number) =>
      page.locator(`[id="${await toggle(i).getAttribute("aria-controls")}"] [data-testid="sidebar-session-row"]`);
    const first = await rowsOf(0);
    const second = await rowsOf(1);
    await press("ArrowDown", first.nth(0));
    await press("ArrowDown", first.nth(1));
    await press("ArrowDown", toggle(1));
    await press("ArrowDown", second.nth(0));
    await press("ArrowLeft", toggle(1));

    // Left collapses, Right expands, Right again enters the rows.
    await page.keyboard.press("ArrowLeft");
    await expect(toggle(1)).toHaveAttribute("aria-expanded", "false");
    await page.keyboard.press("ArrowRight");
    await expect(toggle(1)).toHaveAttribute("aria-expanded", "true");
    await press("ArrowRight", second.nth(0));

    // A collapsed Group's rows are skipped.
    await toggle(0).focus();
    await page.keyboard.press("ArrowLeft");
    await expect(toggle(0)).toHaveAttribute("aria-expanded", "false");
    await press("ArrowDown", toggle(1));
    await press("ArrowUp", toggle(0));

    // Shift+F10 opens the menu with focus on its first item; Escape closes it and focus returns to the header button.
    await expect(toggle(0)).toHaveAttribute("aria-keyshortcuts", "Shift+F10");
    await page.keyboard.press("Shift+F10");
    const menu = page.getByTestId("sidebar-group-context-menu");
    await expect(menu).toBeVisible();
    await expect(menu.getByRole("button").first()).toBeFocused();
    await page.keyboard.press("Escape");
    await expect(menu).toBeHidden();
    await expect(toggle(0)).toBeFocused();
  });
}

test("the Snoozed & archived toggle is a header for the arrow keys", async ({ page }) => {
  // The live session's Group is named like the section, so a shared rows id would send Right into it.
  const sessions: MockSessionInput[] = [
    { id: "s-live", title: "live", project_path: "/tmp/repo-a", branch: "a/live", group: "snoozed-and-archived" },
    {
      id: "s-old",
      title: "old",
      project_path: "/tmp/repo-b",
      branch: "b/old",
      fields: { archived_at: "2026-01-01T00:00:00Z" },
    },
  ];
  await installSidebarMocks(page, { sessions });
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto("/");

  const press = async (key: string, focused: Locator) => {
    await page.keyboard.press(key);
    await expect(focused, key).toBeFocused();
  };
  const toggle = page.getByTestId("sidebar-sunk-toggle");
  const live = page.getByTestId("sidebar-session-row").filter({ hasText: "live" });
  const old = page.getByTestId("sidebar-session-row").filter({ hasText: "old" });

  // Down from the last live row stops on the toggle; Right opens it, Right again reaches its row.
  await live.focus();
  await press("ArrowDown", toggle);
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  await press("ArrowRight", toggle);
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await press("ArrowRight", old);
  // Left on an archived row comes back to the toggle, which Left then folds.
  await press("ArrowLeft", toggle);
  await press("ArrowLeft", toggle);
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
});

test.describe("Archive all that empties a Group", () => {
  // `fields` is merged into each list response, so an archive shows up on the next refresh.
  // `hold` keeps an archive reply back until released.
  async function setup(page: Page, sessions: MockSessionInput[], hold?: Promise<void>) {
    await installSidebarMocks(page, { sessions });
    await page.route("**/api/sessions/*/archive", async (r) => {
      const id = new URL(r.request().url()).pathname.split("/")[3];
      await hold;
      const archived_at = new Date().toISOString();
      Object.assign(sessions.find((s) => s.id === id)?.fields ?? {}, { archived_at });
      await r.fulfill({ json: { id, archived_at } });
    });
    page.on("dialog", (dialog) => void dialog.accept());
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.goto("/");
  }
  const header = (page: Page, path: string) =>
    page.locator(`[data-testid="sidebar-group-header"][data-group-id="${path}"] button[aria-expanded]`).first();
  async function archiveAll(page: Page, from: Locator, sessionId: string) {
    await from.focus();
    await page.keyboard.press("Shift+F10");
    const item = page.getByTestId("sidebar-group-context-menu-archive-all");
    await expect(item).toBeVisible();
    // The menu focuses its first item a frame after opening; wait for that, or it takes focus back.
    await expect(page.getByTestId("sidebar-group-context-menu").getByRole("button").first()).toBeFocused();
    await item.focus();
    await Promise.all([page.waitForRequest(`**/api/sessions/${sessionId}/archive`), page.keyboard.press("Enter")]);
  }
  const one = (id: string, group: string, fields: Record<string, unknown> = {}): MockSessionInput => ({
    id,
    title: `${group}-only`,
    project_path: `/tmp/${group}`,
    branch: `${group}/only`,
    group,
    fields,
  });

  test("moves focus to the next Group header, then from the last Group to the previous one", async ({ page }) => {
    await setup(page, [one("s-a", "group-a"), one("s-b", "group-b"), one("s-c", "group-c")]);
    await archiveAll(page, header(page, "group-a"), "s-a");
    await expect(header(page, "group-a")).toHaveCount(0);
    await expect(header(page, "group-b")).toBeFocused();
    // Snoozed & archived now exists below the last Group; it isn't a Group, so focus goes up instead.
    await expect(page.getByTestId("sidebar-sunk-toggle")).toBeVisible();
    await archiveAll(page, header(page, "group-c"), "s-c");
    await expect(header(page, "group-c")).toHaveCount(0);
    await expect(header(page, "group-b")).toBeFocused();
  });

  test("leaves focus alone when it moved before the Group emptied", async ({ page }) => {
    let release: () => void = () => {};
    const hold = new Promise<void>((resolve) => (release = resolve));
    await setup(page, [one("s-a", "group-a"), one("s-b", "group-b")], hold);
    await archiveAll(page, header(page, "group-a"), "s-a");
    // A click on nothing focusable moves focus to the page while the archive is still out.
    await page.mouse.click(900, 650);
    release();
    await expect(header(page, "group-a")).toHaveCount(0);
    await expect(header(page, "group-b")).not.toBeFocused();
  });
});
