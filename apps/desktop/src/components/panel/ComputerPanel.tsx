import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ExternalLink, MonitorPlay, RefreshCw, RotateCcw, Settings2, Square, TerminalSquare } from "lucide-react";
import { useStore } from "../../lib/store";
import { api, openUrl } from "../../lib/api";
import { confirmDialog } from "../../lib/confirm";
import type { Agent, ComputerBackend } from "../../lib/types";
import { BrowserPane } from "./BrowserPane";
import { cx } from "../../lib/format";
import { IconButton, Spinner } from "../ui";
import { PanelShell, isFloating } from "./PanelShell";

export const BACKENDS: ComputerBackend[] = ["none", "docker", "space", "vmpal", "host", "wsl"];

/** The agent's computer: a live terminal and a file browser over its workspace. */
export function ComputerPanel({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const setPanel = useStore((s) => s.setPanel);
  const showToast = useStore((s) => s.showToast);
  const backend = agent.settings.computer;
  const hasVm = backend === "docker";
  const hasBrowser = agent.settings.browser;
  // The top half is the VM and/or the browser (tabs only when there are both); the terminal is always below.
  const [top, setTop] = useState<"vm" | "browser">(hasVm ? "vm" : "browser");
  const tops = [...(hasVm ? (["vm"] as const) : []), ...(hasBrowser ? (["browser"] as const) : [])];
  const current = tops.includes(top) ? top : tops[0];
  // An agent opened its browser: show it.
  useEffect(() => {
    const focus = () => hasBrowser && setTop("browser");
    window.addEventListener("opencue:browser-focus", focus);
    return () => window.removeEventListener("opencue:browser-focus", focus);
  }, [hasBrowser]);
  // The local VM can be stopped from here; the button follows whether it is running.
  const [running, setRunning] = useState(false);
  useEffect(() => {
    if (backend !== "docker") return;
    let live = true;
    const poll = () => void api.computerRunning(agent.id).then((r) => live && setRunning(r)).catch(() => {});
    poll();
    const timer = window.setInterval(poll, 4000);
    return () => {
      live = false;
      window.clearInterval(timer);
    };
  }, [backend, agent.id]);
  const stop = async () => {
    try {
      await api.stopComputer(agent.id);
      setRunning(false);
    } catch (e) {
      showToast(String(e));
    }
  };
  const none = backend === "none" || backend === "vmpal";

  const actions = (
    <>
      {backend === "docker" && (
        <IconButton label={t("computer.stop")} onClick={() => void stop()} disabled={!running} className={running ? "text-danger" : undefined}>
          <Square size={14} fill="currentColor" />
        </IconButton>
      )}
      {!isFloating && (
        <IconButton label={t("computer.configure")} onClick={() => setPanel("profile")}>
          <Settings2 size={16} />
        </IconButton>
      )}
    </>
  );

  return (
    <PanelShell id={agent.id} kind="computer" title={t("computer.title")} subtitle={agent.name} badge={t(`computer.backend.${backend}`)} actions={actions} widthKey="opencue:panel-computer" initial={460}>
      {none && !hasBrowser ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 px-8 text-center">
          <TerminalSquare size={30} className="text-faint" />
          <p className="text-[13.5px] font-medium">{backend === "vmpal" ? t("computer.vmpalTitle", { name: agent.name }) : t("computer.noneTitle", { name: agent.name })}</p>
          <p className="text-[12.5px] text-muted">{backend === "vmpal" ? t("computer.vmpalHint") : t("computer.noneHint")}</p>
          <button onClick={() => setPanel("profile")} className="mt-1 rounded-lg btn-primary px-3 py-1.5 text-[13px] font-medium text-white">
            {t("computer.choose")}
          </button>
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          {tops.length > 0 && (
            <div className="flex min-h-0 flex-1 basis-1/2 flex-col">
              {tops.length > 1 && (
                <div className="flex gap-1 px-4 pb-2">
                  {tops.map((k) => (
                    <button key={k} onClick={() => setTop(k)} className={cx("rounded-lg px-2.5 py-1 text-[12.5px] font-medium", current === k ? "bg-active text-ink" : "text-muted hover:bg-hover")}>
                      {t(`computer.top.${k}`)}
                    </button>
                  ))}
                </div>
              )}
              {current === "vm" ? <Desktop agent={agent} /> : <BrowserPane agent={agent} />}
            </div>
          )}
          {!none && (
            <div className={cx("flex min-h-0 flex-col", tops.length > 0 ? "basis-1/2 flex-1 border-t border-line pt-2" : "flex-1")}>
              <Terminal agent={agent} />
            </div>
          )}
        </div>
      )}
    </PanelShell>
  );
}

function Desktop({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const setPanel = useStore((s) => s.setPanel);
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
          <button onClick={() => setPanel("profile")} className="mt-1 rounded-lg btn-primary px-3 py-1.5 text-[13px] font-medium text-white">
            {t("computer.configure")}
          </button>
        ) : (
          <button onClick={() => void (needsRestart ? restart() : load())} className="mt-1 rounded-lg btn-primary px-3 py-1.5 text-[13px] font-medium text-white">
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
