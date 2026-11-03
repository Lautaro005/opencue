import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Plug, Sparkles } from "lucide-react";
import { api } from "../../lib/api";
import { confirmDialog } from "../../lib/confirm";
import { useStore } from "../../lib/store";
import { cx } from "../../lib/format";
import type { DashboardData } from "../../lib/types";
import { Button, DialogHeader, Modal, Select, Textarea } from "../ui";

/** What the dashboard can show: the first ones OpenCue has itself, the rest the agent brings from apps. */
const BLOCKS = ["priorities", "agents", "automations", "usage", "activity", "recent", "inbox", "calendar", "tasks", "pipeline", "github", "decisions", "meetings", "notes"] as const;
const LAYOUTS = ["cards", "focus", "compact"] as const;
const STYLES = ["brand", "calm", "bold", "glass", "editorial", "terminal"] as const;
const REFRESH = ["live", "hourly", "morning"] as const;

/** Tap-to-pick chips. */
function Chips<T extends string>({ items, picked, onToggle, label }: { items: readonly T[]; picked: Set<T>; onToggle: (v: T) => void; label: (v: T) => string }) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {items.map((v) => {
        const on = picked.has(v);
        return (
          <button
            key={v}
            type="button"
            aria-pressed={on}
            onClick={() => onToggle(v)}
            className={cx(
              "flex items-center gap-1 rounded-full border px-2.5 py-1 text-[12.5px] transition-colors",
              on ? "border-accent bg-accent/10 text-ink" : "border-line text-muted hover:bg-hover hover:text-ink",
            )}
          >
            {on && <Check size={12} className="text-accent" />}
            {label(v)}
          </button>
        );
      })}
    </div>
  );
}

function toggle<T>(set: Set<T>, v: T): Set<T> {
  const next = new Set(set);
  if (next.has(v)) next.delete(v);
  else next.add(v);
  return next;
}

/**
 * Build the request for the dashboard agent from a few choices: what to show (from OpenCue and
 * from the connected apps), how it should look and how fresh it stays. It is sent as /dashboard in
 * the agent's chat, where the user can keep refining it.
 */
