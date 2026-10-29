import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUpRight, LifeBuoy, RefreshCw, Search } from "lucide-react";
import { useStore } from "../../lib/store";
import { api, checkForUpdate, openUrl } from "../../lib/api";
import { confirmDialog } from "../../lib/confirm";
import type { AppInfo, CliLimits, MemoryEntry, UsageRow } from "../../lib/types";
import { compact, cx } from "../../lib/format";
import { LANGUAGES, setLanguage } from "../../i18n";
import { getTheme, setTheme, type Theme } from "../../lib/theme";
import { Button, IconButton, Input, Select, Textarea, Toggle } from "../ui";
import { Card, CardRow, PageTitle } from "./SettingsModal";

/** Who edits the dashboard (the page on the home screen) and how to ask for changes. */
function DashboardCard() {
  const { t } = useTranslation();
  const agentMap = useStore((s) => s.agents);
  const agents = useMemo(() => Object.values(agentMap), [agentMap]);
  const settings = useStore((s) => s.settings);
  const setSetting = useStore((s) => s.setSetting);
  const select = useStore((s) => s.select);
  const closeSettings = useStore((s) => s.closeSettings);
  const [custom, setCustom] = useState(false);
  const chosen = agents.find((a) => a.id === settings.dashboard_agent);
  const load = () => void api.dashboardInfo().then((i) => setCustom(!i.is_default)).catch(() => {});
  useEffect(() => {
    load();
    window.addEventListener("opencue:dashboard-changed", load);
    return () => window.removeEventListener("opencue:dashboard-changed", load);
  }, []);
  return (
    <Card title={t("settings.general.dashboard")}>
      <CardRow label={t("settings.general.dashboardAgent")} hint={t("settings.general.dashboardAgentHint")}>
        <Select
          className="w-44"
          value={chosen?.id ?? ""}
          onChange={(v) => void setSetting("dashboard_agent", v)}
          options={[{ value: "", label: t("settings.general.dashboardNone") }, ...agents.map((a) => ({ value: a.id, label: a.name }))]}
        />
      </CardRow>
      <CardRow label={custom ? t("settings.general.dashboardCustom") : t("settings.general.dashboardBuiltIn")} hint={chosen ? t("settings.general.dashboardHowTo", { name: chosen.name }) : t("settings.general.dashboardPick")}>
        <div className="flex items-center gap-2">
          {chosen && (
            <Button
              size="sm"
              onClick={() => {
                closeSettings();
                select(chosen.id);
                useStore.getState().setPrefill("/dashboard ");
              }}
            >
              /dashboard
            </Button>
          )}
          {custom && (
            <Button
              size="sm"
              onClick={async () => {
                if (await confirmDialog(t("settings.general.dashboardResetConfirm"), { danger: false, confirmLabel: t("settings.general.dashboardReset") })) {
                  await api.resetDashboard();
                  load();
                }
              }}
            >
              {t("settings.general.dashboardReset")}
            </Button>
          )}
          <Button
            size="sm"
            onClick={() => {
              closeSettings();
              select(null);
            }}
          >
            {t("settings.general.dashboardOpen")}
          </Button>
        </div>
      </CardRow>
    </Card>
  );
}

