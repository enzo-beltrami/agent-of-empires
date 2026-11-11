// The coordinator's directory is `<app dir>/coordinator`; see `src/session/coordinator.rs`.
const COORDINATOR_DIR_RE = /[\\/]\.?agent-of-empires(-dev)?[\\/]coordinator[\\/]?$/;

export function isCoordinatorPath(path: string | null | undefined): boolean {
  return !!path && COORDINATOR_DIR_RE.test(path);
}
