// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CoordinatorPanel } from "../CoordinatorPanel";
import * as api from "../../lib/api";

const row = (id: string, status: string) => ({ id, title: `t-${id}`, project_path: `/code/${id}`, status });

beforeEach(() => {
  vi.spyOn(api, "fetchCoordinatorSummary").mockResolvedValue({
    wip: [row("a", "running"), row("b", "running")],
    needs_action: [row("c", "waiting")],
    idle: [],
  });
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("CoordinatorPanel", () => {
  it("counts sessions per bucket and opens one from its list", async () => {
    vi.spyOn(api, "fetchDeploys").mockResolvedValue([]);
    const onOpen = vi.fn();
    render(<CoordinatorPanel onOpenSession={onOpen} />);
    await waitFor(() => expect(screen.getByTestId("count-wip").textContent).toBe("2"));
    expect(screen.getByTestId("count-needs_action").textContent).toBe("1");
    expect(screen.getByTestId("count-idle").textContent).toBe("0");

    fireEvent.click(screen.getByText("t-c"));
    expect(onOpen).toHaveBeenCalledWith("c");
    fireEvent.click(screen.getByText("WIP"));
    expect(screen.getByText("t-a")).toBeTruthy();
    expect(screen.queryByText("t-c")).toBeNull();
  });

  it("adds a pull request and marks it deployed", async () => {
    const pending = { id: "d1", url: "https://github.com/o/streambox/pull/42", added_at: "2026-11-11T00:00:00Z" };
    const fetchDeploys = vi.spyOn(api, "fetchDeploys").mockResolvedValueOnce([]).mockResolvedValue([pending]);
    const add = vi.spyOn(api, "addDeploy").mockResolvedValue({ ok: true, deploy: pending });
    const mark = vi.spyOn(api, "setDeployDeployed").mockResolvedValue(true);
    render(<CoordinatorPanel onOpenSession={vi.fn()} />);
    expect(await screen.findByText("Nothing waiting to deploy.")).toBeTruthy();

    fireEvent.change(screen.getByLabelText("Pull request URL"), { target: { value: pending.url } });
    fireEvent.click(screen.getByText("Add"));
    expect(await screen.findByText("streambox#42")).toBeTruthy();
    expect(add).toHaveBeenCalledWith(pending.url);
    expect(fetchDeploys.mock.calls.length).toBeGreaterThan(1);

    fireEvent.click(screen.getByLabelText("Mark streambox#42 deployed"));
    await waitFor(() => expect(mark).toHaveBeenCalledWith("d1", true));
  });

  it("shows why a pull request was not added", async () => {
    vi.spyOn(api, "fetchDeploys").mockResolvedValue([]);
    vi.spyOn(api, "addDeploy").mockResolvedValue({ ok: false, message: "The pull request URL is empty" });
    render(<CoordinatorPanel onOpenSession={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("Pull request URL"), { target: { value: "x" } });
    fireEvent.click(screen.getByText("Add"));
    expect(await screen.findByText("The pull request URL is empty")).toBeTruthy();
  });
});
