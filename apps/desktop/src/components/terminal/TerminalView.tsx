import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { RotateCcw, Square, Trash2 } from "lucide-react";
import { api, openUrl } from "../../lib/api";
import { confirmDialog } from "../../lib/confirm";
import { useStore } from "../../lib/store";
import type { TermInfo } from "../../lib/types";
import { IconButton } from "../ui";
import { TermIcon } from "./TermIcon";

const bytes = (b64: string) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));

/** The terminal's own colors: always dark, a little different from the chat so it reads as a terminal. */
const THEME = {
  background: "#0e1014",
  foreground: "#dcdfe4",
  cursor: "#f2a65a",
  selectionBackground: "#2c3a52",
  black: "#1c1f26",
  red: "#ff6b6b",
  green: "#6bd48a",
  yellow: "#f2c15a",
  blue: "#6aa8ff",
  magenta: "#d18cf0",
  cyan: "#5ec9d6",
  white: "#dcdfe4",
  brightBlack: "#5b6270",
  brightRed: "#ff8787",
  brightGreen: "#8be5a3",
  brightYellow: "#ffd479",
  brightBlue: "#8bbcff",
  brightMagenta: "#e2a8f7",
  brightCyan: "#7fdbe6",
  brightWhite: "#ffffff",
};

/** A CLI agent (Claude Code, Codex, Grok…) running in a real terminal inside the app. */
export function TerminalView({ info }: { info: TermInfo }) {
  const { t } = useTranslation();
  const host = useRef<HTMLDivElement>(null);
  const term = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);
  const [exit, setExit] = useState<number | null>(info.alive ? null : -1);
  const refresh = useStore((s) => s.refreshTerminals);
  const selectTerm = useStore((s) => s.selectTerm);

  useEffect(() => {
    setExit(info.alive ? null : -1);
    const el = host.current;
    if (!el) return;
    const x = new Terminal({
      fontFamily: 'ui-monospace, "SF Mono", Menlo, "Cascadia Mono", Consolas, monospace',
      fontSize: 13,
      lineHeight: 1.25,
      cursorBlink: true,
      scrollback: 8000,
      allowProposedApi: true,
      theme: THEME,
    });
    const f = new FitAddon();
    x.loadAddon(f);
    x.loadAddon(new WebLinksAddon((_e, uri) => void openUrl(uri)));
    x.open(el);
    term.current = x;
    fit.current = f;
    const resize = () => {
      try {
        f.fit();
        void api.termResize(info.id, x.cols, x.rows).catch(() => {});
      } catch {
        /* the box has no size yet */
      }
    };
    resize();

    // Output that arrives while the saved screen is being drawn waits its turn.
    let ready = false;
    const waiting: Uint8Array[] = [];
    const onOut = (e: Event) => {
      const d = (e as CustomEvent<{ id: string; data: string }>).detail;
      if (d.id !== info.id) return;
      if (ready) x.write(bytes(d.data));
      else waiting.push(bytes(d.data));
    };
    const onExit = (e: Event) => {
      const d = (e as CustomEvent<{ id: string; code: number }>).detail;
      if (d.id === info.id) setExit(d.code);
    };
    window.addEventListener("opencue:term-output", onOut);
    window.addEventListener("opencue:term-exit", onExit);
    void api
      .termSnapshot(info.id)
      .then((b64) => b64 && x.write(bytes(b64)))
      .catch(() => {})
      .finally(() => {
        ready = true;
        waiting.forEach((c) => x.write(c));
        x.focus();
      });
    const data = x.onData((d) => void api.termWrite(info.id, d).catch(() => {}));
    const ro = new ResizeObserver(resize);
    ro.observe(el);
    return () => {
      ro.disconnect();
      data.dispose();
      window.removeEventListener("opencue:term-output", onOut);
      window.removeEventListener("opencue:term-exit", onExit);
      x.dispose();
      term.current = null;
    };
  }, [info.id]); // eslint-disable-line react-hooks/exhaustive-deps

  const restart = async () => {
    const x = term.current;
    try {
      x?.reset();
      await api.termRestart(info.id, x?.cols ?? 100, x?.rows ?? 30);
      setExit(null);
      await refresh();
      x?.focus();
    } catch (e) {
      useStore.getState().showToast(String(e));
    }
  };

  return (
    <main className="flex min-w-0 flex-1 flex-col bg-[#0e1014]" aria-label={info.label}>
      <header className="flex h-13 shrink-0 items-center gap-3 border-b border-white/10 bg-[#14171d] px-4 text-[#dcdfe4]" data-tauri-drag-region>
        <span className="flex size-7 items-center justify-center rounded-lg bg-white text-[#17171a]">
          <TermIcon tool={info.tool} size={15} />
        </span>
        <span className="min-w-0 flex-1" data-tauri-drag-region>
          <span className="block truncate text-[14px] leading-tight font-semibold">{info.label}</span>
          <span className="block truncate font-mono text-[11px] leading-tight text-[#8a909c]">{info.cwd}</span>
        </span>
        <span className={info.alive && exit === null ? "text-[11.5px] text-[#6bd48a]" : "text-[11.5px] text-[#8a909c]"}>{info.alive && exit === null ? t("terminal.running") : t("terminal.ended")}</span>
        <IconButton label={t("terminal.restart")} onClick={() => void restart()} className="text-[#dcdfe4] hover:bg-white/10">
          <RotateCcw size={15} />
        </IconButton>
        <IconButton label={t("terminal.stop")} onClick={() => void api.termKill(info.id).then(() => setExit(-1))} disabled={exit !== null} className="text-[#dcdfe4] hover:bg-white/10">
          <Square size={14} fill="currentColor" />
        </IconButton>
        <IconButton
          label={t("terminal.close")}
          onClick={async () => {
            if (await confirmDialog(t("terminal.closeConfirm", { name: info.label }), { confirmLabel: t("terminal.close") })) {
              await api.termClose(info.id);
              selectTerm(null);
              await refresh();
            }
          }}
          className="text-[#dcdfe4] hover:bg-white/10"
        >
          <Trash2 size={15} />
        </IconButton>
      </header>
      <div ref={host} className="min-h-0 flex-1 px-3 py-2" onClick={() => term.current?.focus()} />
      {exit !== null && (
        <div className="flex items-center gap-3 border-t border-white/10 bg-[#14171d] px-4 py-2 text-[12.5px] text-[#aab0bb]">
          {t("terminal.endedHint")}
          <button onClick={() => void restart()} className="rounded-lg bg-[#f2a65a] px-3 py-1 font-medium text-[#1b1206] hover:brightness-110">
            {t("terminal.start")}
          </button>
        </div>
      )}
    </main>
  );
}
