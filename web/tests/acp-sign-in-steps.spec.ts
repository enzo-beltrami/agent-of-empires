// The composer's Not logged in label opens the Agent's sign-in steps.

import type { Page } from "@playwright/test";
import { test, expect } from "./helpers/mockedTest";
import { mockAcpSession, openStructuredSession } from "./helpers/acpMock";
import { copiedBy } from "./helpers/clipboard";
import { iPhone13 } from "./helpers/viewports";

test.use({ permissions: ["clipboard-read", "clipboard-write"] });

test("the keyboard opens the steps, Copy takes the command, and desktop offers no Open Terminal", async ({ page }) => {
  const mock = await mockAcpSession(page, {
    title: "story-sign-in",
    initialEvents: [{ AuthStatusUpdated: { status: { kind: "none", label: "Not logged in" } } }],
  });
  const restarts: string[] = [];
  await page.route("**/api/sessions/*/restart", (r) => {
    restarts.push(r.request().method());
    return r.fulfill({ json: {} });
  });
  await openStructuredSession(page, mock);

  const label = page.getByRole("button", { name: "Not logged in" });
  await label.focus();
  await page.keyboard.press("Enter");
  const steps = page.getByRole("dialog", { name: /isn't signed in/ });
  await expect(steps).toBeVisible();
  // Named from the same Agent that picks the steps, not "the Agent".
  await expect(steps.getByRole("heading")).toHaveText("Claude Code isn't signed in");
  await page.keyboard.press("Escape");
  await expect(steps).toBeHidden();
  await expect(label).toBeFocused();
  await page.keyboard.press("Space");
  await expect(steps).toBeVisible();

  const command = await steps.getByTestId("sign-in-command").textContent();
  expect(command).toBe("claude auth login");
  expect(await copiedBy(page, () => steps.getByRole("button", { name: "Copy command" }).click())).toBe(command);
  // Desktop has no paired terminal to open; the command is run elsewhere.
  await expect(steps.getByRole("button")).toHaveText(["Copy command", "Restart Agent"]);

  await steps.getByRole("button", { name: "Restart Agent" }).click();
  await expect.poll(() => restarts).toEqual(["POST"]);
});

for (const [server, about] of [
  ["read-only", { read_only: true }],
  ["CityHall", { cityhall_mode: true }],
] as const) {
  test(`a ${server} server shows only the command`, async ({ page }) => {
    const mock = await mockAcpSession(page, {
      title: "story-sign-in",
      about,
      initialEvents: [{ AuthStatusUpdated: { status: { kind: "none", label: "Not logged in" } } }],
    });
    await openStructuredSession(page, mock);
    await page.getByRole("button", { name: "Not logged in" }).click();
    const steps = page.getByRole("dialog", { name: /isn't signed in/ });
    await expect(steps.getByTestId("sign-in-command")).toHaveText("claude auth login");
    await expect(steps.getByRole("button")).toHaveText(["Copy command"]);
  });
}

/** Records each paired-terminal start by surface, answering it as started. */
async function recordTerminals(page: Page): Promise<string[]> {
  const terminals: string[] = [];
  await page.route(/\/api\/sessions\/[^/]+\/(container-)?terminal\?/, (r) => {
    terminals.push(new URL(r.request().url()).pathname.split("/").pop()!);
    return r.fulfill({ json: {} });
  });
  return terminals;
}

const pairedHasFocus = (page: Page) => page.evaluate(() => !!document.activeElement?.closest('[data-term="paired"]'));

test.describe("on a phone", () => {
  test.use(iPhone13);

  test("Open Terminal promotes the focused Host shell", async ({ page }) => {
    const mock = await mockAcpSession(page, {
      title: "story-sign-in",
      initialEvents: [{ AuthStatusUpdated: { status: { kind: "none", label: "Not logged in" } } }],
    });
    const terminals = await recordTerminals(page);
    await openStructuredSession(page, mock);

    await page.getByRole("button", { name: "Not logged in" }).click();
    const steps = page.getByRole("dialog", { name: /isn't signed in/ });
    await steps.getByRole("button", { name: "Open Terminal" }).click();

    await expect(steps).toBeHidden();
    await expect(page.getByTestId("mobile-paired-layer")).toBeVisible();
    await expect.poll(() => terminals).toEqual(["terminal"]);
    await expect.poll(() => pairedHasFocus(page)).toBe(true);
  });

  test("a sandboxed Session's open Host shell switches to the focused Container shell", async ({ page }) => {
    const mock = await mockAcpSession(page, {
      title: "story-sign-in",
      sessionFields: { is_sandboxed: true },
      initialEvents: [{ AuthStatusUpdated: { status: { kind: "none", label: "Not logged in" } } }],
    });
    const terminals = await recordTerminals(page);
    await openStructuredSession(page, mock);
    await page.getByRole("button", { name: "Toggle panels" }).click();
    await page.getByTestId("mobile-right-panel-pick-paired").click();
    const host = page.getByRole("button", { name: "Host", exact: true });
    const container = page.getByRole("button", { name: "Container", exact: true });
    await expect(host).toHaveAttribute("aria-pressed", "true");
    await expect.poll(() => terminals).toEqual(["terminal"]);

    await page.getByTestId("mobile-back-to-agent").click();
    await page.getByRole("button", { name: "Not logged in" }).click();
    const steps = page.getByRole("dialog", { name: /isn't signed in/ });
    await expect(steps).toContainText("Container terminal");
    await steps.getByRole("button", { name: "Open Terminal" }).click();

    await expect.poll(() => terminals).toEqual(["terminal", "container-terminal"]);
    await expect(container).toHaveAttribute("aria-pressed", "true");
    // The Host shell it replaced was mounted; the new Container shell takes the focus.
    await expect.poll(() => pairedHasFocus(page)).toBe(true);
  });
});
