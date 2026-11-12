export type ToastKind = "error" | "info";

export interface ToastApi {
  push: (message: string, kind?: ToastKind) => void;
  error: (message: string) => void;
  info: (message: string) => void;
  /** An error toast with one button, such as reopening what failed. */
  action: (message: string, label: string, onAction: () => void) => void;
}

interface ToastBus {
  handler: ToastApi | null;
}

export const toastBus: ToastBus = { handler: null };

export function reportError(message: string): void {
  toastBus.handler?.error(message);
}

export function reportInfo(message: string): void {
  toastBus.handler?.info(message);
}
