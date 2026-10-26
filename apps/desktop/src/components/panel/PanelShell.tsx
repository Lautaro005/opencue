import { useState } from "react";
import { useTranslation } from "react-i18next";
import { AppWindow, Pin, PinOff, X } from "lucide-react";
import { api, isTauri } from "../../lib/api";
import { useStore } from "../../lib/store";
import { IconButton } from "../ui";
import { useResizable } from "../ui/Resizer";

declare global {
  interface Window {
    /** Set by the native side when this page is a floating window: which panel to show. */
    __FLOAT__?: { kind: "artifact" | "canvas" | "computer"; id: string };
  }
}

export const isFloating = typeof window !== "undefined" && !!window.__FLOAT__;

/** Open a panel in a window of its own and close the docked copy. */
export async function popOut(kind: "artifact" | "canvas" | "computer", id: string, title: string) {
  try {
    await api.openFloat(kind, id, title);
    useStore.getState().setPanel(null);
  } catch (e) {
    useStore.getState().showToast(String(e));
  }
}

/**
 * The frame of a side panel (computer, artifact, canvas). Docked, it is a column on the right whose
 * edge you drag to resize and that has a button to pop it out; floating, it fills its own window,
 * which you drag around the desktop and can keep on top.
 */
export function PanelShell({
  id,
  kind,
  title,
  subtitle,
  icon,
  badge,
  actions,
  widthKey,
  initial = 520,
  min = 340,
  children,
}: {
  id: string;
  kind: "artifact" | "canvas" | "computer";
  title: string;
  subtitle?: string;
  icon?: React.ReactNode;
  badge?: string;
  actions?: React.ReactNode;
  widthKey: string;
  initial?: number;
  min?: number;
  children: React.ReactNode;
}) {
  const { t } = useTranslation();
  const setPanel = useStore((s) => s.setPanel);
  const max = Math.max(min + 80, Math.min(1400, (typeof window === "undefined" ? 1400 : window.innerWidth) - 460));
  const { width, handle } = useResizable(widthKey, initial, min, max, "left");
  const [pinned, setPinned] = useState(false);

  const head = (
    <div className="flex h-13 shrink-0 items-center gap-2 px-4" data-tauri-drag-region>
      {icon}
      <div className="min-w-0 flex-1" data-tauri-drag-region>
        <h2 className="truncate text-[14px] leading-tight font-semibold" data-tauri-drag-region>
          {title}
        </h2>
        {subtitle && <p className="truncate text-[11.5px] leading-tight text-muted">{subtitle}</p>}
      </div>
      {badge && <span className="rounded-md bg-hover px-1.5 py-0.5 text-[11px] text-muted">{badge}</span>}
      <div className="flex items-center gap-0.5">
        {actions}
        {isFloating ? (
          <>
            <IconButton
              label={pinned ? t("panel.unpin") : t("panel.pin")}
              active={pinned}
              onClick={() => {
                const next = !pinned;
                setPinned(next);
                void api.floatPin(next);
              }}
            >
              {pinned ? <PinOff size={15} /> : <Pin size={15} />}
            </IconButton>
            <IconButton label={t("common.close")} onClick={() => void api.closeFloat()}>
              <X size={17} />
            </IconButton>
          </>
        ) : (
          <>
            {isTauri && (
              <IconButton label={t("panel.popOut")} onClick={() => void popOut(kind, id, title)}>
                <AppWindow size={16} />
              </IconButton>
            )}
            <IconButton label={t("common.close")} onClick={() => setPanel(null)}>
              <X size={17} />
            </IconButton>
          </>
        )}
      </div>
    </div>
  );

  if (isFloating) return <div className="flex h-full w-full flex-col bg-panel">{head}{children}</div>;
  return (
    <aside style={{ width }} className="animate-slide relative flex h-full shrink-0 flex-col border-l border-line bg-panel">
      {handle}
      {head}
      {children}
    </aside>
  );
}
