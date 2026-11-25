// Sidebar grouping axes (#1234, #4867): user Groups (the default) and status buckets, with per-axis collapse
// persisted in localStorage, plus the Group header's own actions. Bucketing is unit-tested in
// src/lib/__tests__/sidebarGroups.test.ts.

import { test, expect } from "./helpers/mockedTest";
import type { Locator, Page, Route } from "@playwright/test";
import { installSidebarMocks, type MockSessionInput } from "./helpers/sidebarMocks";
import { sessionResponse } from "./helpers/sessions";

const HEADER = "[data-testid='sidebar-group-header']";
const ROW = "[data-testid='sidebar-session-row']";
const AXIS_TOGGLE = "[data-testid='sidebar-axis-toggle']";

function groupedSessions(): MockSessionInput[] {
  return [
    { id: "s-f1", title: "feat-one", project_path: "/tmp/project", branch: "feat/one", group: "feature" },
    { id: "s-f2", title: "feat-two", project_path: "/tmp/project", branch: "feat/two", group: "feature" },
    { id: "s-r1", title: "refac-one", project_path: "/tmp/project", branch: "refac/one", group: "refactor" },
  ];
}

function statusSessions(): MockSessionInput[] {
  return [
    {
      id: "s-idle",
      title: "idle-session",
      project_path: "/tmp/project",
      branch: "feat/idle",
      fields: { status: "Idle" },
    },
    {
      id: "s-waiting",
      title: "waiting-session",
      project_path: "/tmp/project",
      branch: "feat/waiting",
      fields: { status: "Waiting" },
    },
    {
      id: "s-running",
      title: "running-session",
      project_path: "/tmp/project",
      branch: "feat/running",
      fields: { status: "Running" },
    },
  ];
}

async function resolvedColor(page: Page, token: string): Promise<string> {
  return await page.evaluate((name) => {
    const probe = document.createElement("span");
    probe.style.color = `var(${name})`;
    document.body.append(probe);
    const rgb = getComputedStyle(probe).color;
    probe.remove();
    return rgb;
  }, token);
}

async function gotoDesktop(page: Page) {
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.goto("/");
}

async function recordArchives(page: Page, ids: string[], sessions?: MockSessionInput[]) {
  await page.route("**/api/sessions/*/archive", async (route) => {
    const id = decodeURIComponent(new URL(route.request().url()).pathname.split("/").at(-2)!);
    const input = sessions?.find((session) => session.id === id);
    ids.push(id);
    await route.fulfill({
      json: input
        ? sessionResponse({
            id,
            title: input.title,
            project_path: input.project_path,
            branch: input.branch,
            group_path: input.group ?? "",
            ...input.fields,
          })
        : sessionResponse({ id }),
    });
  });
}

// The axis picker sets any axis directly, so status is one pick from the default.
async function pickAxis(page: Page, toggle: Locator, target: string) {
  await toggle.click();
  await page.getByTestId(`sidebar-axis-option-${target}`).click();
  await expect(toggle).toHaveAttribute("data-axis", target);
}

const header = (page: Page, id: string) => page.locator(`${HEADER}[data-group-id='${id}']`);

