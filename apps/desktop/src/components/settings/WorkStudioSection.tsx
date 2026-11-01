import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Copy, ExternalLink, FolderTree, Play, RefreshCw } from "lucide-react";
import { api, openUrl } from "../../lib/api";
import { useStore } from "../../lib/store";
import type { BrainInfo, FamilyApp } from "../../lib/types";
import { Badge, Button, Input, Toggle } from "../ui";
import brainLogo from "../../assets/workstudio/brain.png";
import luminaLogo from "../../assets/workstudio/lumina.svg";
import workstudioLogo from "../../assets/workstudio/workstudio.svg";

const LOGO: Record<string, string> = { brain: brainLogo, lumina: luminaLogo, workstudio: workstudioLogo };

/** An app of the family, by its own logo. */
export function FamilyLogo({ id, size = 40 }: { id: string; size?: number }) {
  return <img src={LOGO[id]} alt="" width={size} height={size} draggable={false} className="shrink-0 rounded-xl" style={{ width: size, height: size }} />;
}

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
                <FamilyLogo id={a.id} />
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
                  {a.id === "brain" && <BrainNotebook connected={a.mcp === "on" || via} />}
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

/** brain as the agents' notebook: a folder per agent, per group and one for everyone. */
function BrainNotebook({ connected }: { connected: boolean }) {
  const { t } = useTranslation();
  const setSetting = useStore((s) => s.setSetting);
  const showToast = useStore((s) => s.showToast);
  const [info, setInfo] = useState<BrainInfo | null>(null);
  const [root, setRoot] = useState("");
  const [busy, setBusy] = useState(false);
  const load = useCallback(
    () =>
      void api
        .brainInfo()
        .then((i) => {
          setInfo(i);
          setRoot(i.root);
        })
        .catch(() => {}),
    [],
  );
  useEffect(load, [load]);
  if (!info?.installed) return null;

  const sync = async () => {
    setBusy(true);
    try {
      const n = await api.brainSyncAll();
      showToast(t("settings.workstudio.notebook.synced", { count: n }));
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
    }
  };
  const set = async (key: string, value: string) => {
    await setSetting(key, value);
    load();
  };

  return (
    <div className="mt-3 rounded-xl border border-line p-3">
      <div className="flex items-start gap-3">
        <FolderTree size={16} className="mt-0.5 shrink-0 text-accent" />
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium">{t("settings.workstudio.notebook.title")}</p>
          <p className="text-[11.5px] text-muted">{t("settings.workstudio.notebook.hint")}</p>
        </div>
        <Toggle
          checked={info.enabled}
          disabled={!connected && !info.enabled}
          onChange={async (v) => {
            await set("brain_sync", String(v));
            if (v) void sync();
          }}
          label={t("settings.workstudio.notebook.title")}
        />
      </div>
      {!connected && !info.enabled && <p className="mt-2 text-[11.5px] text-faint">{t("settings.workstudio.notebook.needsTools")}</p>}
      {info.enabled && (
        <div className="mt-3 space-y-2.5 border-t border-line pt-3">
          <label className="block">
            <span className="text-[12px] text-muted">{t("settings.workstudio.notebook.root")}</span>
            <div className="mt-1 flex gap-2">
              <Input value={root} onChange={(e) => setRoot(e.target.value)} placeholder="projects/opencue-agents" className="h-8 font-mono text-[12px]" />
              <Button size="sm" disabled={!root.trim() || root.trim() === info.root} onClick={() => void set("brain_root", root.trim())}>
                {t("common.save")}
              </Button>
            </div>
          </label>
          <div className="flex items-center gap-3">
            <div className="min-w-0 flex-1">
              <p className="text-[12.5px]">{t("settings.workstudio.notebook.shared")}</p>
              <p className="truncate font-mono text-[11px] text-faint">{info.shared_folder || `${info.root}/shared`}</p>
            </div>
            <Toggle checked={info.shared} onChange={(v) => void set("brain_shared", String(v))} label={t("settings.workstudio.notebook.shared")} />
          </div>
          <p className="text-[11.5px] text-faint">{t("settings.workstudio.notebook.perAgent")}</p>
          <div className="flex items-center gap-2">
            <Button size="sm" disabled={busy} onClick={() => void sync()}>
              <RefreshCw size={12} className={busy ? "animate-spin" : undefined} /> {t("settings.workstudio.notebook.syncNow")}
            </Button>
            {info.vault && <span className="truncate font-mono text-[11px] text-faint">{info.vault}</span>}
          </div>
        </div>
      )}
    </div>
  );
}
