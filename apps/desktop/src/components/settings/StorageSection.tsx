import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Archive, Bot, Box, Database, FileText, FolderOpen, HardDrive, MessageCircle, Package, RefreshCw, Sparkles } from "lucide-react";
import { api } from "../../lib/api";
import { useStore } from "../../lib/store";
import { formatBytes } from "../../lib/format";
import type { StorageItem, StorageReport } from "../../lib/types";
import { Button, IconButton, Spinner } from "../ui";
import { PageTitle } from "./SettingsModal";

/** Each kind's color (the same in both themes) and icon. */
const KIND: Record<StorageItem["id"], { color: string; icon: React.ReactNode }> = {
  models: { color: "#7c5cff", icon: <Box size={15} /> },
  agents: { color: "#0a84ff", icon: <Bot size={15} /> },
  files: { color: "#14b8a6", icon: <FileText size={15} /> },
  chats: { color: "#f5a524", icon: <Database size={15} /> },
  artifacts: { color: "#ec4899", icon: <Sparkles size={15} /> },
  memory: { color: "#8b93a7", icon: <Archive size={15} /> },
  messenger: { color: "#30a46c", icon: <MessageCircle size={15} /> },
  space: { color: "#f97316", icon: <Package size={15} /> },
  other: { color: "#a1a1aa", icon: <HardDrive size={15} /> },
};

