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
