import { memo, useRef } from "react";
import type { ProjectInfo, SavedProject } from "../lib/types";
import { useSidebarCompact } from "../lib/sidebarCompact";
import { ContextMenu, MenuItem, MenuSeparator } from "./ContextMenu";
import { FoldChevron, PlusIcon } from "./icons";
import { useContextMenu } from "./useContextMenu";
import { useLongPress } from "./sidebar/useLongPress";
import { usePersistedFlag } from "./usePersistedFlag";

interface ProjectsSectionProps {
  // Registered projects to list, one entry per path (`savedProjects`).
  projects: SavedProject[];
  // Lowercased, trimmed filter query; empty means no filter.
  query: string;
  readOnly?: boolean;
  offline: boolean;
  onCreateSession: (repoPath: string) => void;
  onAddProject: () => void;
  onEditProject: (project: ProjectInfo) => void;
  onRemoveProject: (project: SavedProject) => void;
  /** A Scratch row that starts a Scratch session; omitted hides it. */
  onNewScratch?: () => void;
  onScratchSettings?: () => void;
}

export function ProjectsSection({
  projects,
  query,
  readOnly,
  offline,
  onCreateSession,
  onAddProject,
  onEditProject,
  onRemoveProject,
  onNewScratch,
  onScratchSettings,
}: ProjectsSectionProps) {
  const [expanded, toggle] = usePersistedFlag("aoe-projects-section-expanded", true);
  const compact = useSidebarCompact();

  const visible = query
    ? projects.filter((p) => p.displayName.toLowerCase().includes(query) || p.path.toLowerCase().includes(query))
    : projects;
  const showScratch = !!onNewScratch && (!query || "scratch".includes(query));

  // With CRUD available the header stays so Add is reachable with zero projects.
  const canAdd = !readOnly && !offline;
  if (visible.length === 0 && !canAdd && !showScratch) return null;

  return (
    <div data-testid="sidebar-projects-section">
      <div className="w-full flex items-center gap-2 border-t border-surface-800/60">
        <button
          onClick={toggle}
          data-testid="sidebar-projects-toggle"
          aria-expanded={expanded}
          className={`flex-1 min-w-0 flex items-center gap-2 py-1.5 text-[11px] font-mono uppercase text-text-muted hover:text-text-secondary hover:bg-surface-800/40 cursor-pointer transition-colors ${
            compact ? "px-2" : "px-3 tracking-wider"
          }`}
        >
          <FoldChevron className={`shrink-0 transition-transform duration-75 ${expanded ? "" : "-rotate-90"}`} />
          {/* A clipped count is exactly what looks broken, so the label truncates and the count never
              does; the rail has room for neither (#2288). */}
          <span className="truncate">Projects</span>
          {!compact && <span className="shrink-0">({visible.length})</span>}
        </button>
        {canAdd && !compact && (
          <button
            onClick={onAddProject}
            data-testid="sidebar-projects-add"
            title="Add project"
            aria-label="Add project"
            className="shrink-0 w-7 h-7 mr-1 flex items-center justify-center text-text-muted hover:text-text-secondary hover:bg-surface-800 cursor-pointer rounded-md transition-colors"
          >
            <PlusIcon size={14} />
          </button>
        )}
      </div>
      {expanded &&
        visible.map((project) => (
          <ProjectRow
            key={project.path}
            project={project}
            readOnly={readOnly}
            offline={offline}
            onCreateSession={onCreateSession}
            onEditProject={onEditProject}
            onRemoveProject={onRemoveProject}
          />
        ))}
      {expanded && showScratch && (
        <ScratchRow readOnly={readOnly} offline={offline} onNew={onNewScratch} onSettings={onScratchSettings} />
      )}
      {expanded && visible.length === 0 && !compact && (
        <p className="px-3 py-2 text-[12px] text-text-dim">
          {query ? "No matching projects." : "No registered Projects. Add one to keep a repo handy without a session."}
        </p>
      )}
    </div>
  );
}

const PROJECT_ROW_CLASS =
  "group flex items-center gap-2 px-3 py-1.5 text-text-secondary hover:bg-surface-800/50 transition-colors focus:outline-none focus:ring-2 focus:ring-brand-600 select-none [-webkit-touch-callout:none]";

