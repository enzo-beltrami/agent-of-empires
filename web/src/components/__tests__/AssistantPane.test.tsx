// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { AssistantPane } from "../AssistantPane";
import * as api from "../../lib/api";
import { makeSession } from "./fixtures";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

const assistant = makeSession({ id: "asst", view: "structured" });
const renderSession = (s: { id: string }) => <div>chat {s.id}</div>;

describe("AssistantPane", () => {
  it("renders the ensured session, or a note when it is the main view", async () => {
    vi.spyOn(api, "ensureAssistant").mockResolvedValue({ kind: "ok", sessionId: "asst" });
    const { rerender } = render(
      <AssistantPane sessions={[assistant]} activeSessionId="other" renderSession={renderSession} />,
    );
    expect(await screen.findByText("chat asst")).toBeTruthy();
    rerender(<AssistantPane sessions={[assistant]} activeSessionId="asst" renderSession={renderSession} />);
    expect(screen.getByText("The assistant is open in the main view.")).toBeTruthy();
  });

  it("shows the server's refusal and retries on request", async () => {
    const ensure = vi
      .spyOn(api, "ensureAssistant")
      .mockResolvedValueOnce({ kind: "error", message: "The assistant needs the MCP server." })
      .mockResolvedValueOnce({ kind: "ok", sessionId: "asst" });
    render(<AssistantPane sessions={[assistant]} activeSessionId={null} renderSession={renderSession} />);
    fireEvent.click(await screen.findByText("Try again"));
    expect(await screen.findByText("chat asst")).toBeTruthy();
    expect(screen.queryByText("The assistant needs the MCP server.")).toBeNull();
    expect(ensure).toHaveBeenCalledTimes(2);
  });
});