test.describe("sidebar Group axis (#1234)", () => {
  test("Groups are the default; the filter narrows by Group name or row, and collapse persists per axis", async ({
    page,
  }) => {
    await installSidebarMocks(page, { sessions: groupedSessions() });
    await gotoDesktop(page);

    const axisToggle = page.locator(AXIS_TOGGLE);
    await expect(axisToggle).toHaveAttribute("data-axis", "group");
    await expect(page.getByTestId("sidebar-axis-heading")).toHaveText("Groups");
    await expect(page.locator(HEADER)).toHaveCount(2);
    await expect(page.locator(HEADER).nth(0)).toHaveAttribute("data-group-id", "feature");
    await expect(page.locator(HEADER).nth(1)).toHaveAttribute("data-group-id", "refactor");
    await expect(page.locator(ROW)).toHaveCount(3);

    await page.getByLabel("Filter sessions").click();
    const filter = page.locator("[data-testid='sidebar-filter-input']");
    // (query, Groups left, rows left)
    for (const [query, groups, rows] of [
      ["refactor", 1, 1],
      ["feat-two", 1, 1],
      ["", 2, 3],
    ] as const) {
      await filter.fill(query);
      await expect(page.locator(HEADER), query).toHaveCount(groups);
      await expect(page.locator(ROW), query).toHaveCount(rows);
    }
    await filter.fill("nonexistent-xyz");
    await expect(page.getByText(/No matches for/)).toBeVisible();
    await expect(page.locator(ROW)).toHaveCount(0);
    await filter.fill("");

    const featureExpand = header(page, "feature").locator("button[aria-expanded]");
    await featureExpand.click();
    await expect(featureExpand).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByText("feat-one")).toBeHidden();

    await page.reload();
    await expect(featureExpand).toHaveAttribute("aria-expanded", "false");

    // Collapse maps are per axis: the status bucket holding the same rows starts expanded.
    await pickAxis(page, axisToggle, "status");
    await expect(page.locator(`${HEADER} button[aria-expanded]`)).toHaveAttribute("aria-expanded", "true");
  });

  test("a collapsed group marks that it holds the open session (#3912)", async ({ page }) => {
    await installSidebarMocks(page, { sessions: groupedSessions() });
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.goto("/session/s-f1");

    const featureHeader = header(page, "feature");
    const refactorHeader = header(page, "refactor");
    const expandBtn = featureHeader.locator("button[aria-expanded]");

    // Expanded, the open session's row carries the frame instead of the header.
    await expect(expandBtn).toHaveAttribute("aria-expanded", "true");
    expect(await featureHeader.getAttribute("class")).not.toContain("border-session-active");

    // Collapsed, the header uses the projected token; border-brand-600 fails contrast on catppuccin-latte.
    await expandBtn.click();
    await expect(page.getByText("feat-one")).toBeHidden();
    expect(await featureHeader.getAttribute("class")).toContain("border-session-active");
    expect(await refactorHeader.getAttribute("class")).not.toContain("border-session-active");
    // toHaveCSS polls past the color transition.
    await expect(featureHeader).toHaveCSS("border-left-color", await resolvedColor(page, "--color-session-active"));
  });

  test("a Shift+click range skips the rows hidden under a collapsed Group", async ({ page }) => {
    const sessions: MockSessionInput[] = ["a", "b", "c"].map((name) => ({
      id: `s-${name}`,
      title: `${name}-session`,
      project_path: `/tmp/repo-${name}`,
      branch: `feat/${name}`,
      group: `group-${name}`,
    }));
    const archived: string[] = [];
    await installSidebarMocks(page, { sessions });
    await recordArchives(page, archived, sessions);
    await gotoDesktop(page);

    // Hide Group B's row, then range from A to C across it.
    await header(page, "group-b").locator("button[aria-expanded]").click();
    await expect(page.getByText("b-session")).toBeHidden();

    const row = (title: string) => page.locator(ROW).filter({ hasText: title });
    await row("a-session").click({ modifiers: ["Meta"] });
    await row("c-session").click({ modifiers: ["Shift"] });
    await row("c-session").click({ button: "right" });
    await page.getByTestId("sidebar-context-menu-bulk-archive").click();

    await expect.poll(() => archived.length).toBe(2);
    expect([...archived].sort()).toEqual(["s-a", "s-c"]);
  });
});

