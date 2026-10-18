import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, ChevronRight, ExternalLink, File, Folder, FolderOpen, Maximize2, Minimize2, MonitorPlay, RefreshCw, RotateCcw, Settings2, TerminalSquare, X } from "lucide-react";
import { useStore } from "../../lib/store";
import { api, openUrl } from "../../lib/api";
import { confirmDialog } from "../../lib/confirm";
import type { Agent, ComputerBackend, FileEntry } from "../../lib/types";
import { cx } from "../../lib/format";
import { IconButton, Spinner } from "../ui";

export const BACKENDS: ComputerBackend[] = ["none", "docker", "space", "vmpal", "host", "wsl"];

function formatSize(n: number) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/** The agent's computer: a live terminal and a file browser over its workspace. */
export function ComputerPanel({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const setPanel = useStore((s) => s.setPanel);
  const [tab, setTab] = useState<"terminal" | "files" | "desktop">("terminal");
  const [wide, setWide] = useState(false);
  const backend = agent.settings.computer;
  const tabs = backend === "docker" ? (["terminal", "files", "desktop"] as const) : (["terminal", "files"] as const);
  useEffect(() => {
    if (backend !== "docker" && tab === "desktop") setTab("terminal");
  }, [backend, tab]);

  return (
    <aside className={cx("animate-slide flex h-full shrink-0 flex-col border-l border-line bg-panel transition-[width]", wide ? "w-[min(960px,70vw)]" : "w-[460px]")}>
      <div className="flex h-13 shrink-0 items-center gap-2 px-4">
        <h2 className="text-[14px] font-semibold">{t("computer.title")}</h2>
        <span className="rounded-md bg-hover px-1.5 py-0.5 text-[11px] text-muted">{t(`computer.backend.${backend}`)}</span>
        <div className="ml-auto flex items-center gap-0.5">
          {tab === "desktop" && (
            <IconButton label={wide ? t("computer.narrow") : t("computer.widen")} onClick={() => setWide((v) => !v)}>
              {wide ? <Minimize2 size={15} /> : <Maximize2 size={15} />}
            </IconButton>
          )}
          <IconButton label={t("computer.configure")} onClick={() => setPanel("profile")}>
            <Settings2 size={16} />
          </IconButton>
          <IconButton label={t("common.close")} onClick={() => setPanel(null)}>
            <X size={17} />
          </IconButton>
        </div>
      </div>
      {backend === "none" || backend === "vmpal" ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 px-8 text-center">
          <TerminalSquare size={30} className="text-faint" />
          <p className="text-[13.5px] font-medium">{backend === "vmpal" ? t("computer.vmpalTitle", { name: agent.name }) : t("computer.noneTitle", { name: agent.name })}</p>
          <p className="text-[12.5px] text-muted">{backend === "vmpal" ? t("computer.vmpalHint") : t("computer.noneHint")}</p>
          <button onClick={() => setPanel("profile")} className="mt-1 rounded-lg bg-accent px-3 py-1.5 text-[13px] font-medium text-white hover:bg-accent-hover">
            {t("computer.choose")}
          </button>
        </div>
      ) : (
        <>
          <div className="flex gap-1 px-4 pb-2">
            {tabs.map((k) => (
              <button
                key={k}
                onClick={() => setTab(k)}
                className={cx("rounded-lg px-2.5 py-1 text-[12.5px] font-medium", tab === k ? "bg-active text-ink" : "text-muted hover:bg-hover")}
              >
                {t(`computer.${k}`)}
              </button>
            ))}
          </div>
          {tab === "terminal" ? <Terminal agent={agent} /> : tab === "files" ? <Files agent={agent} /> : <Desktop agent={agent} />}
        </>
      )}
    </aside>
  );
}