export function GeneralSection() {
  const { t, i18n } = useTranslation();
  const settings = useStore((s) => s.settings);
  const setSetting = useStore((s) => s.setSetting);
  const [theme, setThemeState] = useState<Theme>(getTheme());
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [global, setGlobal] = useState<MemoryEntry[]>([]);
  const [draft, setDraft] = useState("");
  useEffect(() => void api.appInfo().then(setInfo), []);
  const loadGlobal = () => void api.listMemory("global").then(setGlobal);
  useEffect(loadGlobal, []);

  const sharedOn = settings.global_memory_enabled !== "false";

  return (
    <>
      <PageTitle title={t("settings.nav.general")} />
      <Card title={t("settings.general.appearance")}>
        <CardRow label={t("settings.general.language")}>
          <Select
            className="w-44"
            value={i18n.language.startsWith("es") ? "es" : "en"}
            onChange={(v) => {
              setLanguage(v);
              void setSetting("language", v);
            }}
            options={LANGUAGES.map((l) => ({ value: l.code, label: l.label }))}
          />
        </CardRow>
        <CardRow label={t("settings.general.theme")}>
          <Select
            className="w-44"
            value={theme}
            onChange={(v) => {
              setTheme(v as Theme);
              setThemeState(v as Theme);
              void setSetting("theme", v);
            }}
            options={[
              { value: "system", label: t("settings.general.themeSystem") },
              { value: "light", label: t("settings.general.themeLight") },
              { value: "dark", label: t("settings.general.themeDark") },
            ]}
          />
        </CardRow>
      </Card>
      <Card title={t("settings.general.behavior")}>
        <CardRow label={t("settings.general.closeToTray")}>
          <Toggle checked={settings.close_to_tray !== "false"} onChange={(v) => void setSetting("close_to_tray", String(v))} />
        </CardRow>
        <CardRow label={t("settings.general.autoOpenComputer")} hint={t("settings.general.autoOpenComputerHint")}>
          <Toggle checked={settings.auto_open_computer !== "false"} onChange={(v) => void setSetting("auto_open_computer", String(v))} />
        </CardRow>
        <CardRow label={t("settings.general.autoOpenArtifact")} hint={t("settings.general.autoOpenArtifactHint")}>
          <Toggle checked={settings.artifact_auto_open === "true"} onChange={(v) => void setSetting("artifact_auto_open", String(v))} />
        </CardRow>
      </Card>
      <DashboardCard />
      <Card title={t("settings.general.sharedMemory")}>
        <CardRow label={t("settings.general.sharedMemoryToggle")} hint={t("settings.general.sharedMemoryHint")}>
          <Toggle checked={sharedOn} onChange={(v) => void setSetting("global_memory_enabled", String(v))} />
        </CardRow>
        {sharedOn && (
          <div className="space-y-1.5 px-4 py-3">
            {global.map((e) => (
              <div key={`${e.id}-${e.text}`} className="group flex items-center gap-2 rounded-lg bg-hover/60 px-2.5 py-1.5 text-[13px]">
                <span className="selectable flex-1">{e.text}</span>
                <button onClick={() => void api.deleteMemory("global", e.id).then(loadGlobal)} className="text-[12px] text-faint opacity-0 group-hover:opacity-100 hover:text-danger">
                  {t("common.remove")}
                </button>
              </div>
            ))}
            <div className="flex gap-2 pt-1">
              <Input
                value={draft}
                onChange={(e) => setDraft(e.target.value)}
                placeholder={t("profile.memoryAdd")}
                onKeyDown={async (e) => {
                  if (e.key === "Enter" && draft.trim()) {
                    await api.addMemory("global", draft.trim());
                    setDraft("");
                    loadGlobal();
                  }
                }}
              />
            </div>
          </div>
        )}
      </Card>
      <Card title={t("walkthrough.title")}>
        <CardRow label={t("walkthrough.replay")} hint={t("walkthrough.replayHint")}>
          <Button
            size="sm"
            onClick={() => {
              useStore.getState().closeSettings();
              useStore.getState().setDialog("walkthrough");
            }}
          >
            {t("walkthrough.start")}
          </Button>
        </CardRow>
      </Card>
      {info && (
        <Card title={t("settings.general.data")}>
          <CardRow label={t("settings.general.dataDir")} hint={info.data_dir}>
            <span className="text-[12px] text-faint">{t("settings.general.version", { v: info.version, platform: info.platform })}</span>
          </CardRow>
        </Card>
      )}
    </>
  );
}

export function AccountSection() {
  const { t } = useTranslation();
  const settings = useStore((s) => s.settings);
  const setSetting = useStore((s) => s.setSetting);
  const [name, setName] = useState(settings.user_name ?? "");
  const [about, setAbout] = useState(settings.about_me ?? "");
  const [saved, setSaved] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Saved as you type, so nothing is lost when the window closes.
  const onAbout = (v: string) => {
    setAbout(v);
    setSaved(false);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => void setSetting("about_me", v.trim()).then(() => setSaved(true)), 600);
  };

  return (
    <>
      <PageTitle title={t("settings.nav.account")} hint={t("settings.account.localOnly")} />
      <Card>
        <CardRow label={t("settings.account.name")} hint={t("settings.account.nameHint")}>
          <Input className="w-56" value={name} onChange={(e) => setName(e.target.value)} onBlur={() => void setSetting("user_name", name.trim())} />
        </CardRow>
      </Card>
      <Card title={t("settings.account.aboutTitle")} action={saved ? <span className="text-[11.5px] text-faint">{t("common.saved")}</span> : null}>
        <div className="space-y-2 p-3">
          <Textarea value={about} onChange={(e) => onAbout(e.target.value)} rows={7} placeholder={t("settings.account.aboutPlaceholder")} />
          <p className="text-[12px] text-muted">{t("settings.account.aboutHint")}</p>
        </div>
      </Card>
    </>
  );
}

