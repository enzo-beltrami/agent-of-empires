// @vitest-environment jsdom

import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";

import { MobileRightPanelPicker } from "../MobileRightPanelPicker";

function setup(overrides: Partial<Parameters<typeof MobileRightPanelPicker>[0]> = {}) {
  const onSelect = vi.fn();
  const onClose = vi.fn();
  render(
    <MobileRightPanelPicker
      open
      active="agent"
      sessionTitle="s1"
      onSelect={onSelect}
      onClose={onClose}
      {...overrides}
    />,
  );
  return { onSelect, onClose };
}

describe("MobileRightPanelPicker", () => {
  it("offers the Agent and its paired terminal, marking and selecting them", () => {
    const { onSelect } = setup({ active: "paired" });
    expect(screen.getAllByRole("button").map((b) => b.dataset.testid)).toEqual([
      "mobile-right-panel-pick-agent",
      "mobile-right-panel-pick-paired",
    ]);
    expect(screen.getByTestId("mobile-right-panel-pick-paired").getAttribute("aria-current")).toBe("true");
    fireEvent.click(screen.getByTestId("mobile-right-panel-pick-agent"));
    expect(onSelect).toHaveBeenCalledWith("agent");
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