export function CustomizeDashboard({ open, onClose, data }: { open: boolean; onClose: () => void; data: DashboardData | null }) {
  const { t } = useTranslation();
  const agentMap = useStore((s) => s.agents);
  const agents = useMemo(() => Object.values(agentMap), [agentMap]);
  const settings = useStore((s) => s.settings);
  const setSetting = useStore((s) => s.setSetting);
  const select = useStore((s) => s.select);
  const send = useStore((s) => s.send);
  const [agentId, setAgentId] = useState("");
  const [blocks, setBlocks] = useState<Set<(typeof BLOCKS)[number]>>(new Set(["priorities", "agents", "automations"]));
  const [apps, setApps] = useState<Set<string>>(new Set());
  const [layout, setLayout] = useState<(typeof LAYOUTS)[number]>("cards");
  const [style, setStyle] = useState<(typeof STYLES)[number]>("brand");
  const [refresh, setRefresh] = useState<(typeof REFRESH)[number]>("live");
  const [extra, setExtra] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) setAgentId(agents.some((a) => a.id === settings.dashboard_agent) ? settings.dashboard_agent! : (agents[0]?.id ?? ""));
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  const connected = data?.apps ?? [];
  const appName = (id: string) => connected.find((a) => a.id === id)?.name ?? id;
  const nothing = blocks.size === 0 && apps.size === 0 && !extra.trim();

  const go = async () => {
    if (!agentId || nothing) return;
    setBusy(true);
    try {
      if (settings.dashboard_agent !== agentId) await setSetting("dashboard_agent", agentId);
      const lines = [
        blocks.size > 0 && `Show: ${[...blocks].map((b) => t(`dashboard.customize.blocks.${b}`)).join("; ")}.`,
        apps.size > 0 && `Bring data from these connected apps (extract it with their tools and save it with update_dashboard_data): ${[...apps].map(appName).join(", ")}.`,
        `Layout: ${t(`dashboard.customize.layouts.${layout}`)}. Style: ${t(`dashboard.customize.styles.${style}`)}.`,
        refresh !== "live" && `Keep the app data fresh with an automation that refreshes it ${refresh === "hourly" ? "every hour" : "every morning at 7:00"}.`,
        extra.trim() && `Also: ${extra.trim()}`,
      ].filter(Boolean);
      onClose();
      select(agentId);
      await send(agentId, `/dashboard ${lines.join("\n")}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open} onClose={onClose} className="w-[620px]" label={t("dashboard.customize.title")}>
      <DialogHeader title={t("dashboard.customize.title")} subtitle={t("dashboard.customize.subtitle")} onClose={onClose} />
      <div className="max-h-[68vh] space-y-5 overflow-y-auto px-6 pb-2">
        <section>
          <h3 className="mb-1.5 text-[12.5px] font-semibold">{t("dashboard.customize.agent")}</h3>
          <Select className="w-60" value={agentId} onChange={setAgentId} options={agents.map((a) => ({ value: a.id, label: a.name }))} />
        </section>
        <section>
          <h3 className="mb-1.5 text-[12.5px] font-semibold">{t("dashboard.customize.show")}</h3>
          <Chips items={BLOCKS} picked={blocks} onToggle={(v) => setBlocks((s) => toggle(s, v))} label={(v) => t(`dashboard.customize.blocks.${v}`)} />
        </section>
        <section>
          <h3 className="mb-0.5 flex items-center gap-1.5 text-[12.5px] font-semibold">
            <Plug size={13} className="text-accent" /> {t("dashboard.customize.apps")}
          </h3>
          <p className="mb-1.5 text-[11.5px] text-muted">{t("dashboard.customize.appsHint")}</p>
          {connected.length === 0 ? (
            <p className="text-[12px] text-faint">{t("dashboard.customize.noApps")}</p>
          ) : (
            <Chips items={connected.map((a) => a.id)} picked={apps} onToggle={(v) => setApps((s) => toggle(s, v))} label={appName} />
          )}
        </section>
        <div className="grid grid-cols-3 gap-3">
          <section>
            <h3 className="mb-1.5 text-[12.5px] font-semibold">{t("dashboard.customize.layout")}</h3>
            <Select value={layout} onChange={(v) => setLayout(v as typeof layout)} options={LAYOUTS.map((v) => ({ value: v, label: t(`dashboard.customize.layouts.${v}`) }))} />
          </section>
          <section>
            <h3 className="mb-1.5 text-[12.5px] font-semibold">{t("dashboard.customize.style")}</h3>
            <Select value={style} onChange={(v) => setStyle(v as typeof style)} options={STYLES.map((v) => ({ value: v, label: t(`dashboard.customize.styles.${v}`) }))} />
          </section>
          <section>
            <h3 className="mb-1.5 text-[12.5px] font-semibold">{t("dashboard.customize.refresh")}</h3>
            <Select value={refresh} onChange={(v) => setRefresh(v as typeof refresh)} options={REFRESH.map((v) => ({ value: v, label: t(`dashboard.customize.refreshes.${v}`) }))} />
          </section>
        </div>
        <section>
          <h3 className="mb-1.5 text-[12.5px] font-semibold">{t("dashboard.customize.extra")}</h3>
          <Textarea value={extra} onChange={(e) => setExtra(e.target.value)} rows={3} placeholder={t("dashboard.customize.extraPlaceholder")} />
        </section>
      </div>
      <div className="flex items-center justify-between gap-2 border-t border-line px-6 py-3.5">
        <Button
          variant="ghost"
          onClick={async () => {
            if (!(await confirmDialog(t("settings.general.dashboardResetConfirm"), { danger: false, confirmLabel: t("settings.general.dashboardReset") }))) return;
            await api.resetDashboard();
            onClose();
          }}
        >
          {t("settings.general.dashboardReset")}
        </Button>
        <div className="flex gap-2">
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button variant="primary" disabled={!agentId || nothing || busy} onClick={() => void go()}>
            <Sparkles size={13} /> {t("dashboard.customize.go")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}
