// @vitest-environment jsdom

import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { MobileRightPanelPicker } from "../MobileRightPanelPicker";
import { FileDiff } from "lucide-react";

const describePane = (id: string) => ({ title: id === "diff" ? "Changes" : id, icon: FileDiff });

function setup(overrides: Partial<Parameters<typeof MobileRightPanelPicker>[0]> = {}) {
  const onSelect = vi.fn();
  const onClose = vi.fn();
  render(
    <MobileRightPanelPicker
      open
      active="agent"
      sessionTitle="s1"
      availablePanes={["diff"]}
      describePane={describePane}
      onSelect={onSelect}
      onClose={onClose}
      {...overrides}
    />,
  );
  return { onSelect, onClose };
}

describe("MobileRightPanelPicker", () => {
  it("hides gated entries not in availablePanes and marks the active one", () => {
    setup({ availablePanes: [], active: "paired" });
    expect(screen.queryByTestId("mobile-right-panel-pick-agents")).toBeNull();
    expect(screen.queryByTestId("mobile-right-panel-pick-diff")).toBeNull();
    expect(screen.queryByTestId("mobile-right-panel-pick-files")).toBeNull();
    // The mobile-only pseudo-views are never gated.
    expect(screen.getByTestId("mobile-right-panel-pick-agent")).toBeDefined();
    expect(screen.getByTestId("mobile-right-panel-pick-paired").getAttribute("aria-current")).toBe("true");
  });

  it("shows the Sub agents and Files entries and selects them when available", () => {
    const { onSelect } = setup({ availablePanes: ["diff", "files", "agents"] });
    fireEvent.click(screen.getByTestId("mobile-right-panel-pick-agents"));
    expect(onSelect).toHaveBeenCalledWith("agents");
    fireEvent.click(screen.getByTestId("mobile-right-panel-pick-files"));
    expect(onSelect).toHaveBeenCalledWith("files");
  });

  it("lists panes with their descriptor title and selects them by id", () => {
    const { onSelect } = setup({ availablePanes: ["diff"], active: "diff" });
    const option = screen.getByTestId("mobile-right-panel-pick-diff");
    expect(option.textContent).toContain("Changes");
    expect(option.getAttribute("aria-current")).toBe("true");
    fireEvent.click(option);
    expect(onSelect).toHaveBeenCalledWith("diff");
  });

  it("closes on backdrop click and on Escape only", () => {
    const { onClose } = setup();
    fireEvent.keyDown(window, { key: "Enter" });
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByTestId("mobile-right-panel-picker-backdrop"));
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
