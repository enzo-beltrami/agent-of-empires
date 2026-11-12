// @vitest-environment jsdom

import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

import type { PluginListResponse, PluginView } from "../../../lib/api";

const api = vi.hoisted(() => ({
  fetchPlugins: vi.fn(),
  setPluginEnabled: vi.fn(),
}));
vi.mock("../../../lib/api", () => api);

import { PluginsSettings } from "../PluginsSettings";

const STATUS: PluginView = {
  id: "aoe.status",
  name: "Agent Status Detection",
  version: "1.1.0",
  description: "Detects agent session status.",
  enabled: true,
};
const OTHER: PluginView = {
  ...STATUS,
  id: "aoe.other",
  name: "Other",
  version: "0.1.0",
  description: "A second plugin.",
  enabled: false,
};

const list = (plugins: PluginView[] = [STATUS, OTHER]): PluginListResponse => ({ plugins });

beforeEach(() => {
  Object.values(api).forEach((fn) => fn.mockReset());
  api.fetchPlugins.mockResolvedValue(list());
});

function renderWith(plugins?: PluginView[]) {
  if (plugins) api.fetchPlugins.mockResolvedValue(list(plugins));
  return render(<PluginsSettings />);
}

describe("installed plugins", () => {
  it("renders name, version and description, and hides the switch when read-only", async () => {
    const { unmount } = renderWith();
    for (const text of ["Agent Status Detection", "v1.1.0", "A second plugin."]) {
      expect(await screen.findByText(text)).toBeTruthy();
    }
    unmount();
    render(<PluginsSettings readOnly />);
    await screen.findByText("Agent Status Detection");
    expect(screen.queryByLabelText("Enable Agent Status Detection")).toBeNull();
  });

  it("shows an error when the list fails to load", async () => {
    api.fetchPlugins.mockResolvedValue(null);
    render(<PluginsSettings />);
    expect(await screen.findByText("Failed to load plugins.")).toBeTruthy();
  });

  it("disabling another plugin adopts the refreshed list without a confirm", async () => {
    renderWith([STATUS]);
    api.setPluginEnabled.mockResolvedValue({ kind: "ok", data: list([{ ...STATUS, enabled: false }]) });
    const toggle = (await screen.findByLabelText(`Enable ${STATUS.name}`)) as HTMLInputElement;
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle.checked).toBe(false));
    expect(api.setPluginEnabled).toHaveBeenLastCalledWith(STATUS.id, false);
    expect(screen.queryByTestId("web-dashboard-off-dialog")).toBeNull();
  });

  it("disabling the Web dashboard plugin asks first; only Turn off sends the request", async () => {
    const web = { ...STATUS, id: "aoe.web", name: "Web Dashboard" };
    renderWith([web]);
    const toggle = (await screen.findByLabelText("Enable Web Dashboard")) as HTMLInputElement;
    const dismissals: Array<[string, () => void]> = [
      ["Cancel", () => fireEvent.click(screen.getByRole("button", { name: "Cancel" }))],
      ["Escape", () => fireEvent.keyDown(document, { key: "Escape" })],
      ["backdrop", () => fireEvent.click(screen.getByTestId("web-dashboard-off-dialog"))],
    ];
    for (const [how, dismiss] of dismissals) {
      fireEvent.click(toggle);
      const dialog = await screen.findByRole("dialog", { name: "Turn off the Web dashboard?" });
      expect(dialog.textContent, how).toContain("won't start until you turn it back on");
      dismiss();
      await waitFor(() => expect(screen.queryByTestId("web-dashboard-off-dialog"), how).toBeNull());
      expect(toggle.checked, how).toBe(true);
    }
    expect(api.setPluginEnabled).not.toHaveBeenCalled();

    api.setPluginEnabled.mockResolvedValue({ kind: "ok", data: list([{ ...web, enabled: false }]) });
    fireEvent.click(toggle);
    fireEvent.click(await screen.findByTestId("web-dashboard-off-confirm"));
    await waitFor(() => expect(toggle.checked).toBe(false));
    expect(api.setPluginEnabled).toHaveBeenCalledTimes(1);
    expect(api.setPluginEnabled).toHaveBeenLastCalledWith("aoe.web", false);
    expect(screen.queryByTestId("web-dashboard-off-dialog")).toBeNull();

    // Turning it back on needs no confirm.
    api.setPluginEnabled.mockResolvedValue({ kind: "ok", data: list([web]) });
    fireEvent.click(toggle);
    await waitFor(() => expect(toggle.checked).toBe(true));
    expect(api.setPluginEnabled).toHaveBeenLastCalledWith("aoe.web", true);
  });

  it("surfaces a rejected toggle", async () => {
    api.setPluginEnabled.mockResolvedValue({ kind: "error", message: "Dashboard is read-only." });
    renderWith();
    fireEvent.click(await screen.findByLabelText("Enable Agent Status Detection"));
    expect(await screen.findByText("Dashboard is read-only.")).toBeTruthy();
  });
});
