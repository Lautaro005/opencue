import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, ArrowRight, Globe, Plus, RefreshCw, Trash2, X } from "lucide-react";
import { api } from "../../lib/api";
import { useStore } from "../../lib/store";
import type { Agent, BrowserTab } from "../../lib/types";
import { cx } from "../../lib/format";
import { IconButton } from "../ui";

/**
 * The agent's browser: a native webview the desktop app places exactly over the box below. It is
 * separate from the app's own page and from the user's browsers (its own cookies and storage).
 */
export function BrowserPane({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const showToast = useStore((s) => s.showToast);
  // The native view floats above the page, so it steps aside while a dialog or settings are open.
  const covered = useStore((s) => s.settingsSection !== null || s.dialog !== null || s.call !== null);
  const box = useRef<HTMLDivElement>(null);
  const refresh = useRef<() => void>(() => {});
  const [url, setUrl] = useState("");
  const [typing, setTyping] = useState(false);
  const [title, setTitle] = useState("");
  const [tabs, setTabs] = useState<BrowserTab[]>([]);
  const native = "__TAURI_INTERNALS__" in window;

  useEffect(() => {
    const el = box.current;
    if (!el || !native) return;
    if (covered) {
      void api.browserHide();
      return;
    }
    // The panel slides in and resizes, which moves the box without resizing it: follow it every
    // frame (sending only when it changed) so the native view never sits where the box isn't.
    let raf = 0;
    let last = "";
    const follow = () => {
      const r = el.getBoundingClientRect();
      const key = `${Math.round(r.left)}|${Math.round(r.top)}|${Math.round(r.width)}|${Math.round(r.height)}`;
      if (key !== last && r.width > 0 && r.height > 0) {
        last = key;
        void api.browserPlace(agent.id, r.left, r.top, r.width, r.height);
      }
      raf = requestAnimationFrame(follow);
    };
    follow();
    return () => cancelAnimationFrame(raf);
  }, [covered, native, agent.id]);
  useEffect(() => () => void api.browserHide(), []);

  useEffect(() => {
    if (!native) return;
    let live = true;
    const poll = () =>
      void Promise.all([api.browserState(agent.id), api.browserTabs(agent.id)])
        .then(([[u, ti], list]) => {
          if (!live) return;
          if (!typing) setUrl(u || list.find((x) => x.active)?.url.replace(/^about:blank$/, "") || "");
          setTitle(ti);
          setTabs(list);
        })
        .catch(() => {});
    refresh.current = poll;
    poll();
    const timer = window.setInterval(poll, 1500);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [native, typing, agent.id]);

  const tabAct = (p: Promise<unknown>) => void p.then(() => refresh.current()).catch((e) => showToast(String(e)));
  const newTab = () => {
    setUrl("");
    tabAct(api.browserTabNew(agent.id));
  };

  const act = (action: string, to?: string) => void api.browserControl(agent.id, action, to).catch((e) => showToast(String(e)));

  return (
    <div className="flex min-h-0 flex-1 flex-col px-4 pb-2">
      {native && (
        <div role="tablist" aria-label={t("computer.browser.tabs")} className="mb-1.5 flex items-center gap-1 overflow-x-auto">
          {tabs.map((tab) => {
            const name = tab.title || tab.url.replace(/^https?:\/\//, "").replace(/\/.*$/, "") || t("computer.browser.newTab");
            return (
              <div
                key={tab.id}
                role="tab"
                aria-selected={tab.active}
                title={tab.url || name}
                onClick={() => tabAct(api.browserTabSelect(agent.id, tab.id))}
                className={cx("group flex h-7 max-w-[150px] min-w-[84px] shrink-0 cursor-pointer items-center gap-1.5 rounded-lg px-2 text-[12px]", tab.active ? "bg-active text-ink" : "text-muted hover:bg-hover")}
              >
                <Globe size={12} className="shrink-0 text-faint" />
                <span className="min-w-0 flex-1 truncate">{name}</span>
                <button
                  aria-label={t("computer.browser.closeTab")}
                  onClick={(e) => {
                    e.stopPropagation();
                    tabAct(api.browserTabClose(agent.id, tab.id));
                  }}
                  className="flex size-4 shrink-0 items-center justify-center rounded text-faint hover:bg-hover hover:text-ink"
                >
                  <X size={11} />
                </button>
              </div>
            );
          })}
          <IconButton label={t("computer.browser.newTab")} onClick={newTab} className="size-7 shrink-0">
            <Plus size={14} />
          </IconButton>
        </div>
      )}
      <div className="mb-2 flex items-center gap-0.5">
        <IconButton label={t("computer.browser.back")} onClick={() => act("back")} className="size-7">
          <ArrowLeft size={14} />
        </IconButton>
        <IconButton label={t("computer.browser.forward")} onClick={() => act("forward")} className="size-7">
          <ArrowRight size={14} />
        </IconButton>
        <IconButton label={t("computer.reload")} onClick={() => act("reload")} className="size-7">
          <RefreshCw size={13} />
        </IconButton>
        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onFocus={() => setTyping(true)}
          onBlur={() => setTyping(false)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && url.trim()) {
              act("go", url.trim());
              (e.target as HTMLInputElement).blur();
            }
          }}
          placeholder={t("computer.browser.address")}
          spellCheck={false}
          className="mx-1 min-w-0 flex-1 rounded-lg border border-line bg-bg px-2.5 py-1 font-mono text-[11.5px] outline-none focus:border-accent"
        />
        <IconButton label={t("computer.browser.clear")} onClick={() => void api.browserClear(agent.id).then(() => showToast(t("computer.browser.cleared")))} className="size-7">
          <Trash2 size={13} />
        </IconButton>
      </div>
      <div ref={box} aria-label={title || agent.name} className="relative min-h-0 flex-1 overflow-hidden rounded-xl border border-line bg-bg">
        <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center text-[12.5px] text-muted">
          <Globe size={26} className="text-faint" />
          {native ? t("computer.browser.empty", { name: agent.name }) : t("computer.browser.desktopOnly")}
        </div>
      </div>
    </div>
  );
}
