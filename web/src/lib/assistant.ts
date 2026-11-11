// The assistant's directory is `<app dir>/assistant`; see `src/session/assistant.rs`.
const ASSISTANT_DIR_RE = /[\\/]\.?agent-of-empires(-dev)?[\\/]assistant[\\/]?$/;

export function isAssistantPath(path: string | null | undefined): boolean {
  return !!path && ASSISTANT_DIR_RE.test(path);
}