test.describe("sidebar Group header", () => {
  async function openMenu(page: Page, id: string) {
    await header(page, id).click({ button: "right" });
    await expect(page.getByTestId("sidebar-group-context-menu")).toBeVisible();
  }

  test("Rename sends the full new path for the Group's profile", async ({ page }) => {
    await installSidebarMocks(page, { sessions: groupedSessions() });
    const renames: unknown[] = [];
    await page.route("**/api/groups", async (route) => {
      if (route.request().method() !== "PATCH") return route.fallback();
      renames.push(route.request().postDataJSON());
      await route.fulfill({ json: {} });
    });
    await gotoDesktop(page);

    await openMenu(page, "feature");
    await page.getByTestId("sidebar-group-context-menu-rename").click();
    const input = page.getByTestId("sidebar-group-rename-input");
    await expect(input).toHaveValue("feature");
    await input.fill("work/features");
    await input.press("Enter");

    await expect
      .poll(() => renames)
      .toEqual([
        { source: { profile: "default", path: "feature" }, target: { profile: "default", path: "work/features" } },
      ]);
  });

  test("a color tints the header and survives a reload", async ({ page }) => {
    await installSidebarMocks(page, { sessions: groupedSessions() });
    await gotoDesktop(page);

    await expect(header(page, "feature")).not.toHaveAttribute("style", /background-color/);
    await openMenu(page, "feature");
    await page.getByTestId("sidebar-group-color-teal").click();
    await page.keyboard.press("Escape");
    await expect(header(page, "feature")).toHaveAttribute("style", /background-color/);
    expect(await page.evaluate(() => localStorage.getItem("aoe-group-color-v1"))).toBe(
      JSON.stringify({ feature: "teal" }),
    );

    await page.reload();
    await expect(header(page, "feature")).toHaveAttribute("style", /background-color/);
    await expect(header(page, "refactor")).not.toHaveAttribute("style", /background-color/);
  });

  test("the header's + opens the wizard with its Group prefilled", async ({ page }) => {
    await installSidebarMocks(page, {
      sessions: [
        ...groupedSessions(),
        { id: "s-loose", title: "loose-one", project_path: "/tmp/project", branch: "loose/one" },
      ],
    });

    // (Group id, prefilled Group)
    for (const [id, prefilled] of [
      ["refactor", "refactor"],
      ["__ungrouped__", ""],
    ] as const) {
      await gotoDesktop(page);
      await header(page, id)
        .getByRole("button", { name: /New session in/ })
        .click();
      // With no path prefilled the wizard opens on its Project picker; the Group field is one step back.
      await page.getByRole("button", { name: "Back" }).click();
      await expect(page.locator("#wizard-group"), id).toHaveValue(prefilled);
    }
  });
});