const ProjectRow = memo(function ProjectRow({
  project,
  readOnly,
  offline,
  onCreateSession,
  onEditProject,
  onRemoveProject,
}: {
  project: SavedProject;
  readOnly?: boolean;
  offline: boolean;
  onCreateSession: (repoPath: string) => void;
  onEditProject: (project: ProjectInfo) => void;
  onRemoveProject: (project: SavedProject) => void;
}) {
  const openedAtRef = useRef(0);
  const { menu, menuRef, openMenu, closeMenu } = useContextMenu<{ x: number; y: number }>(openedAtRef);
  const baseBranch = project.registrations[0]?.default_base_branch;
  const canModify = !readOnly && !offline;
  const longPress = useLongPress(canModify, (x, y) => {
    openedAtRef.current = Date.now();
    openMenu({ x, y });
  });

  return (
    <>
      <div
        data-testid="sidebar-project-row"
        data-repo-path={project.path}
        tabIndex={canModify ? 0 : undefined}
        aria-haspopup={canModify ? "menu" : undefined}
        aria-label={canModify ? `Project actions for ${project.displayName}` : undefined}
        onContextMenu={
          canModify
            ? (e) => {
                e.preventDefault();
                openMenu({ x: e.clientX, y: e.clientY });
              }
            : undefined
        }
        {...longPress.handlers}
        onKeyDown={
          canModify
            ? (e) => {
                // Only the row itself, not the inner New-session button.
                if (e.target !== e.currentTarget) return;
                if (e.key !== "ContextMenu" && !(e.shiftKey && e.key === "F10")) return;
                e.preventDefault();
                const rect = e.currentTarget.getBoundingClientRect();
                openMenu({ x: rect.left + 12, y: rect.bottom + 4 });
              }
            : undefined
        }
        className={PROJECT_ROW_CLASS}
      >
        <button
          onClick={() => canModify && onCreateSession(project.path)}
          disabled={!canModify}
          title={`New session in ${project.displayName}`}
          className="min-w-0 flex-1 text-left cursor-pointer disabled:cursor-not-allowed"
        >
          <span className="block truncate text-[13px] md:text-[14px] font-mono text-text-primary">
            {project.displayName}
          </span>
          <span className="block truncate text-[11px] text-text-dim" title={project.path}>
            {project.path}
            {baseBranch ? ` · ${baseBranch}` : ""}
          </span>
        </button>
        {canModify && (
          <button
            onClick={() => onCreateSession(project.path)}
            title="New session"
            aria-label={`New session in ${project.displayName}`}
            className="shrink-0 w-6 h-6 flex items-center justify-center text-text-dim opacity-0 group-hover:opacity-100 hover:text-text-primary hover:bg-surface-700/50 cursor-pointer rounded transition"
          >
            <PlusIcon size={13} />
          </button>
        )}
      </div>

      {canModify && menu && (
        <ContextMenu menu={menu} menuRef={menuRef} testId="sidebar-project-context-menu">
          {project.registrations.map((reg) => (
            <MenuItem
              key={`${reg.scope}:${reg.path}`}
              onClick={() => {
                closeMenu();
                onEditProject(reg);
              }}
              testId="sidebar-project-context-menu-edit"
            >
              {project.registrations.length > 1 ? `Project settings (${reg.scope})` : "Project settings"}
            </MenuItem>
          ))}
          <MenuSeparator />
          <MenuItem
            onClick={() => {
              closeMenu();
              onRemoveProject(project);
            }}
            testId="sidebar-project-context-menu-remove"
          >
            Remove project
          </MenuItem>
        </ContextMenu>
      )}
    </>
  );
});

/** Scratch sessions run in a directory AoE owns, so the row starts one and holds their settings. */
function ScratchRow({
  readOnly,
  offline,
  onNew,
  onSettings,
}: {
  readOnly?: boolean;
  offline: boolean;
  onNew: () => void;
  onSettings?: () => void;
}) {
  const openedAtRef = useRef(0);
  const { menu, menuRef, openMenu, closeMenu } = useContextMenu<{ x: number; y: number }>(openedAtRef);
  const canModify = !readOnly && !offline;
  const hasMenu = canModify && !!onSettings;
  const longPress = useLongPress(hasMenu, (x, y) => {
    openedAtRef.current = Date.now();
    openMenu({ x, y });
  });
  return (
    <>
      <div
        data-testid="sidebar-scratch-row"
        onContextMenu={
          hasMenu
            ? (e) => {
                e.preventDefault();
                openMenu({ x: e.clientX, y: e.clientY });
              }
            : undefined
        }
        {...longPress.handlers}
        className={PROJECT_ROW_CLASS}
      >
        <button
          onClick={() => canModify && onNew()}
          disabled={!canModify}
          title="New scratch session"
          className="min-w-0 flex-1 text-left cursor-pointer disabled:cursor-not-allowed"
        >
          <span className="block truncate text-[13px] md:text-[14px] font-mono text-text-primary">Scratch</span>
          <span className="block truncate text-[11px] text-text-dim">Temporary directory, no project</span>
        </button>
        {canModify && (
          <button
            onClick={onNew}
            title="New session"
            aria-label="New scratch session"
            className="shrink-0 w-6 h-6 flex items-center justify-center text-text-dim opacity-0 group-hover:opacity-100 hover:text-text-primary hover:bg-surface-700/50 cursor-pointer rounded transition"
          >
            <PlusIcon size={13} />
          </button>
        )}
      </div>
      {hasMenu && menu && (
        <ContextMenu menu={menu} menuRef={menuRef} testId="sidebar-scratch-context-menu">
          <MenuItem
            onClick={() => {
              closeMenu();
              onSettings?.();
            }}
            testId="sidebar-scratch-context-menu-settings"
          >
            Scratch settings
          </MenuItem>
        </ContextMenu>
      )}
    </>
  );
}
