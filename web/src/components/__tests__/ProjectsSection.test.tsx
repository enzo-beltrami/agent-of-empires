// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";

import { ProjectsSection } from "../ProjectsSection";
import { SidebarCompactContext } from "../../lib/sidebarCompact";
import type { ProjectInfo, SavedProject } from "../../lib/types";

afterEach(cleanup);

function emptyProject(path: string, over: Partial<SavedProject> = {}): SavedProject {
  const name = path.split("/").pop() ?? path;
  const registration: ProjectInfo = { name, path, scope: "global" };
  return { path, displayName: name, registrations: [registration], ...over };
}

function renderSection(props: Partial<Parameters<typeof ProjectsSection>[0]> = {}) {
  const handlers = {
    onCreateSession: vi.fn(),
    onAddProject: vi.fn(),
    onEditProject: vi.fn(),
    onRemoveProject: vi.fn(),
  };
  render(
    <ProjectsSection projects={[emptyProject("/work/alpha")]} query="" offline={false} {...handlers} {...props} />,
  );
  return handlers;
}

function longPress(release: (row: HTMLElement) => void) {
  vi.useFakeTimers();
  try {
    renderSection();
    const row = screen.getByTestId("sidebar-project-row");
    fireEvent.touchStart(row, { touches: [{ clientX: 10, clientY: 10 }] });
    release(row);
    act(() => {
      vi.advanceTimersByTime(500);
    });
    return screen.queryByTestId("sidebar-project-context-menu");
  } finally {
    vi.useRealTimers();
    cleanup();
  }
}

describe("ProjectsSection", () => {
  it("renders the row with its base branch, adds a project, and starts a session", () => {
    const h = renderSection({
      projects: [
        emptyProject("/work/alpha", {
          registrations: [{ name: "alpha", path: "/work/alpha", scope: "global", default_base_branch: "develop" }],
        }),
      ],
    });
    expect(screen.getByText("alpha")).toBeTruthy();
    expect(screen.getByText(/develop/)).toBeTruthy();
    fireEvent.click(screen.getByTestId("sidebar-projects-add"));
    expect(h.onAddProject).toHaveBeenCalled();
    fireEvent.click(screen.getByTitle("New session in alpha"));
    expect(h.onCreateSession).toHaveBeenCalledWith("/work/alpha");
  });

  it("hides add and disables create for read-only viewers", () => {
    const h = renderSection({ readOnly: true });
    expect(screen.queryByTestId("sidebar-projects-add")).toBeNull();
    fireEvent.click(screen.getByTitle("New session in alpha"));
    expect(h.onCreateSession).not.toHaveBeenCalled();
  });

  it("opens the context menu on right-click and fires edit / remove", () => {
    const h = renderSection();
    fireEvent.contextMenu(screen.getByTestId("sidebar-project-row"));
    fireEvent.click(screen.getByTestId("sidebar-project-context-menu-edit"));
    expect(h.onEditProject).toHaveBeenCalledWith(expect.objectContaining({ path: "/work/alpha" }));

    fireEvent.keyDown(screen.getByTestId("sidebar-project-row"), { key: "F10", shiftKey: true });
    fireEvent.click(screen.getByTestId("sidebar-project-context-menu-remove"));
    expect(h.onRemoveProject).toHaveBeenCalledWith(expect.objectContaining({ path: "/work/alpha" }));
  });

  it("opens the context menu on a long-press unless the gesture ends or moves first", () => {
    expect(longPress(() => {})).not.toBeNull();
    // Well past LONG_PRESS_SLOP_PX (8px): a deliberate drag, not a jittery hold.
    expect(longPress((row) => fireEvent.touchMove(row, { touches: [{ clientX: 100, clientY: 100 }] }))).toBeNull();
    expect(longPress((row) => fireEvent.touchEnd(row, { touches: [] }))).toBeNull();
    expect(longPress((row) => fireEvent.touchCancel(row))).toBeNull();
  });

  it("caps the context menu with the dynamic viewport so its tail scrolls on iOS (#2870)", () => {
    renderSection();
    fireEvent.contextMenu(screen.getByTestId("sidebar-project-row"));
    const menu = screen.getByTestId("sidebar-project-context-menu");
    // `100vh` overshoots iOS Safari's visible viewport (dynamic toolbar), so the menu would never exceed its own
    // max-height and overflow-y-auto would never engage.
    expect(menu.style.maxHeight).toContain("dvh");
    expect(menu.className).toContain("overflow-y-auto");
  });

  it("counts the rows it lists, except in the rail", () => {
    for (const paths of [[], ["/work/alpha"], ["/work/alpha", "/work/beta"]]) {
      renderSection({ projects: paths.map((p) => emptyProject(p)) });
      const toggle = screen.getByTestId("sidebar-projects-toggle");
      expect(Array.from(toggle.querySelectorAll("span"), (span) => span.textContent)).toEqual([
        "Projects",
        `(${paths.length})`,
      ]);
      expect(screen.queryAllByTestId("sidebar-project-row")).toHaveLength(paths.length);
      cleanup();
    }
    // The rail has no room for the count.
    render(
      <SidebarCompactContext.Provider value={true}>
        <ProjectsSection
          projects={[emptyProject("/work/alpha")]}
          query=""
          offline={false}
          onCreateSession={vi.fn()}
          onAddProject={vi.fn()}
          onEditProject={vi.fn()}
          onRemoveProject={vi.fn()}
        />
      </SidebarCompactContext.Provider>,
    );
    const toggle = screen.getByTestId("sidebar-projects-toggle");
    expect(Array.from(toggle.querySelectorAll("span"), (span) => span.textContent)).toEqual(["Projects"]);
  });

  it("lists Scratch, which starts a scratch session and opens its settings, and filters it by name", () => {
    const onNewScratch = vi.fn();
    const onScratchSettings = vi.fn();
    renderSection({ onNewScratch, onScratchSettings });
    fireEvent.click(screen.getByTitle("New scratch session"));
    expect(onNewScratch).toHaveBeenCalled();
    fireEvent.contextMenu(screen.getByTestId("sidebar-scratch-row"));
    fireEvent.click(screen.getByTestId("sidebar-scratch-context-menu-settings"));
    expect(onScratchSettings).toHaveBeenCalled();
    cleanup();
    renderSection({ onNewScratch, query: "alpha" });
    expect(screen.queryByTestId("sidebar-scratch-row")).toBeNull();
    cleanup();
    renderSection({ onNewScratch, onScratchSettings, readOnly: true });
    fireEvent.click(screen.getByTitle("New scratch session"));
    expect(onNewScratch).toHaveBeenCalledTimes(1);
    expect(screen.queryByLabelText("New scratch session")).toBeNull();
  });

  it("shows the no-match and empty hints, and nothing when there is no way to add", () => {
    renderSection({ query: "zzz" });
    expect(screen.queryByTestId("sidebar-project-row")).toBeNull();
    expect(screen.getByText("No matching projects.")).toBeTruthy();
    cleanup();
    renderSection({ projects: [] });
    expect(screen.getByText(/No registered Projects\./)).toBeTruthy();
    cleanup();
    renderSection({ projects: [], readOnly: true });
    expect(screen.queryByTestId("sidebar-projects-section")).toBeNull();
  });
});
