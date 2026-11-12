import type { SidebarAxis } from "../../lib/sidebarAxis";
import type { SidebarSortMode } from "../../lib/sidebarSort";
import { OFFLINE_TITLE } from "../../lib/connectionState";
import { SidebarAxisPicker } from "../SidebarAxisPicker";
import { SidebarSortPicker } from "../SidebarSortPicker";
import { Tooltip } from "../Tooltip";
import { DISABLED_ICON_BUTTON, TOOLBAR_BUTTON, TOOLBAR_TINT } from "./styles";
import { StrokeIcon } from "../icons";
import { SidebarLegend } from "./SidebarLegend";

/** Heading above the list; the picker labels its axes itself. */
const AXIS_HEADING: Record<SidebarAxis, string> = {
  repo: "Sessions",
  org: "Sessions",
  group: "Groups",
  "repo+group": "Sessions",
  status: "Status",
};

export function SidebarToolbar({
  compact,
  axis,
  onAxisChange,
  sortMode,
  onSortModeChange,
  filterOpen,
  onToggleFilter,
  offline,
  onNew,
  onToggleCompact,
  onClose,
}: {
  compact: boolean;
  axis: SidebarAxis;
  onAxisChange: (axis: SidebarAxis) => void;
  sortMode: SidebarSortMode;
  onSortModeChange: (mode: SidebarSortMode) => void;
  filterOpen: boolean;
  onToggleFilter: () => void;
  offline: boolean;
  onNew: () => void;
  onToggleCompact: () => void;
  onClose: () => void;
}) {
  const axisHeading = AXIS_HEADING[axis];
  return (
    <div className={`${compact ? "px-1" : "px-3"} pt-3 pb-1 flex items-center`}>
      {compact ? (
        <span className="flex-1" />
      ) : (
        <>
          <span data-testid="sidebar-axis-heading" className="text-sm text-text-muted flex-1">
            {axisHeading}
          </span>
          <SidebarAxisPicker axis={axis} onAxisChange={onAxisChange} />
          <SidebarSortPicker sortMode={sortMode} onSortModeChange={onSortModeChange} />
          <Tooltip text="Filter">
            <button
              onClick={onToggleFilter}
              className={`${TOOLBAR_BUTTON} ${TOOLBAR_TINT(filterOpen, "text-text-secondary")}`}
              aria-label="Filter sessions"
            >
              <StrokeIcon size={14} strokeWidth="2">
                <polygon points="22 3 2 3 10 12.46 10 19 14 21 14 12.46 22 3" />
              </StrokeIcon>
            </button>
          </Tooltip>
          <Tooltip text={offline ? OFFLINE_TITLE : "New project session"}>
            <button
              onClick={onNew}
              disabled={offline}
              className={`w-8 h-8 flex items-center justify-center text-text-muted hover:text-text-secondary hover:bg-surface-800 cursor-pointer rounded-md transition-colors ${DISABLED_ICON_BUTTON}`}
              aria-label="New project session"
            >
              <StrokeIcon size={16} strokeWidth="1.5">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
                <line x1="12" y1="11" x2="12" y2="17" />
                <line x1="9" y1="14" x2="15" y2="14" />
              </StrokeIcon>
            </button>
          </Tooltip>
        </>
      )}
      <SidebarLegend />
      <Tooltip text={compact ? "Expand sidebar" : "Compact sidebar"}>
        <button
          onClick={onToggleCompact}
          aria-pressed={compact}
          aria-label={compact ? "Expand sidebar" : "Compact sidebar"}
          data-testid="sidebar-compact-toggle"
          className={`${TOOLBAR_BUTTON} ${TOOLBAR_TINT(compact)}`}
        >
          <StrokeIcon size={14} strokeWidth="2">
            <rect x="3" y="3" width="18" height="18" rx="2" />
            <line x1="9" y1="3" x2="9" y2="21" />
          </StrokeIcon>
        </button>
      </Tooltip>
      <button
        onClick={onClose}
        className="md:hidden w-8 h-8 flex items-center justify-center text-text-dim hover:text-text-secondary cursor-pointer rounded-md hover:bg-surface-800 ml-1"
      >
        &times;
      </button>
    </div>
  );
}