/** Settings → Storage: how much room OpenCue takes, by kind, with what can be done about it. */
export function StorageSection() {
  const { t } = useTranslation();
  const showToast = useStore((s) => s.showToast);
  const openSettings = useStore((s) => s.openSettings);
  const [report, setReport] = useState<StorageReport | null>(null);
  const [busy, setBusy] = useState<"load" | "compact" | null>("load");
  // Bars grow from zero the first time the numbers arrive.
  const [shown, setShown] = useState(false);

  const load = useCallback(async () => {
    setBusy("load");
    try {
      setReport(await api.storageReport());
      requestAnimationFrame(() => setShown(true));
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(null);
    }
  }, [showToast]);
  useEffect(() => void load(), [load]);

  const compact = async () => {
    const before = report?.items.find((i) => i.id === "chats")?.bytes ?? 0;
    setBusy("compact");
    try {
      const next = await api.storageCompact();
      setReport(next);
      const saved = before - (next.items.find((i) => i.id === "chats")?.bytes ?? 0);
      showToast(saved > 0 ? t("settings.storage.compacted", { saved: formatBytes(saved) }) : t("settings.storage.compactedNone"));
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const total = report?.total ?? 0;
  const diskUsed = report?.disk_total != null && report.disk_free != null ? report.disk_total - report.disk_free : null;

  return (
    <>
      <PageTitle title={t("settings.storage.title")} hint={t("settings.storage.hint")} />

      <section className="surface relative mb-5 overflow-hidden rounded-2xl p-5">
        <div className="pointer-events-none absolute -top-24 -right-16 size-56 rounded-full bg-accent/10 blur-3xl" />
        <div className="relative flex items-start gap-4">
          <div className="min-w-0 flex-1">
            <p className="text-[11.5px] font-semibold tracking-[0.06em] text-muted uppercase">{t("settings.storage.used")}</p>
            <p className="mt-1 text-[32px] leading-none font-semibold tracking-[-0.02em] tabular-nums">{report ? formatBytes(total) : "—"}</p>
            {report?.disk_total != null && report.disk_free != null && (
              <p className="mt-2 text-[12.5px] text-muted">{t("settings.storage.ofDisk", { free: formatBytes(report.disk_free), total: formatBytes(report.disk_total) })}</p>
            )}
          </div>
          <IconButton label={t("settings.storage.refresh")} onClick={() => void load()} disabled={busy !== null}>
            <RefreshCw size={15} className={busy === "load" ? "animate-spin" : ""} />
          </IconButton>
        </div>

        {/* What OpenCue's share is made of. */}
        <div className="relative mt-5 flex h-3 w-full gap-[3px] overflow-hidden rounded-full bg-hover" role="img" aria-label={t("settings.storage.used")}>
          {report?.items.map((i) => (
            <span
              key={i.id}
              title={`${t(`settings.storage.kind.${i.id}.name`)} · ${formatBytes(i.bytes)}`}
              className="h-full rounded-full transition-[width] duration-700 ease-out"
              style={{ width: shown && total ? `max(4px, ${(i.bytes / total) * 100}%)` : 0, background: KIND[i.id].color }}
            />
          ))}
        </div>
        {/* And how much of the disk that is. */}
        {diskUsed != null && report?.disk_total ? (
          <div className="relative mt-3 flex h-1.5 w-full overflow-hidden rounded-full bg-hover">
            <span className="h-full bg-ink/25 transition-[width] duration-700 ease-out" style={{ width: shown ? `${((diskUsed - total) / report.disk_total) * 100}%` : 0 }} />
            <span className="h-full bg-accent transition-[width] duration-700 ease-out" style={{ width: shown ? `max(3px, ${(total / report.disk_total) * 100}%)` : 0 }} />
          </div>
        ) : null}
      </section>

      <section className="surface mb-5 overflow-hidden rounded-2xl">
        {!report ? (
          <div className="flex items-center justify-center gap-2 py-10 text-[13px] text-muted">
            <Spinner size={14} /> {t("common.loading")}
          </div>
        ) : report.items.length === 0 ? (
          <p className="py-10 text-center text-[13px] text-muted">{t("settings.storage.empty")}</p>
        ) : (
          <ul className="divide-y divide-line">
            {report.items.map((i) => {
              const pct = total ? (i.bytes / total) * 100 : 0;
              return (
                <li key={i.id} className="flex items-center gap-3.5 px-4 py-3">
                  <span className="flex size-9 shrink-0 items-center justify-center rounded-xl" style={{ background: `color-mix(in srgb, ${KIND[i.id].color} 14%, transparent)`, color: KIND[i.id].color }}>
                    {KIND[i.id].icon}
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-baseline gap-2">
                      <span className="text-[13.5px] font-medium">{t(`settings.storage.kind.${i.id}.name`)}</span>
                      <span className="text-[11.5px] text-faint tabular-nums">{t("settings.storage.files", { count: i.files })}</span>
                      {i.id === "models" && (
                        <button onClick={() => openSettings("models", "local")} className="text-[11.5px] font-medium text-accent hover:underline">
                          {t("settings.storage.manage")}
                        </button>
                      )}
                    </div>
                    <p className="truncate text-[12px] text-muted">{t(`settings.storage.kind.${i.id}.what`)}</p>
                    <div className="mt-1.5 h-1 w-full overflow-hidden rounded-full bg-hover">
                      <span className="block h-full rounded-full transition-[width] duration-700 ease-out" style={{ width: shown ? `${pct}%` : 0, background: KIND[i.id].color }} />
                    </div>
                  </div>
                  <div className="w-[88px] shrink-0 text-right">
                    <div className="text-[13.5px] font-medium tabular-nums">{formatBytes(i.bytes)}</div>
                    <div className="text-[11px] text-faint tabular-nums">{pct < 1 ? "<1" : Math.round(pct)}%</div>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </section>

      <section className="surface divide-y divide-line overflow-hidden rounded-2xl">
        <div className="flex items-center gap-4 px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="text-[13.5px]">{t("settings.storage.compact")}</div>
            <div className="mt-0.5 text-[12px] text-muted">{t("settings.storage.compactHint")}</div>
          </div>
          <Button size="sm" onClick={() => void compact()} disabled={busy !== null}>
            {busy === "compact" && <Spinner size={12} />} {t("settings.storage.compact")}
          </Button>
        </div>
        <div className="flex items-center gap-4 px-4 py-3">
          <div className="min-w-0 flex-1">
            <div className="text-[13.5px]">{t("settings.storage.folder")}</div>
            <div className="selectable mt-0.5 truncate font-mono text-[11.5px] text-muted" title={report?.root}>{report?.root ?? "—"}</div>
          </div>
          <Button size="sm" onClick={() => void api.openDataDir().catch((e) => showToast(String(e)))}>
            <FolderOpen size={13} /> {t("settings.storage.openFolder")}
          </Button>
        </div>
      </section>
    </>
  );
}
