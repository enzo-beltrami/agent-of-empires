// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";

import type { ServerAbout } from "../../lib/api";
import type { PushHealth } from "../../lib/pushHealth";
import { setServerDown } from "../../lib/connectionState";
import type { ConnectionBannerState } from "../../hooks/useConnectionBanner";

const fetchAbout = vi.fn();
vi.mock("../../lib/api", () => ({
  fetchAbout: (...args: unknown[]) => fetchAbout(...args),
  fetchSandboxImageStatus: () => Promise.resolve(null),
}));

let pushHealth: PushHealth = "healthy";
vi.mock("../../hooks/usePushSubscription", () => ({
  usePushSubscription: () => ({ state: { kind: "off" }, health: pushHealth, enable: vi.fn() }),
}));

// Null means the real hook runs.
let forced: {
  connection: ConnectionBannerState | null;
  dashboard: boolean;
  sandbox?: boolean;
} | null = null;
vi.mock("../../hooks/useConnectionBanner", async (importOriginal) => {
  const mod = await importOriginal<typeof import("../../hooks/useConnectionBanner")>();
  return { ...mod, useConnectionBanner: () => (forced ? forced.connection : mod.useConnectionBanner()) };
});
vi.mock("../../hooks/useDashboardUpdate", async (importOriginal) => {
  const mod = await importOriginal<typeof import("../../hooks/useDashboardUpdate")>();
  return {
    ...mod,
    useDashboardUpdate: () => (forced ? (forced.dashboard ? "1.22.3" : null) : mod.useDashboardUpdate()),
  };
});
vi.mock("../../hooks/useSandboxImageBanner", async (importOriginal) => {
  const mod = await importOriginal<typeof import("../../hooks/useSandboxImageBanner")>();
  return {
    ...mod,
    useSandboxImageBanner: () =>
      forced
        ? forced.sandbox
          ? { kind: "available", image: "img", onUpdate: () => {}, onDismiss: () => {} }
          : null
        : mod.useSandboxImageBanner(),
  };
});

import { BannerStack } from "../BannerStack";

function about(webBuildId: string): ServerAbout {
  return { web_build_id: webBuildId } as ServerAbout;
}

const NAMES: [RegExp, string][] = [
  [/Daemon unreachable/, "disconnected"],
  [/server's settings/, "server settings"],
  [/Dashboard updated/, "dashboard updated"],
  [/Reconnected/, "reconnected"],
  [/Sandbox image/, "sandbox image"],
  [/Notifications stopped/, "push"],
];

function shown(container: HTMLElement): string[] {
  // Top-level banners only; a banner's own copy feedback is a status region inside it.
  return Array.from(container.querySelectorAll(':scope > [role="alert"], :scope > [role="status"]')).map((el) => {
    const text = `${el.getAttribute("aria-label") ?? ""} ${el.textContent ?? ""}`;
    return NAMES.find(([re]) => re.test(text))?.[1] ?? text;
  });
}

beforeEach(() => {
  const script = document.createElement("script");
  script.type = "module";
  script.src = "/assets/index-PageBuild.js";
  document.head.appendChild(script);
  fetchAbout.mockReset();
  pushHealth = "healthy";
  forced = null;
  setServerDown(false);
});

afterEach(() => {
  cleanup();
  setServerDown(false);
  document.head.querySelectorAll("script").forEach((s) => s.remove());
});

describe("BannerStack", () => {
  it("shows one connection or update banner, most urgent first, and push only while connected", () => {
    // (connection, dashboard updated, sandbox image, push unhealthy, banners top to bottom)
    const cases: [ConnectionBannerState | null, boolean, boolean, boolean, string[]][] = [
      [null, false, false, false, []],
      [null, false, false, true, ["push"]],
      [null, false, true, false, ["sandbox image"]],
      [null, false, true, true, ["sandbox image", "push"]],
      ["down", true, true, true, ["disconnected"]],
      ["reconnected", false, true, true, ["reconnected", "push"]],
      ["reconnected", true, false, false, ["dashboard updated"]],
      [null, true, true, true, ["dashboard updated", "push"]],
    ];
    for (const [connection, dashboard, sandbox, pushUnhealthy, want] of cases) {
      forced = { connection, dashboard, sandbox };
      pushHealth = pushUnhealthy ? "revoked" : "healthy";
      const { container, unmount } = render(<BannerStack />);
      expect(shown(container), JSON.stringify({ connection, dashboard, sandbox, pushUnhealthy })).toEqual(want);
      unmount();
    }
  });

  it("puts the server settings notice under Disconnected and over the rest, and Retry asks again", () => {
    // (connection, banners top to bottom)
    const cases: [ConnectionBannerState | null, string[]][] = [
      ["down", ["disconnected"]],
      [null, ["server settings", "push"]],
      ["reconnected", ["server settings", "push"]],
    ];
    const retry = vi.fn();
    for (const [connection, want] of cases) {
      forced = { connection, dashboard: true, sandbox: true };
      pushHealth = "revoked";
      const { container, unmount } = render(<BannerStack onRetryServerSettings={retry} />);
      expect(shown(container), String(connection)).toEqual(want);
      unmount();
    }
    forced = { connection: null, dashboard: false, sandbox: false };
    render(<BannerStack onRetryServerSettings={retry} />);
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(retry).toHaveBeenCalledTimes(1);
  });

  it("re-checks the bundle once on reconnect", async () => {
    fetchAbout.mockResolvedValue(about("index-PageBuild.js"));
    const { container } = render(<BannerStack />);
    await waitFor(() => expect(fetchAbout).toHaveBeenCalledTimes(1));

    act(() => setServerDown(true));
    expect(shown(container)).toEqual(["disconnected"]);

    // The Daemon restarted with a new bundle. The reconnect lands inside the about check's 30 s
    // throttle, so only the forced re-check can see it.
    fetchAbout.mockResolvedValue(about("index-NewerBuild.js"));
    act(() => setServerDown(false));
    await screen.findByRole("status", { name: "Dashboard update available" });
    expect(fetchAbout).toHaveBeenCalledTimes(2);
    expect(shown(container)).toEqual(["dashboard updated"]);
  });
});
