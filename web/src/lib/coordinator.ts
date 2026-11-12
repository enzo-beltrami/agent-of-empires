import type { Deploy } from "./api";

// The coordinator's directory is `<app dir>/coordinator`; see `src/session/coordinator.rs`.
const COORDINATOR_DIR_RE = /[\\/]\.?agent-of-empires(-dev)?[\\/]coordinator[\\/]?$/;

export function isCoordinatorPath(path: string | null | undefined): boolean {
  return !!path && COORDINATOR_DIR_RE.test(path);
}

/** `owner/repo/pull/12` shows as `repo#12`; anything else as the URL. */
export function deployLabel(deploy: Pick<Deploy, "url" | "title">): string {
  if (deploy.title) return deploy.title;
  const m = /\/([^/]+)(?:\/-)?\/(?:pull|merge_requests)\/(\d+)/.exec(deploy.url);
  return m ? `${m[1]}#${m[2]}` : deploy.url;
}

/** A ticket key shows as itself; a URL shows as its last path segment and links to it. */
export function ticketLabel(ticket: string): { text: string; href?: string } {
  if (!/^https?:\/\//.test(ticket)) return { text: ticket };
  const last = ticket
    .replace(/[?#].*$/, "")
    .split("/")
    .filter(Boolean)
    .pop();
  return { text: last ?? ticket, href: ticket };
}

/** Pending first, newest added first; then deployed, grouped by local deploy day, newest day first. */
export function deployGroups(deploys: Deploy[], now: Date): { label: string; deploys: Deploy[] }[] {
  const day = (iso: string) => {
    const d = new Date(iso);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  };
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  // Calendar arithmetic, not 24 h, so a DST change keeps "yesterday" right.
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1).getTime();
  const pending = deploys.filter((d) => !d.deployed_at).sort((a, b) => b.added_at.localeCompare(a.added_at));
  const byDay = new Map<number, Deploy[]>();
  for (const d of deploys.filter((d) => d.deployed_at).sort((a, b) => b.deployed_at!.localeCompare(a.deployed_at!))) {
    const key = day(d.deployed_at!);
    byDay.set(key, [...(byDay.get(key) ?? []), d]);
  }
  const label = (key: number) =>
    key === today
      ? "Deployed today"
      : key === yesterday
        ? "Deployed yesterday"
        : `Deployed ${new Date(key).toLocaleDateString(undefined, { weekday: "short", month: "short", day: "numeric" })}`;
  const groups = pending.length > 0 ? [{ label: "Pending", deploys: pending }] : [];
  for (const key of [...byDay.keys()].sort((a, b) => b - a))
    groups.push({ label: label(key), deploys: byDay.get(key)! });
  return groups;
}
