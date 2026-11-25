// @vitest-environment jsdom

import { describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";

import type { SessionResponse } from "../../lib/types";
import { makeSession as baseSession } from "./fixtures";

vi.mock("../TerminalSessionStack", () => ({
  TerminalSessionStack: () => <div data-testid="agent-terminal" />,
}));
vi.mock("../PairedTerminal", () => ({
  PairedShellPane: () => <div data-testid="paired-shell" />,
}));
vi.mock("../acp/StructuredView", () => ({
  StructuredView: () => <div data-testid="acp-view" />,
}));

import { MobileMainPane } from "../MobileMainPane";

const session = (overrides: Partial<SessionResponse> = {}) =>
  baseSession({ id: "s1", title: "t", project_path: "/tmp/t", status: "Running", ...overrides });

function setup(overrides: Partial<Parameters<typeof MobileMainPane>[0]> = {}) {
  const onBackToAgent = vi.fn();
  const props: Parameters<typeof MobileMainPane>[0] = {
    view: "agent",
    readOnly: false,
    onBackToAgent,
    canRestartAgent: false,
    shellMode: "host",
    onShellModeChange: vi.fn(),
    pairedMounted: false,
    activeSession: session(),
    activeSessionId: "s1",
    sessions: [session()],
    serverAbout: null,
    webSettings: {
      persistentTerminals: false,
      maxPersistentTerminals: 3,
      persistentStructuredViews: false,
      maxPersistentStructuredViews: 3,
    },
    ...overrides,
  };
  render(<MobileMainPane {...props} />);
  return { onBackToAgent };
}

describe("MobileMainPane", () => {
  it("renders the structured view for structured view sessions", async () => {
    const structured = session({ view: "structured" });
    setup({ view: "agent", activeSession: structured, sessions: [structured] });
    // StructuredView is lazy-loaded behind Suspense, so await its resolution.
    expect(await screen.findByTestId("acp-view")).toBeDefined();
  });

  it("shows the back header and returns to agent on click", () => {
    const { onBackToAgent } = setup({ view: "paired", pairedMounted: true });
    expect(screen.getByText("Paired terminal")).toBeDefined();
    fireEvent.click(screen.getByTestId("mobile-back-to-agent"));
    expect(onBackToAgent).toHaveBeenCalled();
  });

  it("mounts the paired shell only once activated", () => {
    setup({ view: "agent", pairedMounted: false });
    expect(screen.getByTestId("agent-terminal")).toBeDefined();
    expect(screen.queryByTestId("mobile-back-to-agent")).toBeNull();
    expect(screen.queryByTestId("paired-shell")).toBeNull();
    cleanup();
    setup({ view: "agent", pairedMounted: true });
    expect(screen.getByTestId("paired-shell")).toBeDefined();
  });
});