/** The Docker computer's web desktop (noVNC / KasmVNC / Selkies), served on 127.0.0.1. */
function Desktop({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const openSettings = useStore((s) => s.openSettings);
  const [state, setState] = useState<{ url?: string; error?: string; loading: boolean }>({ loading: true });
  const [frameKey, setFrameKey] = useState(0);

  const load = useCallback(async () => {
    setState({ loading: true });
    try {
      setState({ url: await api.computerDesktop(agent.id), loading: false });
    } catch (e) {
      setState({ error: String(e), loading: false });
    }
  }, [agent.id]);

  useEffect(() => {
    void load();
  }, [load]);

  const restart = async () => {
    setState({ loading: true });
    try {
      await api.resetComputer(agent.id);
    } catch (e) {
      setState({ error: String(e), loading: false });
      return;
    }
    await load();
  };

  if (state.loading) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 text-[12.5px] text-muted">
        <Spinner /> {t("computer.desktopStarting")}
      </div>
    );
  }
  if (state.error || !state.url) {
    const needsSetting = /settings/i.test(state.error ?? "");
    const needsRestart = /without a desktop/i.test(state.error ?? "");
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-2 px-8 text-center">
        <MonitorPlay size={30} className="text-faint" />
        <p className="text-[13.5px] font-medium">{t("computer.desktopTitle")}</p>
        <p className="text-[12.5px] text-muted">{needsSetting ? t("computer.desktopOff") : needsRestart ? t("computer.desktopRestartHint") : state.error}</p>
        {needsSetting ? (
          <button onClick={() => openSettings("computers")} className="mt-1 rounded-lg bg-accent px-3 py-1.5 text-[13px] font-medium text-white hover:bg-accent-hover">
            {t("computer.openSettings")}
          </button>
        ) : (
          <button onClick={() => void (needsRestart ? restart() : load())} className="mt-1 rounded-lg bg-accent px-3 py-1.5 text-[13px] font-medium text-white hover:bg-accent-hover">
            {needsRestart ? t("computer.restart") : t("computer.retry")}
          </button>
        )}
      </div>
    );
  }
  return (
    <div className="flex min-h-0 flex-1 flex-col px-4 pb-4">
      <div className="mb-2 flex items-center gap-1">
        <span className="truncate font-mono text-[11.5px] text-muted">{state.url}</span>
        <div className="ml-auto flex items-center">
          <IconButton label={t("computer.reload")} onClick={() => setFrameKey((k) => k + 1)} className="size-7">
            <RefreshCw size={13} />
          </IconButton>
          <IconButton label={t("computer.openBrowser")} onClick={() => void openUrl(state.url!)} className="size-7">
            <ExternalLink size={13} />
          </IconButton>
          <IconButton label={t("computer.restart")} onClick={async () => (await confirmDialog(t("computer.restartConfirm"), { danger: false, confirmLabel: t("computer.restart") })) && void restart()} className="size-7">
            <RotateCcw size={13} />
          </IconButton>
        </div>
      </div>
      <iframe key={frameKey} src={state.url} title={t("computer.desktop")} className="min-h-0 flex-1 rounded-xl border border-line bg-black" allow="clipboard-read; clipboard-write; fullscreen" />
    </div>
  );
}

function Terminal({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const [log, setLog] = useState("");
  const [cmd, setCmd] = useState("");
  const [running, setRunning] = useState(false);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void api.computerLog(agent.id).then(setLog);
    const onOut = (e: Event) => {
      const d = (e as CustomEvent<{ agent_id: string; text: string }>).detail;
      if (d.agent_id === agent.id) setLog((l) => (l + d.text).slice(-100_000));
    };
    window.addEventListener("opencue:computer-output", onOut);
    return () => window.removeEventListener("opencue:computer-output", onOut);
  }, [agent.id]);

  useEffect(() => {
    // Newer engines return a promise from scrollIntoView; effects must not return it.
    endRef.current?.scrollIntoView({ block: "end" });
  }, [log]);

  const run = async () => {
    const c = cmd.trim();
    if (!c || running) return;
    setCmd("");
    setRunning(true);
    try {
      await api.computerExec(agent.id, c);
    } catch (e) {
      useStore.getState().showToast(String(e));
    } finally {
      setRunning(false);
    }
  };

  return (
    <div className="mx-4 mb-4 flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl bg-[#16171a] text-[#e6e6e6]">
      <pre className="selectable min-h-0 flex-1 overflow-auto px-3.5 py-3 font-mono text-[12px] leading-[1.55] whitespace-pre-wrap">
        {log || <span className="text-[#8a8a8f]">{t("computer.emptyLog")}</span>}
        <div ref={endRef} />
      </pre>
      <div className="flex items-center gap-2 border-t border-white/10 px-3.5 py-2">
        <span className="font-mono text-[12px] text-[#5ec2f2]">$</span>
        <input
          value={cmd}
          onChange={(e) => setCmd(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && void run()}
          placeholder={t("computer.prompt")}
          spellCheck={false}
          className="min-w-0 flex-1 bg-transparent font-mono text-[12px] text-[#e6e6e6] outline-none placeholder:text-[#6b6b70]"
        />
        {running && <Spinner size={12} />}
      </div>
    </div>
  );
}

