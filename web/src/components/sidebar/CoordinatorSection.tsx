import { Sparkles } from "lucide-react";
import type { SidebarGroup, SidebarWorkspaceView } from "../../lib/sidebarGroups";
import { useSidebarCompact } from "../../lib/sidebarCompact";
import { SessionRow } from "./SessionRow";

/** The coordinator's rows, pinned above every Group in its own tint; not collapsible, draggable or filtered. */
export function CoordinatorSection({
  group,
  rowProps,
}: {
  group: SidebarGroup;
  rowProps: (v: SidebarWorkspaceView) => Omit<React.ComponentProps<typeof SessionRow>, "indented">;
}) {
  const compact = useSidebarCompact();
  return (
    <section
      aria-label="Coordinator"
      data-testid="sidebar-coordinator"
      className="border-b border-surface-700/60 border-l-2 border-l-brand-500/60 bg-brand-500/10"
    >
      <div
        className={`flex items-center gap-2 pt-2 pb-1 text-[11px] font-mono uppercase text-brand-400 ${
          compact ? "px-2" : "px-3 tracking-wider"
        }`}
      >
        <Sparkles className="h-3.5 w-3.5 shrink-0" aria-hidden />
        {!compact && <span className="truncate">Coordinator</span>}
      </div>
      {group.workspaces.map((v) => (
        <SessionRow key={v.key} {...rowProps(v)} indented />
      ))}
    </section>
  );
}