export function UsageSection() {
  const { t } = useTranslation();
  const [rows, setRows] = useState<UsageRow[] | null>(null);
  useEffect(() => void api.usage().then(setRows), []);
  const total = (rows ?? []).reduce((a, r) => ({ i: a.i + r.input_tokens, o: a.o + r.output_tokens, n: a.n + r.messages }), { i: 0, o: 0, n: 0 });
  return (
    <>
      <PageTitle title={t("settings.usage.title")} hint={t("settings.usage.hint")} />
      {rows && rows.length === 0 && <p className="text-[13px] text-muted">{t("settings.usage.empty")}</p>}
      {rows && rows.length > 0 && (
        <>
          <div className="mb-5 grid grid-cols-3 gap-3">
            {[
              [t("settings.usage.messages"), total.n],
              [t("settings.usage.input"), total.i],
              [t("settings.usage.output"), total.o],
            ].map(([label, v]) => (
              <div key={label as string} className="rounded-xl border border-line px-4 py-3">
                <div className="text-[12px] text-muted">{label}</div>
                <div className="mt-1 text-[22px] font-semibold tabular-nums tracking-tight">{compact(v as number)}</div>
              </div>
            ))}
          </div>
          <div className="overflow-hidden rounded-xl border border-line">
            <table className="w-full text-[13px]">
              <thead className="bg-sidebar text-left text-[12px] text-muted">
                <tr>
                  <th className="px-4 py-2 font-medium">{t("settings.usage.agent")}</th>
                  <th className="px-4 py-2 text-right font-medium">{t("settings.usage.messages")}</th>
                  <th className="px-4 py-2 text-right font-medium">{t("settings.usage.input")}</th>
                  <th className="px-4 py-2 text-right font-medium">{t("settings.usage.output")}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-line">
                {rows.map((r) => (
                  <tr key={r.agent_id ?? "x"}>
                    <td className="px-4 py-2">{r.agent_name ?? <span className="text-muted">{t("settings.usage.removed")}</span>}</td>
                    <td className="px-4 py-2 text-right tabular-nums">{r.messages}</td>
                    <td className="px-4 py-2 text-right tabular-nums">{compact(r.input_tokens)}</td>
                    <td className="px-4 py-2 text-right tabular-nums">{compact(r.output_tokens)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
      <CliLimitsCard />
    </>
  );
}

/** "3h 20m", "2d 4h": how long until a limit window resets. */
function untilReset(at: number, lang: string): string {
  const secs = Math.max(0, at - Math.floor(Date.now() / 1000));
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const left = d > 0 ? `${d}d ${h}h` : h > 0 ? `${h}h ${m}m` : `${Math.max(1, m)}m`;
  const when = new Date(at * 1000).toLocaleString(lang, { weekday: "short", hour: "numeric", minute: "2-digit" });
  return `${left} · ${when}`;
}

/** The 5-hour and weekly limits of the subscription CLIs the user connected. */
function CliLimitsCard() {
  const { t, i18n } = useTranslation();
  const [rows, setRows] = useState<CliLimits[] | null>(null);
  const load = () => void api.cliLimits().then(setRows).catch(() => setRows([]));
  useEffect(load, []);
  if (!rows || rows.length === 0) return null;
  return (
    <div className="mt-7">
      <div className="mb-3 flex items-center gap-2">
        <h3 className="text-[14px] font-semibold">{t("settings.usage.limits")}</h3>
        <IconButton label={t("common.refresh")} onClick={load} className="size-7">
          <RefreshCw size={13} />
        </IconButton>
      </div>
      <p className="mb-3 text-[12.5px] text-muted">{t("settings.usage.limitsHint")}</p>
      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        {rows.map((c) => (
          <div key={c.cli} className="rounded-2xl border border-line p-4">
            <div className="mb-3 flex items-center gap-2">
              <span className="text-[14px] font-semibold">{c.name}</span>
              {c.plan && <span className="rounded-full bg-hover px-2 py-0.5 text-[11px] font-medium uppercase text-muted">{c.plan}</span>}
              {c.updated_at && <span className="ml-auto text-[11px] text-faint">{t("settings.usage.updated", { when: new Date(c.updated_at * 1000).toLocaleTimeString(i18n.language, { hour: "numeric", minute: "2-digit" }) })}</span>}
            </div>
            {c.windows.length === 0 ? (
              <p className="text-[12.5px] text-muted">{c.note}</p>
            ) : (
              <div className="space-y-4">
                {c.windows.map((w) => {
                  const used = w.used_percent;
                  const left = used === null ? null : Math.max(0, Math.round(100 - used));
                  const tone = used === null ? "bg-accent" : used >= 85 ? "bg-danger" : used >= 60 ? "bg-warning" : "bg-success";
                  return (
                    <div key={w.label}>
                      <div className="mb-1.5 flex items-baseline justify-between">
                        <span className="text-[12.5px] font-medium">{t(`settings.usage.window.${w.label}`, { defaultValue: w.label })}</span>
                        <span className="text-[12px] tabular-nums text-muted">
                          {left === null ? t("settings.usage.open") : <><span className="text-[15px] font-semibold text-ink">{left}%</span> {t("settings.usage.left")}</>}
                        </span>
                      </div>
                      <div className="h-2 overflow-hidden rounded-full bg-hover" role="progressbar" aria-valuenow={used ?? 0} aria-valuemin={0} aria-valuemax={100} aria-label={w.label}>
                        <div className={cx("h-full rounded-full transition-[width] duration-500", tone)} style={{ width: `${used === null ? 8 : Math.max(2, Math.min(100, used))}%` }} />
                      </div>
                      {w.resets_at && <div className="mt-1 text-[11.5px] text-faint">{t("settings.usage.resets", { when: untilReset(w.resets_at, i18n.language) })}</div>}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

export function InternetSection() {
  const { t } = useTranslation();
  const settings = useStore((s) => s.settings);
  const setSetting = useStore((s) => s.setSetting);
  const [url, setUrl] = useState(settings.searxng_url ?? "");
  const [q, setQ] = useState("");
  const [results, setResults] = useState<{ title: string; url: string; snippet: string }[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const search = async () => {
    if (!q.trim()) return;
    setBusy(true);
    setErr(null);
    try {
      setResults(await api.webSearch(q));
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <PageTitle title={t("settings.internet.title")} hint={t("settings.internet.hint")} />
      <Card>
        <CardRow label={t("settings.internet.searxng")} hint={t("settings.internet.searxngHint")}>
          <Input className="w-64" value={url} placeholder="http://localhost:8888" onChange={(e) => setUrl(e.target.value)} onBlur={() => void setSetting("searxng_url", url.trim())} />
        </CardRow>
      </Card>
      <Card title={t("settings.internet.tryIt")}>
        <div className="flex gap-2 p-3">
          <Input value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void search()} placeholder={t("settings.internet.tryPlaceholder")} />
          <Button onClick={() => void search()} disabled={busy}>
            <Search size={14} />
          </Button>
        </div>
        {err && <p className="px-4 py-3 text-[12.5px] text-danger">{err}</p>}
        {results?.map((r) => (
          <div key={r.url} className="px-4 py-2.5">
            <div className="text-[13.5px] font-medium">{r.title}</div>
            <div className="truncate text-[12px] text-accent">{r.url}</div>
            <div className="mt-0.5 line-clamp-2 text-[12.5px] text-muted">{r.snippet}</div>
          </div>
        ))}
      </Card>
    </>
  );
}

function UpdateRow() {
  const { t } = useTranslation();
  const update = useStore((s) => s.update);
  const autoUpdate = useStore((s) => s.settings.auto_update !== "false");
  const setSetting = useStore((s) => s.setSetting);
  const [state, setState] = useState<{ kind: "idle" | "checking" | "latest" | "installing" | "error"; text?: string; pct?: number }>({ kind: "idle" });

  const nonce = useStore((s) => s.updateCheckNonce);
  useEffect(() => {
    if (nonce > 0) void check();
  }, [nonce]); // eslint-disable-line react-hooks/exhaustive-deps

  const check = async () => {
    setState({ kind: "checking" });
    try {
      const found = await checkForUpdate();
      useStore.setState({ update: found });
      setState({ kind: found ? "idle" : "latest" });
    } catch (e) {
      setState({ kind: "error", text: String(e) });
    }
  };

  const install = async () => {
    if (!update) return;
    setState({ kind: "installing", pct: 0 });
    try {
      await update.install((pct) => setState({ kind: "installing", pct }));
    } catch (e) {
      setState({ kind: "error", text: String(e) });
    }
  };

  return (
    <>
      <CardRow
        label={t("update.title")}
        hint={
          state.kind === "error"
            ? state.text
            : update
              ? t("update.available", { version: update.version })
              : state.kind === "latest"
                ? t("update.latest")
                : state.kind === "installing"
                  ? t("update.installing", { pct: state.pct ?? 0 })
                  : t("update.hint")
        }
      >
        {update ? (
          <Button size="sm" variant="primary" onClick={() => void install()} disabled={state.kind === "installing"}>
            {t("update.install")}
          </Button>
        ) : (
          <Button size="sm" onClick={() => void check()} disabled={state.kind === "checking"}>
            {state.kind === "checking" ? t("update.checking") : t("update.check")}
          </Button>
        )}
      </CardRow>
      <CardRow label={t("update.auto")}>
        <Toggle checked={autoUpdate} onChange={(v) => void setSetting("auto_update", v ? "true" : "false")} />
      </CardRow>
    </>
  );
}

export function AboutSection() {
  const { t } = useTranslation();
  const [info, setInfo] = useState<AppInfo | null>(null);
  useEffect(() => void api.appInfo().then(setInfo), []);
  const links = [
    { label: t("settings.about.website"), url: "https://lautaro005.github.io/opencue/" },
    { label: "GitHub", url: "https://github.com/Lautaro005/opencue" },
    { label: t("settings.about.changelog"), url: "https://lautaro005.github.io/opencue/changelog/" },
    { label: t("settings.about.privacy"), url: "https://lautaro005.github.io/opencue/privacy/" },
  ];
  return (
    <>
      <PageTitle title={t("settings.nav.about")} hint={t("settings.about.hint")} />
      <Card>
        <CardRow label={t("settings.about.version")}>
          <span className="font-mono text-[12.5px] text-muted">{info?.version}</span>
        </CardRow>
        <UpdateRow />
        <CardRow label={t("settings.about.dataDir")}>
          <span className="selectable max-w-[320px] truncate font-mono text-[12px] text-muted">{info?.data_dir}</span>
        </CardRow>
        <CardRow label={t("settings.about.license")} hint={t("settings.about.licenseHint")}>
          <span className="text-[12.5px] text-muted">Apache-2.0 + Commons Clause</span>
        </CardRow>
      </Card>
      <Card title={t("settings.about.help")}>
        <div className="flex items-center gap-3 px-4 py-3.5">
          <span className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-accent-soft text-accent">
            <LifeBuoy size={17} />
          </span>
          <p className="min-w-0 flex-1 text-[13px] text-muted">{t("settings.about.helpHint")}</p>
          <Button size="sm" variant="primary" onClick={() => void openUrl("https://github.com/Lautaro005/opencue/issues/new")}>
            {t("settings.about.reportIssue")} <ArrowUpRight size={13} />
          </Button>
          <Button size="sm" onClick={() => void openUrl("https://lautaro005.github.io/opencue/docs/")}>
            {t("settings.about.docs")}
          </Button>
        </div>
      </Card>
      <div className="flex flex-wrap gap-2">
        {links.map((l) => (
          <Button key={l.url} size="sm" variant="ghost" onClick={() => void openUrl(l.url)}>
            {l.label} <ArrowUpRight size={12} className="text-faint" />
          </Button>
        ))}
      </div>
    </>
  );
}
