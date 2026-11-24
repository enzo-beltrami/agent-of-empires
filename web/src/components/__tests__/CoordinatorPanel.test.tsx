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
  vi.spyOn(api, "fetchCoordinatorNotes").mockResolvedValue([]);
});

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const pending: api.Deploy = {
  id: "d1",
  url: "https://github.com/o/streambox/pull/42",
  title: "Fix login",
  session_id: "s1",
  session_title: "PI-1921",
  ticket: "https://acme.atlassian.net/browse/PI-1921",
  note: "Retries the token refresh",
  tested: false,
  added_at: "2026-11-11T10:00:00Z",
};

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

  it("switches tabs by click and arrow key, with the pending count on the Deploys tab", async () => {
    vi.spyOn(api, "fetchDeploys").mockResolvedValue([pending]);
    render(<CoordinatorPanel onOpenSession={vi.fn()} />);
    const sessions = screen.getByRole("tab", { name: "Sessions" });
    expect(sessions.getAttribute("aria-selected")).toBe("true");
    expect(await screen.findByTestId("deploys-pending-count")).toBeTruthy();

    fireEvent.keyDown(sessions, { key: "ArrowRight" });
    const deploys = screen.getByRole("tab", { name: /Deploys/ });
    expect(deploys.getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(deploys);
    expect(screen.queryByRole("tabpanel", { name: "Sessions" })).toBeNull();
  });

  it("shows each pull request's details, with no way to add one, and toggles deployed and tested", async () => {
    vi.spyOn(api, "fetchDeploys").mockResolvedValue([pending]);
    const mark = vi.spyOn(api, "updateDeploy").mockResolvedValue(true);
    const onOpen = vi.fn();
    render(<CoordinatorPanel onOpenSession={onOpen} />);
    fireEvent.click(screen.getByRole("tab", { name: /Deploys/ }));

    expect(await screen.findByText("Fix login")).toBeTruthy();
    expect(screen.getByText("streambox#42")).toBeTruthy();
    expect(screen.getByText("PI-1921", { selector: "a" }).getAttribute("href")).toBe(pending.ticket);
    expect(screen.getByText("Retries the token refresh")).toBeTruthy();
    expect(screen.getByText(/^Pending/)).toBeTruthy();
    expect(screen.queryByRole("textbox")).toBeNull();

    fireEvent.click(screen.getByText("PI-1921", { selector: "button" }));
    expect(onOpen).toHaveBeenCalledWith("s1");
    fireEvent.click(screen.getByLabelText("Mark Fix login deployed"));
    await waitFor(() => expect(mark).toHaveBeenCalledWith("d1", { deployed: true }));
    fireEvent.click(screen.getByRole("button", { name: "Untested: mark Fix login tested" }));
    await waitFor(() => expect(mark).toHaveBeenCalledWith("d1", { tested: true }));
  });

  const note = (id: string, text: string, created: string, updated = created): api.CoordinatorNote => ({
    id,
    text,
    created_at: created,
    updated_at: updated,
  });

  it("lists notes newest first and adds one with Ctrl+Enter", async () => {
    vi.spyOn(api, "fetchDeploys").mockResolvedValue([]);
    const notes = [
      note("n1", "ship Friday", "2026-11-12T10:00:00Z"),
      note("n2", "ask about tests", "2026-11-13T10:00:00Z"),
    ];
    vi.spyOn(api, "fetchCoordinatorNotes")
      .mockResolvedValueOnce(notes)
      .mockResolvedValue([...notes, note("n3", "call Ana", "2026-11-13T11:00:00Z")]);
    const add = vi.spyOn(api, "addCoordinatorNote").mockResolvedValue({ ok: true });
    render(<CoordinatorPanel onOpenSession={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "Notes" }));
    await waitFor(() => expect(screen.getAllByTestId("note-card")).toHaveLength(2));
    expect(screen.getAllByTestId("note-card")[0]!.textContent).toContain("ask about tests");

    const box = screen.getByLabelText("New note");
    fireEvent.change(box, { target: { value: "call Ana" } });
    fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
    await waitFor(() => expect(add).toHaveBeenCalledWith("call Ana"));
    await waitFor(() => expect(screen.getAllByTestId("note-card")).toHaveLength(3));
    expect((box as HTMLTextAreaElement).value).toBe("");
  });

  it("edits one note, cancels with Escape, and deletes only after confirming", async () => {
    vi.spyOn(api, "fetchDeploys").mockResolvedValue([]);
    vi.spyOn(api, "fetchCoordinatorNotes").mockResolvedValue([note("n1", "ship Friday", "2026-11-12T10:00:00Z")]);
    const update = vi.spyOn(api, "updateCoordinatorNote").mockResolvedValue({ ok: true });
    const remove = vi.spyOn(api, "removeCoordinatorNote").mockResolvedValue(true);
    render(<CoordinatorPanel onOpenSession={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "Notes" }));
    await screen.findByText("ship Friday");

    fireEvent.click(screen.getByText("Edit"));
    fireEvent.change(screen.getByLabelText("Edit note"), { target: { value: "ship Monday" } });
    fireEvent.keyDown(screen.getByLabelText("Edit note"), { key: "Escape" });
    expect(screen.queryByLabelText("Edit note")).toBeNull();
    expect(update).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("Edit"));
    fireEvent.change(screen.getByLabelText("Edit note"), { target: { value: "ship Monday" } });
    fireEvent.click(screen.getByText("Save"));
    await waitFor(() => expect(update).toHaveBeenCalledWith("n1", "ship Monday"));

    fireEvent.click(screen.getByText("Delete"));
    expect(remove).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Confirm delete"));
    await waitFor(() => expect(remove).toHaveBeenCalledWith("n1"));
  });

  it("shows why a note was not added", async () => {
    vi.spyOn(api, "fetchDeploys").mockResolvedValue([]);
    vi.spyOn(api, "addCoordinatorNote").mockResolvedValue({ ok: false, message: "The note is empty" });
    render(<CoordinatorPanel onOpenSession={vi.fn()} />);
    fireEvent.click(screen.getByRole("tab", { name: "Notes" }));
    fireEvent.change(screen.getByLabelText("New note"), { target: { value: "x" } });
    fireEvent.click(screen.getByText("Add note"));
    expect(await screen.findByText("The note is empty")).toBeTruthy();
  });
});
