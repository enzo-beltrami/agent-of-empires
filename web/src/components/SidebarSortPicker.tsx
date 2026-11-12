import { useRef, useState } from "react";
import { Check, Clock, ListOrdered, Siren } from "lucide-react";
import type { SidebarSortMode } from "../lib/sidebarSort";
import { Tooltip } from "./Tooltip";
import { useOutsideDismiss } from "./useOutsideDismiss";

interface ModeSpec {
  mode: SidebarSortMode;
  label: string;
  Icon: typeof Clock;
}

const MODES: readonly ModeSpec[] = [
  { mode: "manual", label: "Manual", Icon: ListOrdered },
  { mode: "lastActivity", label: "Last activity", Icon: Clock },
  { mode: "attention", label: "Attention", Icon: Siren },
];

const BUILTIN_TOOLTIP: Record<SidebarSortMode, string> = {
  manual: "Sort: manual, drag enabled",
  lastActivity: "Sort: last activity, drag disabled",
  attention: "Sort: attention, drag disabled",
};

interface Props {
  sortMode: SidebarSortMode;
  onSortModeChange: (mode: SidebarSortMode) => void;
}

/** Sort mode dropdown. */
export function SidebarSortPicker({ sortMode, onSortModeChange }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useOutsideDismiss(open, [ref], () => setOpen(false));

  const active = MODES.find((m) => m.mode === sortMode) ?? MODES[0]!;
  const ActiveIcon = active.Icon;
  const computed = sortMode !== "manual";

  return (
    <div ref={ref} className="relative">
      <Tooltip text={BUILTIN_TOOLTIP[sortMode]}>
        <button
          onClick={() => setOpen((o) => !o)}
          aria-haspopup="menu"
          aria-expanded={open}
          aria-label={`Sort sessions, current: ${active.label}`}
          data-testid="sidebar-sort-toggle"
          data-sort-mode={sortMode}
          className={`w-8 h-8 flex items-center justify-center cursor-pointer rounded-md transition-colors ${
            computed ? "text-brand-500" : "text-text-dim hover:text-text-secondary"
          }`}
        >
          <ActiveIcon className="h-3.5 w-3.5" />
        </button>
      </Tooltip>

      {open && (
        <div
          role="menu"
          data-testid="sidebar-sort-menu"
          className="absolute right-0 top-full mt-1 min-w-[160px] bg-surface-800 border border-surface-700/50 rounded-md shadow-xl py-1 z-50 animate-fade-in"
        >
          {MODES.map(({ mode, label, Icon }) => (
            <SortOption
              key={mode}
              testId={`sidebar-sort-option-${mode}`}
              Icon={Icon}
              label={label}
              selected={mode === sortMode}
              onSelect={(selected) => {
                setOpen(false);
                if (!selected) onSortModeChange(mode);
              }}
            />
          ))}
        </div>
      )}
    </div>
  );
}

function SortOption({
  testId,
  Icon,
  label,
  selected,
  onSelect,
}: {
  testId: string;
  Icon: typeof Clock;
  label: string;
  selected: boolean;
  onSelect: (selected: boolean) => void;
}) {
  return (
    <button
      role="menuitemradio"
      aria-checked={selected}
      data-testid={testId}
      onClick={() => onSelect(selected)}
      className={`w-full flex items-center gap-2 px-3 py-1.5 text-sm cursor-pointer hover:bg-surface-700/60 ${
        selected ? "text-brand-500" : "text-text-secondary hover:text-text-primary"
      }`}
    >
      <Icon className="h-3.5 w-3.5 shrink-0" />
      <span className="flex-1 text-left">{label}</span>
      {selected && <Check className="h-3.5 w-3.5 shrink-0" />}
    </button>
  );
}
