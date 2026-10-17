import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, ChevronRight, File, Folder, FolderOpen, RefreshCw, Settings2, TerminalSquare, X } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import type { Agent, ComputerBackend, FileEntry } from "../../lib/types";
import { cx } from "../../lib/format";
import { IconButton, Spinner } from "../ui";

export const BACKENDS: ComputerBackend[] = ["none", "docker", "space", "host", "wsl"];

function formatSize(n: number) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/** The agent's computer: a live terminal and a file browser over its workspace. */
export function ComputerPanel({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const setPanel = useStore((s) => s.setPanel);
  const [tab, setTab] = useState<"terminal" | "files">("terminal");
  const backend = agent.settings.computer;

  return (
    <aside className="animate-slide flex h-full w-[460px] shrink-0 flex-col border-l border-line bg-panel">
      <div className="flex h-13 shrink-0 items-center gap-2 px-4">
        <h2 className="text-[14px] font-semibold">{t("computer.title")}</h2>
        <span className="rounded-md bg-hover px-1.5 py-0.5 text-[11px] text-muted">{t(`computer.backend.${backend}`)}</span>
        <div className="ml-auto flex items-center gap-0.5">
          <IconButton label={t("computer.configure")} onClick={() => setPanel("profile")}>
            <Settings2 size={16} />
          </IconButton>
          <IconButton label={t("common.close")} onClick={() => setPanel(null)}>
            <X size={17} />
          </IconButton>
        </div>
      </div>
      {backend === "none" ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 px-8 text-center">
          <TerminalSquare size={30} className="text-faint" />
          <p className="text-[13.5px] font-medium">{t("computer.noneTitle", { name: agent.name })}</p>
          <p className="text-[12.5px] text-muted">{t("computer.noneHint")}</p>
          <button onClick={() => setPanel("profile")} className="mt-1 rounded-lg bg-accent px-3 py-1.5 text-[13px] font-medium text-white hover:bg-accent-hover">
            {t("computer.choose")}
          </button>
        </div>
      ) : (
        <>
          <div className="flex gap-1 px-4 pb-2">
            {(["terminal", "files"] as const).map((k) => (
              <button
                key={k}
                onClick={() => setTab(k)}
                className={cx("rounded-lg px-2.5 py-1 text-[12.5px] font-medium", tab === k ? "bg-active text-ink" : "text-muted hover:bg-hover")}
              >
                {t(`computer.${k}`)}
              </button>
            ))}
          </div>
          {tab === "terminal" ? <Terminal agent={agent} /> : <Files agent={agent} />}
        </>
      )}
    </aside>
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
