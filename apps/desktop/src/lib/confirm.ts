import { create } from "zustand";

interface ConfirmRequest {
  title?: string;
  message: string;
  confirmLabel?: string;
  /** Red confirm button (default). */
  danger?: boolean;
  resolve: (ok: boolean) => void;
}

export const useConfirm = create<{ req: ConfirmRequest | null }>(() => ({ req: null }));

/** Ask in the app's own window. Resolves true when the user confirms. */
export function confirmDialog(message: string, opts: { title?: string; confirmLabel?: string; danger?: boolean } = {}): Promise<boolean> {
  return new Promise((resolve) => {
    // A newer question cancels the one still open.
    useConfirm.getState().req?.resolve(false);
    useConfirm.setState({ req: { message, ...opts, resolve } });
  });
}
