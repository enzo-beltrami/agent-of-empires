// Plan modal on an ExitPlanMode approval: focus entry, Tab containment, Escape
// and focus restore, and inert local file links.

import type { Page } from "@playwright/test";
import { test, expect, pressShortcut } from "./helpers/mockedTest";
import { mockAcpSession, openStructuredSession } from "./helpers/acpMock";
import { mockSessionsList } from "./helpers/sessions";

const TITLE = "plan-modal";
const PLAN = `# Plan\n\nRead [PLAN.md](/tmp/${TITLE}/PLAN.md) first.\n\n- step one\n- step two`;

const approval = {
  nonce: "n-plan",
  tool_call: {
    id: "t-plan",
    name: "Ready to code?",
    kind: "switch_mode",
    args_preview: JSON.stringify({ plan: PLAN }),
    started_at: new Date().toISOString(),
  },
  destructive: false,
  options: [],
  choice: false,
  plan: PLAN,
  requested_at: new Date().toISOString(),
  resolved: null,
};

async function openPlanModal(page: Page, beforeOpen?: () => Promise<void>) {
  const mock = await mockAcpSession(page, { title: TITLE, reducedState: { pending_approvals: [approval] } });
  await openStructuredSession(page, mock);
  await beforeOpen?.();
  const opener = page.getByRole("button", { name: "View plan" });
  await expect(opener).toBeVisible({ timeout: 10_000 });
  await opener.click();
  const dialog = page.getByRole("dialog", { name: "Plan" });
  await expect(dialog).toBeVisible();
  return { opener, dialog };
}

test("plan modal takes focus, contains Tab, and returns focus on Escape", async ({ page }) => {
  const { opener, dialog } = await openPlanModal(page);
  const insideDialog = () => dialog.evaluate((el) => el.contains(document.activeElement));

  await expect.poll(insideDialog).toBe(true);
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press("Tab");
    expect(await insideDialog()).toBe(true);
  }
  for (let i = 0; i < 12; i++) {
    await page.keyboard.press("Shift+Tab");
    expect(await insideDialog()).toBe(true);
  }

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(opener).toBeFocused();
});

test("the command palette keeps Tab and Escape while it covers the plan modal", async ({ page }) => {
  const { dialog } = await openPlanModal(page);
  await pressShortcut(page, "ControlOrMeta+k");
  const palette = page.getByRole("dialog", { name: /command palette/i });
  await expect(palette).toBeVisible();
  const insidePalette = () => palette.evaluate((el) => el.contains(document.activeElement));
  await expect.poll(insidePalette).toBe(true);

  await page.keyboard.press("Tab");
  expect(await insidePalette()).toBe(true);

  await page.keyboard.press("Escape");
  await expect(palette).toBeHidden();
  await expect(dialog).toBeVisible();
});

test("a retained plan modal in a covered session ignores Escape meant for the active one", async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem("aoe-web-settings", JSON.stringify({ persistentStructuredViews: true }));
  });
  const mock = await mockAcpSession(page, {
    sessionId: "sess-a",
    title: "plan-a",
    reducedState: { pending_approvals: [approval] },
  });
  // Registered after the ACP mock so this two-session list wins; both sessions share its approval.
  await mockSessionsList(page, () =>
    ["sess-a", "sess-b"].map((id) => ({
      id,
      title: id === "sess-a" ? "plan-a" : "plan-b",
      project_path: `/tmp/${id}`,
      group_path: "/tmp",
      status: "Waiting",
      view: "structured",
      acp_worker_state: "running",
      claude_fullscreen: false,
    })),
  );
  await openStructuredSession(page, mock);
  await page.getByRole("button", { name: "View plan" }).click();
  await expect(page.getByTestId("plan-modal")).toHaveCount(1);

  await pressShortcut(page, "a");
  await expect(page).toHaveURL(/\/session\/sess-b$/);
  await expect(page.getByTestId("plan-modal")).toBeHidden();
  await page.getByRole("button", { name: "View plan" }).click();
  await expect(page.getByTestId("plan-modal")).toHaveCount(2);

  await page.keyboard.press("Escape");
  await expect(page.getByTestId("plan-modal")).toHaveCount(1);
  await expect(page.locator('[inert] [data-testid="plan-modal"]')).toHaveCount(1);
});

test("a plan file link renders as inert text and leaves the modal and the approval in place", async ({ page }) => {
  const { dialog } = await openPlanModal(page);

  await expect(dialog.getByRole("link", { name: "PLAN.md" })).toHaveCount(0);
  const path = dialog.locator("span.acp-inert-path", { hasText: "PLAN.md" });
  await expect(path).toBeVisible();
  await path.click();
  await expect(dialog).toBeVisible();
  await expect(page.getByRole("alertdialog", { includeHidden: true })).toHaveCount(1);
});

test.describe("short phone", () => {
  test.use({ viewport: { width: 390, height: 560 } });

  test("a long answer list scrolls the footer so the last action stays reachable", async ({ page }) => {
    const answers = [
      "Yes, and auto-accept edits",
      "Yes, and manually approve edits",
      "Yes, but only for this file",
      "Yes, after running the tests",
      "No, revise the plan first",
      "No, keep planning",
    ];
    const choice = {
      ...approval,
      choice: true,
      options: answers.map((name, i) => ({ option_id: `o-${i}`, name, kind: "allow_once" })),
    };
    const mock = await mockAcpSession(page, { title: TITLE, reducedState: { pending_approvals: [choice] } });
    const decisions: unknown[] = [];
    await page.route("**/api/sessions/*/acp/approvals/**", (r) => {
      decisions.push(r.request().postDataJSON());
      return r.fulfill({ json: {} });
    });
    await openStructuredSession(page, mock);
    await page.getByRole("button", { name: "View plan" }).click();
    const dialog = page.getByRole("dialog", { name: "Plan" });
    const panel = (await dialog.locator("> div").boundingBox())!;
    expect(panel.y + panel.height).toBeLessThanOrEqual(560);
    for (const name of ["Close", ...answers, "Dismiss"]) {
      const box = (await dialog.getByRole("button", { name, exact: true }).boundingBox())!;
      expect(box.x, name).toBeGreaterThanOrEqual(panel.x);
      expect(box.x + box.width, name).toBeLessThanOrEqual(panel.x + panel.width);
      expect(box.height, name).toBeLessThan(60);
    }

    // The list overflows its bounded footer; wheeling over it brings Dismiss fully into view.
    const footer = dialog.locator("> div > div").last();
    expect(await footer.evaluate((el) => el.scrollHeight > el.clientHeight)).toBe(true);
    const dismiss = dialog.getByRole("button", { name: "Dismiss", exact: true });
    await expect(dismiss).not.toBeInViewport({ ratio: 1 });
    await footer.hover();
    await page.mouse.wheel(0, 1000);
    await expect(dismiss).toBeInViewport({ ratio: 1 });
    const bounds = (await footer.boundingBox())!;
    const box = (await dismiss.boundingBox())!;
    expect(box.y).toBeGreaterThanOrEqual(bounds.y);
    expect(box.y + box.height).toBeLessThanOrEqual(bounds.y + bounds.height);
    await dismiss.click();
    await expect.poll(() => decisions).toEqual([{ decision: "Cancelled" }]);
  });
});
