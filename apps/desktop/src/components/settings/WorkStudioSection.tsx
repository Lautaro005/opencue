import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { BrainCircuit, Copy, ExternalLink, LayoutGrid, Mic, Play } from "lucide-react";
import { api, openUrl } from "../../lib/api";
import { useStore } from "../../lib/store";
import type { FamilyApp } from "../../lib/types";
import { Badge, Button, Toggle } from "../ui";
import { cx } from "../../lib/format";

const TILE: Record<string, string> = {
  brain: "bg-gradient-to-br from-violet-500 to-fuchsia-500",
  lumina: "bg-gradient-to-br from-amber-400 to-orange-500",
  workstudio: "bg-gradient-to-br from-sky-500 to-indigo-500",
};

const ICON: Record<string, React.ReactNode> = {
  brain: <BrainCircuit size={18} />,
  lumina: <Mic size={18} />,
  workstudio: <LayoutGrid size={18} />,
};

/** Settings → Apps → WorkStudio: brain, Lumina and AI WorkStudio, found on this computer and wired
 *  to the agents with one switch. */
export function WorkStudioApps() {
  const { t } = useTranslation();
  const showToast = useStore((s) => s.showToast);
  const [apps, setApps] = useState<FamilyApp[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(() => void api.familyStatus().then(setApps).catch(() => setApps([])), []);
  useEffect(() => {
    load();
    // A started app takes a moment to answer: keep looking while this page is open.
    const timer = window.setInterval(load, 4000);
    return () => window.clearInterval(timer);
  }, [load]);

  if (!apps) return null;
  const ws = apps.find((a) => a.id === "workstudio");
  const act = async (key: string, run: () => Promise<unknown>) => {
    setBusy(key);
    try {
      await run();
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(null);
      load();
    }
  };

  return (
    <>
      <div className="space-y-3">
        {apps.map((a) => {
          const via = a.mcp === "via_workstudio";
          return (
            <section key={a.id} className="surface rounded-2xl p-4">
              <div className="flex items-center gap-3">
                <span className={cx("flex size-10 shrink-0 items-center justify-center rounded-xl text-white shadow-sm", TILE[a.id])}>{ICON[a.id]}</span>
                <div className="min-w-0 flex-1">
                  <h3 className="flex items-center gap-2 text-[14.5px] font-semibold">
                    {a.name}
                    {a.version && <span className="font-mono text-[11.5px] font-normal text-faint">v{a.version}</span>}
                  </h3>
                  <p className="text-[12.5px] text-muted">{t(`settings.workstudio.${a.id}.what`)}</p>
                </div>
                {!a.installed ? (
                  <Badge>{t("settings.workstudio.notInstalled")}</Badge>
                ) : a.running === true ? (
                  <Badge tone="success">{t("settings.workstudio.running")}</Badge>
                ) : a.running === false ? (
                  <Badge tone="warning">{t("settings.workstudio.stopped")}</Badge>
                ) : (
                  <Badge tone="success">{t("settings.workstudio.installed")}</Badge>
                )}
              </div>

              {a.installed ? (
                <>
                  <div className="mt-3 flex items-center gap-3 rounded-xl bg-hover/60 px-3 py-2.5">
                    <div className="min-w-0 flex-1">
                      <p className="text-[13px] font-medium">{t("settings.workstudio.tools")}</p>
                      <p className="text-[11.5px] text-muted">
                        {via ? t("settings.workstudio.viaWorkstudio") : a.id === "workstudio" ? t("settings.workstudio.toolsHub") : t(`settings.workstudio.${a.id}.tools`)}
                      </p>
                    </div>
                    <Toggle
                      checked={a.mcp === "on" || via}
                      disabled={busy === `mcp-${a.id}` || via}
                      onChange={(v) => void act(`mcp-${a.id}`, async () => { setApps(await api.familySetMcp(a.id, v)); })}
                      label={t("settings.workstudio.tools")}
                    />
                  </div>
                  <div className="mt-3 flex flex-wrap items-center gap-2">
                    {a.url && a.running ? (
                      <Button size="sm" variant="primary" onClick={() => void openUrl(a.url!)}>
                        <ExternalLink size={13} /> {t("settings.workstudio.open")}
                      </Button>
                    ) : (
                      <Button size="sm" variant="primary" disabled={busy === `start-${a.id}`} onClick={() => void act(`start-${a.id}`, () => api.familyStart(a.id))}>
                        <Play size={13} /> {a.id === "workstudio" ? t("settings.workstudio.open") : t("settings.workstudio.start")}
                      </Button>
                    )}
                    <Button size="sm" variant="ghost" onClick={() => void openUrl(a.repo_url)}>
                      GitHub
                    </Button>
                    {a.id !== "workstudio" && ws?.installed && a.mcp !== "on" && !via && ws.mcp !== "on" && <span className="text-[11.5px] text-faint">{t("settings.workstudio.tipWorkstudio")}</span>}
                  </div>
                </>
              ) : (
                <div className="mt-3">
                  <p className="mb-1.5 text-[12px] text-muted">{t("settings.workstudio.installHint")}</p>
                  <div className="flex items-center gap-2 rounded-xl bg-hover/60 px-3 py-2">
                    <code className="selectable min-w-0 flex-1 truncate font-mono text-[11.5px]">{a.install_command}</code>
                    <Button
                      size="sm"
                      onClick={() => void navigator.clipboard.writeText(a.install_command).then(() => showToast(t("settings.workstudio.copied")))}
                    >
                      <Copy size={12} /> {t("common.copy")}
                    </Button>
                  </div>
                  <Button size="sm" variant="ghost" className="mt-2" onClick={() => void openUrl(a.repo_url)}>
                    <ExternalLink size={12} /> GitHub
                  </Button>
                </div>
              )}
            </section>
          );
        })}
      </div>
      <p className="mt-4 text-[12px] text-faint">{t("settings.workstudio.note")}</p>
    </>
  );
}