test.describe("sidebar status axis (#4867)", () => {
  test("groups sessions by execution status in attention order; collapse persists and stays per axis", async ({
    page,
  }) => {
    await installSidebarMocks(page, { sessions: statusSessions() });
    await gotoDesktop(page);

    const axisToggle = page.locator(AXIS_TOGGLE);
    const axisHeading = page.getByTestId("sidebar-axis-heading");
    await expect(axisToggle).toHaveAttribute("data-axis", "group");
    await expect(page.locator(ROW)).toHaveCount(3);

    await pickAxis(page, axisToggle, "status");
    await expect(axisToggle).toHaveAttribute("data-axis", "status");
    await expect(axisHeading).toHaveText("Status");

    const headers = page.locator(HEADER);
    await expect(headers).toHaveCount(3);
    // The Attention sort's order: Waiting, then Idle, then Running.
    await expect(headers.nth(0)).toHaveAttribute("data-group-id", "status:Waiting");
    await expect(headers.nth(1)).toHaveAttribute("data-group-id", "status:Idle");
    await expect(headers.nth(2)).toHaveAttribute("data-group-id", "status:Running");
    await expect(page.locator(ROW)).toHaveCount(3);

    const waitingExpand = page.locator(`${HEADER}[data-group-id='status:Waiting']`).locator("button[aria-expanded]");
    await waitingExpand.click();
    await expect(waitingExpand).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByText("waiting-session")).toBeHidden();
    await expect(page.getByText("running-session")).toBeVisible();

    await page.reload();
    await expect(axisToggle).toHaveAttribute("data-axis", "status");
    await expect(
      page.locator(`${HEADER}[data-group-id='status:Waiting']`).locator("button[aria-expanded]"),
    ).toHaveAttribute("aria-expanded", "false");

    // Collapse keys are per axis; the Ungrouped header never collapsed.
    await pickAxis(page, axisToggle, "group");
    await expect(page.locator(`${HEADER} button[aria-expanded]`)).toHaveAttribute("aria-expanded", "true");
  });

  test("the picker follows the menu-button pattern: focus on open, arrows, Escape and Tab-out", async ({ page }) => {
    await installSidebarMocks(page, { sessions: statusSessions() });
    await gotoDesktop(page);

    const axisToggle = page.locator(AXIS_TOGGLE);
    const menu = page.getByTestId("sidebar-axis-menu");
    const option = (target: string) => page.getByTestId(`sidebar-axis-option-${target}`);
    await axisToggle.focus();
    await page.keyboard.press("Enter");

    await expect(menu).toBeVisible();
    await expect(option("group")).toHaveText("Group");
    // Same single-select menu semantics as the sort picker beside it.
    await expect(menu).toHaveAttribute("role", "menu");
    await expect(option("group")).toHaveAttribute("role", "menuitemradio");
    // Focus enters on the checked option, and the menu owns the arrows, so options are not Tab stops.
    await expect(option("group")).toBeFocused();
    await expect(option("group")).toHaveAttribute("tabindex", "-1");

    await page.keyboard.press("ArrowDown");
    await expect(option("status")).toBeFocused();
    // Both ends wrap.
    await page.keyboard.press("ArrowDown");
    await expect(option("group")).toBeFocused();
    await page.keyboard.press("ArrowUp");
    await expect(option("status")).toBeFocused();
    await page.keyboard.press("Home");
    await expect(option("group")).toBeFocused();
    await page.keyboard.press("End");
    await expect(option("status")).toBeFocused();

    // Escape closes without picking and hands the focus back to the trigger.
    await page.keyboard.press("Escape");
    await expect(menu).toBeHidden();
    await expect(axisToggle).toBeFocused();
    await expect(axisToggle).toHaveAttribute("data-axis", "group");

    // Enter on an arrowed-to option commits it, and the trigger keeps the focus.
    await page.keyboard.press("Enter");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("Enter");
    await expect(menu).toBeHidden();
    await expect(axisToggle).toHaveAttribute("data-axis", "status");
    await expect(axisToggle).toBeFocused();

    // Tabbing out closes the menu instead of walking its options.
    await page.keyboard.press("Enter");
    await expect(menu).toBeVisible();
    await page.keyboard.press("Tab");
    await expect(menu).toBeHidden();
  });

  test("bulk archive targets each selected row's own session when one workspace straddles two buckets", async ({
    page,
  }) => {
    // One workspace with a Waiting and a Running session: the status axis renders it twice, and the
    // Waiting bucket sorts first, so the workspace id alone resolves to the Waiting session.
    const sessions: MockSessionInput[] = [
      {
        id: "s-wait",
        title: "wait-session",
        project_path: "/tmp/shared",
        branch: "feat/shared",
        fields: { status: "Waiting" },
      },
      {
        id: "s-run",
        title: "run-session",
        project_path: "/tmp/shared",
        branch: "feat/shared",
        fields: { status: "Running" },
      },
      {
        id: "s-other",
        title: "other-session",
        project_path: "/tmp/other",
        branch: "feat/other",
        fields: { status: "Running" },
      },
    ];
    const archived: string[] = [];
    await installSidebarMocks(page, { sessions });
    await recordArchives(page, archived, sessions);
    await gotoDesktop(page);
    await pickAxis(page, page.locator(AXIS_TOGGLE), "status");

    await expect(page.getByText("wait-session")).toBeVisible();
    // Cmd rather than Ctrl: Chromium on macOS reads Ctrl+click as a secondary click.
    const [runRow, otherRow] = ["run-session", "other-session"].map((title) =>
      page.locator(ROW).filter({ hasText: title }),
    );
    await runRow!.click({ modifiers: ["Meta"] });
    await otherRow!.click({ modifiers: ["Meta"] });
    await otherRow!.click({ button: "right" });
    await page.getByTestId("sidebar-context-menu-bulk-archive").click();

    await expect.poll(() => archived.length).toBe(2);
    expect([...archived].sort()).toEqual(["s-other", "s-run"]);
  });

  test("a workspace split across buckets marks only the slice showing the active session", async ({ page }) => {
    const sessions: MockSessionInput[] = [
      {
        id: "s-wait",
        title: "wait-session",
        project_path: "/tmp/shared",
        branch: "feat/shared",
        fields: { status: "Waiting" },
      },
      {
        id: "s-run",
        title: "run-session",
        project_path: "/tmp/shared",
        branch: "feat/shared",
        fields: { status: "Running" },
      },
    ];
    await installSidebarMocks(page, { sessions });
    await gotoDesktop(page);
    await pickAxis(page, page.locator(AXIS_TOGGLE), "status");

    await page.locator(ROW).filter({ hasText: "run-session" }).click();
    await expect(page).toHaveURL(/\/session\/s-run/);
    // Both slices share the workspace id, but only the Running one shows the open session.
    const active = page.locator("[data-active-session-row]");
    await expect(active).toHaveCount(1);
    await expect(active).toContainText("run-session");
  });

  test("a range across the footer selects each workspace once, not its hidden status slices", async ({ page }) => {
    const archivedUntil = { archived_at: "2026-01-01T00:00:00Z" };
    const sessions: MockSessionInput[] = [
      {
        id: "s-w1-waiting",
        title: "w1-waiting",
        project_path: "/tmp/w1",
        branch: "feat/w1",
        fields: { status: "Waiting", ...archivedUntil },
      },
      {
        id: "s-w1-running",
        title: "w1-running",
        project_path: "/tmp/w1",
        branch: "feat/w1",
        fields: { status: "Running", ...archivedUntil },
      },
      {
        id: "s-w2-idle",
        title: "w2-idle",
        project_path: "/tmp/w2",
        branch: "feat/w2",
        fields: { status: "Idle", ...archivedUntil },
      },
    ];
    const unarchived: string[] = [];
    await installSidebarMocks(page, { sessions });
    await recordArchives(page, unarchived);
    await gotoDesktop(page);
    await pickAxis(page, page.locator(AXIS_TOGGLE), "status");
    await page.getByTestId("sidebar-sunk-toggle").click();

    // One footer row per workspace: W1 is sliced into Waiting and Running buckets internally.
    const footerRows = page.locator(`[data-testid='sidebar-sunk-section'] ${ROW}`);
    await expect(footerRows).toHaveCount(2);

    await footerRows.nth(0).click();
    await footerRows.nth(1).click({ modifiers: ["Shift"] });
    await footerRows.nth(1).click({ button: "right" });
    await page.getByTestId("sidebar-context-menu-bulk-unarchive").click();

    await expect.poll(() => unarchived.length).toBe(2);
    expect([...unarchived].sort()).toEqual(["s-w1-waiting", "s-w2-idle"]);
  });

  test("snoozing one status slice leaves its sibling row and bulk eligibility alone", async ({ page }) => {
    const sessions: MockSessionInput[] = [
      {
        id: "s-wait",
        title: "wait-session",
        project_path: "/tmp/shared",
        branch: "feat/shared",
        fields: { status: "Waiting" },
      },
      {
        id: "s-run",
        title: "run-session",
        project_path: "/tmp/shared",
        branch: "feat/shared",
        fields: { status: "Running" },
      },
    ];
    const snoozed: string[] = [];
    await installSidebarMocks(page, { sessions });
    await page.route("**/api/sessions/*/snooze", async (route) => {
      const id = decodeURIComponent(new URL(route.request().url()).pathname.split("/").at(-2)!);
      snoozed.push(id);
      const { minutes } = route.request().postDataJSON() as { minutes: number | null };
      await route.fulfill({
        json: sessionResponse({ id, snoozed_until: minutes == null ? null : "2099-01-01T00:00:00Z" }),
      });
    });
    await gotoDesktop(page);
    await pickAxis(page, page.locator(AXIS_TOGGLE), "status");

    const waitingRow = page.locator(ROW).filter({ hasText: "wait-session" });
    const runningRow = page.locator(ROW).filter({ hasText: "run-session" });
    await runningRow.click({ button: "right" });
    await page.getByTestId("sidebar-context-menu-snooze").click();
    await page.getByTestId("snooze-modal-preset-60").click();
    await expect.poll(() => snoozed).toEqual(["s-run"]);

    // Only the Running row carries the pending snooze.
    await expect(runningRow.getByLabel("Snoozed")).toBeVisible();
    await expect(waitingRow.getByLabel("Snoozed")).toHaveCount(0);

    // Bulk buckets follow the same scope: one snoozable row, one unsnoozable. A plain click first
    // clears the single selection the snooze row's right-click left behind.
    await waitingRow.click();
    await waitingRow.click({ modifiers: ["Meta"] });
    await runningRow.click({ modifiers: ["Meta"] });
    await runningRow.click({ button: "right" });
    await expect(page.getByTestId("sidebar-context-menu-bulk-unsnooze")).toHaveText("Unsnooze 1");
    await expect(page.getByText("Snooze 1", { exact: true })).toBeVisible();
  });

  test("archiving one status slice leaves its sibling slice's own pending action alone", async ({ page }) => {
    const sessions: MockSessionInput[] = [
      {
        id: "s-wait",
        title: "wait-session",
        project_path: "/tmp/shared",
        branch: "feat/shared",
        fields: { status: "Waiting" },
      },
      {
        id: "s-run",
        title: "run-session",
        project_path: "/tmp/shared",
        branch: "feat/shared",
        fields: { status: "Running" },
      },
    ];
    const patched: string[] = [];
    let release!: () => void;
    const held = new Promise<void>((resolve) => (release = resolve));
    await installSidebarMocks(page, { sessions });
    // Hold both PATCHes so the two slices carry pending values at the same time.
    const hold = async (route: Route) => {
      const parts = new URL(route.request().url()).pathname.split("/");
      const id = decodeURIComponent(parts.at(-2)!);
      patched.push(`${id}:${parts.at(-1)}`);
      await held;
      await route.fulfill({ json: sessionResponse({ id }) });
    };
    await page.route("**/api/sessions/*/archive", hold);
    await page.route("**/api/sessions/*/unread", hold);
    await gotoDesktop(page);
    await pickAxis(page, page.locator(AXIS_TOGGLE), "status");

    const row = (title: string) => page.locator(ROW).filter({ hasText: title });
    await row("wait-session").click({ button: "right" });
    await page.getByTestId("sidebar-context-menu-archive").click();
    await expect(row("wait-session").getByTitle("Archived")).toBeVisible();

    // The sibling's action must not claim the Waiting slice's pending archive.
    await row("run-session").click({ button: "right" });
    await page.getByTestId("sidebar-context-menu-unread").click();
    await expect.poll(() => patched.length).toBe(2);

    await expect(row("wait-session").getByTitle("Archived")).toBeVisible();
    await expect(row("run-session").getByTitle("Archived")).toHaveCount(0);

    // Bulk eligibility follows each slice's own state: one pending-archived, one live.
    await row("wait-session").click();
    await row("wait-session").click({ modifiers: ["Meta"] });
    await row("run-session").click({ modifiers: ["Meta"] });
    await row("run-session").click({ button: "right" });
    await expect(page.getByTestId("sidebar-context-menu-bulk-unarchive")).toHaveText("Unarchive 1");
    await expect(page.getByTestId("sidebar-context-menu-bulk-archive")).toHaveText("Archive 1");

    release();
    await expect.poll(() => [...patched].sort()).toEqual(["s-run:unread", "s-wait:archive"]);
  });
});
