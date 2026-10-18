import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search } from "lucide-react";
import { useStore } from "../../lib/store";
import { api, checkForUpdate, openUrl } from "../../lib/api";
import type { AppInfo, MemoryEntry, UsageRow } from "../../lib/types";
import { compact } from "../../lib/format";
import { LANGUAGES, setLanguage } from "../../i18n";
import { getTheme, setTheme, type Theme } from "../../lib/theme";
import { Button, Input, Select, Textarea, Toggle } from "../ui";
import { Card, CardRow, PageTitle } from "./SettingsModal";

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
      </Card>
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
    </>
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
      <div className="flex flex-wrap gap-2">
        {links.map((l) => (
          <Button key={l.url} size="sm" onClick={() => void openUrl(l.url)}>
            {l.label}
          </Button>
        ))}
      </div>
    </>
  );
}
