import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, CircleAlert, Download, Play, Square, X } from "lucide-react";
import { api } from "../../lib/api";
import { cx } from "../../lib/format";
import { useStore } from "../../lib/store";
import type { SpaceStatus } from "../../lib/types";
import { Badge, Button, Spinner } from "../ui";

/** Install Perplexity SPACE and connect it, from Settings → Computers (Linux with KVM). */
export function SpaceSetup({ onChanged }: { onChanged: () => void }) {
  const { t } = useTranslation();
  const [s, setS] = useState<SpaceStatus | null>(null);
  const showToast = useStore((st) => st.showToast);
  const log = useRef<HTMLPreElement>(null);
  const was = useRef("");

  const load = useCallback(() => void api.spaceStatus().then(setS).catch(() => {}), []);
  useEffect(() => {
    load();
    const timer = window.setInterval(load, 2500);
    return () => window.clearInterval(timer);
  }, [load]);
  useEffect(() => {
    if (log.current) log.current.scrollTop = log.current.scrollHeight;
  }, [s?.log]);
  // When an installation finishes, the address and token below are filled in: show them.
  useEffect(() => {
    if (s && was.current === "installing" && s.state === "ready") onChanged();
    if (s) was.current = s.state;
  }, [s, onChanged]);
  if (!s || !s.supported) return null;

  const run = (fn: () => Promise<void>) => void fn().catch((e) => showToast(String(e))).finally(load);
  const blockers = s.checks.filter((c) => !c.ok);
  const busy = s.state === "installing" || s.state === "checking";

  return (
    <div className="border-b border-line px-4 py-3.5">
      <div className="flex items-center gap-2">
        <span className="text-[13.5px] font-medium">{t("settings.computers.auto.title")}</span>
        {s.state === "ready" && (s.api_up ? <Badge tone="success">{t("settings.computers.auto.running")}</Badge> : <Badge tone="warning">{t("settings.computers.auto.stopped")}</Badge>)}
        {s.state === "error" && <Badge tone="danger">{t("settings.computers.auto.failed")}</Badge>}
      </div>
      <p className="mt-0.5 text-[12px] text-muted">{t("settings.computers.auto.hint")}</p>

      {!busy && s.checks.length > 0 && s.state !== "ready" && (
        <ul className="mt-2.5 space-y-1">
          {s.checks.map((c) => (
            <li key={c.id} className="flex items-start gap-2 text-[12px]">
              {c.ok ? <Check size={13} strokeWidth={3} className="mt-0.5 shrink-0 text-success" /> : <X size={13} strokeWidth={3} className="mt-0.5 shrink-0 text-danger" />}
              <span className={cx(c.ok ? "text-muted" : "text-ink")}>{c.detail}</span>
            </li>
          ))}
        </ul>
      )}

      {busy && (
        <div className="mt-2.5">
          <p className="flex items-center gap-2 text-[12.5px] font-medium">
            <Spinner size={13} /> {s.step}
          </p>
        </div>
      )}
      {(busy || s.state === "error") && s.log && (
        <pre ref={log} className="selectable mt-2 max-h-44 overflow-auto rounded-lg bg-hover/60 px-3 py-2 font-mono text-[11px] leading-[1.5] whitespace-pre-wrap text-muted">
          {s.log}
        </pre>
      )}

      <div className="mt-3 flex flex-wrap items-center gap-2">
        {busy ? (
          <Button onClick={() => run(() => api.spaceCancel())}>
            <Square size={11} fill="currentColor" /> {t("common.cancel")}
          </Button>
        ) : s.state === "ready" || s.managed ? (
          <>
            {!s.api_up && (
              <Button variant="primary" onClick={() => run(() => api.spaceStart())}>
                <Play size={13} /> {t("settings.computers.auto.start")}
              </Button>
            )}
            {s.api_up && (
              <Button onClick={() => run(() => api.spaceStop())}>
                <Square size={11} fill="currentColor" /> {t("settings.computers.auto.stop")}
              </Button>
            )}
            <Button variant="ghost" onClick={() => run(() => api.spaceInstall())} disabled={blockers.length > 0}>
              {t("settings.computers.auto.reinstall")}
            </Button>
          </>
        ) : (
          <Button variant="primary" onClick={() => run(() => api.spaceInstall())} disabled={blockers.length > 0}>
            <Download size={13} /> {t("settings.computers.auto.install")}
          </Button>
        )}
        {blockers.length > 0 && !busy && (
          <span className="flex items-center gap-1 text-[11.5px] text-warning">
            <CircleAlert size={12} /> {t("settings.computers.auto.fix", { count: blockers.length })}
          </span>
        )}
      </div>
      <p className="mt-2 text-[11.5px] text-faint">{t("settings.computers.auto.note")}</p>
    </div>
  );
}
