// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { CoordinatorPane } from "../CoordinatorPane";
import * as api from "../../lib/api";
import { makeSession } from "./fixtures";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const coordinator = makeSession({ id: "asst", view: "structured" });
const renderSession = (s: { id: string }) => <div>chat {s.id}</div>;

describe("CoordinatorPane", () => {
  it("renders the ensured session, or a note when it is the main view", async () => {
    vi.spyOn(api, "ensureCoordinator").mockResolvedValue({ kind: "ok", sessionId: "asst" });
    const { rerender } = render(
      <CoordinatorPane sessions={[coordinator]} activeSessionId="other" renderSession={renderSession} />,
    );
    expect(await screen.findByText("chat asst")).toBeTruthy();
    rerender(<CoordinatorPane sessions={[coordinator]} activeSessionId="asst" renderSession={renderSession} />);
    expect(screen.getByText("The coordinator is open in the main view.")).toBeTruthy();
  });

  it("shows the server's refusal and retries on request", async () => {
    const ensure = vi
      .spyOn(api, "ensureCoordinator")
      .mockResolvedValueOnce({ kind: "error", message: "The coordinator needs the MCP server." })
      .mockResolvedValueOnce({ kind: "ok", sessionId: "asst" });
    render(<CoordinatorPane sessions={[coordinator]} activeSessionId={null} renderSession={renderSession} />);
    fireEvent.click(await screen.findByText("Try again"));
    expect(await screen.findByText("chat asst")).toBeTruthy();
    expect(screen.queryByText("The coordinator needs the MCP server.")).toBeNull();
    expect(ensure).toHaveBeenCalledTimes(2);
  });
});