function Files({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const [path, setPath] = useState("");
  const [entries, setEntries] = useState<FileEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [preview, setPreview] = useState<{ path: string; text: string } | null>(null);

  const load = useCallback(async () => {
    setEntries(null);
    setError(null);
    try {
      setEntries(await api.computerList(agent.id, path));
    } catch (e) {
      setError(String(e));
      setEntries([]);
    }
  }, [agent.id, path]);

  useEffect(() => void load(), [load]);

  if (preview) {
    return (
      <div className="flex min-h-0 flex-1 flex-col px-4 pb-4">
        <div className="mb-2 flex items-center gap-2">
          <IconButton label={t("common.back")} onClick={() => setPreview(null)} className="size-7">
            <ArrowLeft size={15} />
          </IconButton>
          <span className="truncate font-mono text-[12px] text-muted">{preview.path}</span>
        </div>
        <pre className="selectable min-h-0 flex-1 overflow-auto rounded-xl border border-line bg-bg p-3 font-mono text-[12px] whitespace-pre-wrap">{preview.text}</pre>
      </div>
    );
  }

  const crumbs = path ? path.split("/") : [];
  return (
    <div className="flex min-h-0 flex-1 flex-col px-4 pb-4">
      <div className="mb-2 flex items-center gap-1 text-[12.5px]">
        <button onClick={() => setPath("")} className="flex items-center gap-1 rounded-md px-1.5 py-0.5 text-muted hover:bg-hover hover:text-ink">
          <FolderOpen size={13} /> {t("computer.workspace")}
        </button>
        {crumbs.map((c, i) => (
          <span key={i} className="flex items-center gap-1">
            <ChevronRight size={12} className="text-faint" />
            <button onClick={() => setPath(crumbs.slice(0, i + 1).join("/"))} className="rounded-md px-1.5 py-0.5 text-muted hover:bg-hover hover:text-ink">
              {c}
            </button>
          </span>
        ))}
        <IconButton label={t("common.refresh")} onClick={() => void load()} className="ml-auto size-7">
          <RefreshCw size={13} />
        </IconButton>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-line">
        {entries === null ? (
          <div className="flex justify-center py-6 text-muted">
            <Spinner />
          </div>
        ) : error ? (
          <p className="p-4 text-[12.5px] text-danger">{error}</p>
        ) : entries.length === 0 ? (
          <p className="p-4 text-center text-[12.5px] text-faint">{t("computer.emptyFolder")}</p>
        ) : (
          entries.map((f) => (
            <button
              key={f.path}
              onClick={() => (f.is_dir ? setPath(f.path) : void api.computerRead(agent.id, f.path).then((text) => setPreview({ path: f.path, text })).catch((e) => setError(String(e))))}
              className="flex w-full items-center gap-2.5 border-b border-line px-3 py-2 text-left text-[13px] last:border-b-0 hover:bg-hover"
            >
              {f.is_dir ? <Folder size={15} className="text-accent" /> : <File size={15} className="text-faint" />}
              <span className="min-w-0 flex-1 truncate">{f.name}</span>
              {!f.is_dir && <span className="text-[11px] text-faint">{formatSize(f.size)}</span>}
            </button>
          ))
        )}
      </div>
    </div>
  );
}
