import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../../lib/api";
import { useStore } from "../../lib/store";
import type { DashboardData } from "../../lib/types";
import { Logo } from "../Sidebar";
import { frameBase } from "../panel/ArtifactPanel";

const REFRESH_MS = 5000;

/**
 * The home screen. The page is HTML (the built-in one, or one the user's agent wrote with
 * /dashboard) in a sandboxed frame; the app sends it live data as a message and opens an agent
 * when the page asks to.
 */
export function Dashboard() {
  const { t } = useTranslation();
  const select = useStore((s) => s.select);
  const setDialog = useStore((s) => s.setDialog);
  const hasAgents = useStore((s) => s.threads.length > 0);
  const frame = useRef<HTMLIFrameElement>(null);
  const data = useRef<DashboardData | null>(null);
  const [base, setBase] = useState<string | null>(null);
  const [srcDoc, setSrcDoc] = useState("");
  const [nonce, setNonce] = useState(0);

  useEffect(() => void frameBase().then(setBase), []);

  // Outside the desktop app the page comes through srcdoc.
  const loadPage = useCallback(() => {
    void api.dashboardInfo().then((i) => setSrcDoc(i.html)).catch(() => {});
  }, []);
  useEffect(loadPage, [loadPage]);

  const push = useCallback(() => {
    const d = data.current;
    if (!d) return;
    const dark = document.documentElement.classList.contains("dark");
    frame.current?.contentWindow?.postMessage({ type: "opencue:data", data: { ...d, theme: dark ? "dark" : "light" } }, "*");
  }, []);

  const refresh = useCallback(async () => {
    try {
      data.current = await api.dashboardData();
      push();
    } catch {
      /* the next tick tries again */
    }
  }, [push]);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), REFRESH_MS);
    // A new page (the agent rewrote it) or a change in what is running shows at once.
    const changed = () => {
      loadPage();
      setNonce((n) => n + 1);
    };
    window.addEventListener("opencue:dashboard-changed", changed);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("opencue:dashboard-changed", changed);
    };
  }, [refresh, loadPage]);

  // Messages from the page: it wants the data, or to open an agent.
  useEffect(() => {
    const onMessage = (e: MessageEvent) => {
      if (e.source !== frame.current?.contentWindow) return;
      const m = e.data as { type?: string; agent_id?: string };
      if (m?.type === "opencue:ready") void refresh();
      if (m?.type === "opencue:open" && typeof m.agent_id === "string" && useStore.getState().threads.some((th) => th.id === m.agent_id)) select(m.agent_id);
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [refresh, select]);

  // The theme can change while the page is open.
  useEffect(() => {
    const obs = new MutationObserver(push);
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => obs.disconnect();
  }, [push]);

  if (!hasAgents) {
    return (
      <main className="flex flex-1 flex-col items-center justify-center gap-3 bg-bg text-center" data-tauri-drag-region>
        <Logo size={48} />
        <h1 className="text-[19px] font-semibold tracking-tight">{t("chat.selectTitle")}</h1>
        <p className="max-w-xs text-[13.5px] text-muted">{t("chat.selectHint")}</p>
        <button onClick={() => setDialog("create-agent")} className="mt-1 rounded-lg bg-accent px-3.5 py-1.5 text-[13px] font-medium text-white hover:bg-accent-hover">
          {t("sidebar.createAgent")}
        </button>
      </main>
    );
  }

  return (
    <main className="flex min-w-0 flex-1 flex-col bg-bg">
      <header className="flex h-13 shrink-0 items-center gap-2 border-b border-line px-4" data-tauri-drag-region>
        <h1 className="text-[14px] font-semibold">{t("dashboard.title")}</h1>
      </header>
      {base !== null && (
        <iframe
          key={nonce}
          ref={frame}
          title={t("dashboard.title")}
          sandbox="allow-scripts"
          onLoad={push}
          {...(base ? { src: `${base}/dashboard?v=${nonce}` } : { srcDoc })}
          className="min-h-0 w-full flex-1 border-0 bg-bg"
        />
      )}
    </main>
  );
}
