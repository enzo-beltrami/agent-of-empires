import { createElement, type ComponentType } from "react";
import type { LucideIcon } from "lucide-react";

interface Props {
  icon: LucideIcon;
  className: string;
  testId?: string;
}

/** The activity-bar/dock-tab icon for one pane. */
export function PaneIcon({ icon, className, testId }: Props) {
  // `data-testid` isn't in LucideProps; widen to a generic component type
  // rather than dropping the attribute.
  return createElement(icon as ComponentType<Record<string, unknown>>, {
    className,
    "aria-hidden": true,
    "data-testid": testId,
  });
}
