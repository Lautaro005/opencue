import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Download, ExternalLink, Play, Square, Trash2 } from "lucide-react";
import { api, openUrl } from "../../lib/api";
import { useStore } from "../../lib/store";
import { cx, formatBytes } from "../../lib/format";
import type { LocalModel, PersonaStatus, VoiceSettings } from "../../lib/types";
import { Badge, Button, Input, Select, Spinner, Toggle } from "../ui";
import { Card, CardRow } from "./SettingsModal";

/** Kokoro voices by language (the names the core maps to speakers). */
const VOICES: { lang: string; name: string }[] = [
  { lang: "es", name: "ef_dora" },
  { lang: "es", name: "em_alex" },
  { lang: "es", name: "em_santa" },
  { lang: "en", name: "af_heart" },
  { lang: "en", name: "af_bella" },
  { lang: "en", name: "am_michael" },
  { lang: "en", name: "bf_emma" },
  { lang: "en", name: "bm_george" },
  { lang: "fr", name: "ff_siwis" },
  { lang: "it", name: "if_sara" },
  { lang: "it", name: "im_nicola" },
  { lang: "pt", name: "pf_dora" },
  { lang: "pt", name: "pm_alex" },
  { lang: "hi", name: "hf_alpha" },
  { lang: "ja", name: "jf_alpha" },
  { lang: "zh", name: "zf_xiaobei" },
];

/** Settings → Models → Local: speech models that run on this computer, and PersonaPlex. */
export function LocalModels() {
  const { t } = useTranslation();
  const [models, setModels] = useState<LocalModel[]>([]);
  const [voice, setVoice] = useState<VoiceSettings | null>(null);
  const progress = useStore((s) => s.modelProgress);
  const showToast = useStore((s) => s.showToast);

  const refresh = useCallback(() => {
    void api.modelsStatus().then(setModels).catch((e) => showToast(String(e)));
    void api.voiceSettings().then(setVoice).catch(() => {});
  }, [showToast]);
  useEffect(() => {
    refresh();
    window.addEventListener("opencue:models-changed", refresh);
    return () => window.removeEventListener("opencue:models-changed", refresh);
  }, [refresh]);

  const save = async (patch: Partial<VoiceSettings>) => {
    if (!voice) return;
    setVoice(await api.saveVoiceSettings({ ...voice, ...patch }));
  };
  const stt = models.filter((m) => m.kind === "stt");
  const tts = models.filter((m) => m.kind === "tts");
  const runtime = models.find((m) => m.kind === "runtime");
  const supported = runtime?.supported ?? true;

  return (
    <>
      <p className="mb-5 text-[13px] text-muted">{t("settings.local.hint")}</p>
      {!supported && <p className="mb-4 rounded-lg bg-warning-soft px-3 py-2 text-[12.5px]">{t("settings.local.unsupported")}</p>}

      <Card title={t("settings.local.stt")}>
        {stt.map((m) => (
          <ModelRow key={m.id} m={m} p={progress[m.id]} chosen={(voice?.local_stt || "parakeet-v3") === m.id} onUse={() => void save({ local_stt: m.id, stt_engine: "local" })} onChange={refresh} />
        ))}
      </Card>

      <Card title={t("settings.local.tts")}>
        {tts.map((m) => (
          <ModelRow key={m.id} m={m} p={progress[m.id]} chosen={(voice?.local_tts || "kokoro-v1") === m.id} onUse={() => void save({ local_tts: m.id, tts_engine: "local" })} onChange={refresh} />
        ))}
        <CardRow label={t("settings.local.voice")} hint={t("settings.local.voiceHint")}>
          <div className="flex items-center gap-1.5">
            <Select
              className="w-44"
              value={voice?.local_voice ?? ""}
              onChange={(v) => void save({ local_voice: v })}
              options={[{ value: "", label: t("settings.local.voiceAuto") }, ...VOICES.map((v) => ({ value: v.name, label: `${v.name} · ${v.lang}` }))]}
            />
            <TestVoice />
          </div>
        </CardRow>
      </Card>

      {runtime && (
        <Card title={t("settings.local.runtime")}>
          <ModelRow m={runtime} p={progress[runtime.id]} onChange={refresh} runtimeRow />
        </Card>
      )}

      <PersonaPlex />
    </>
  );
}

function TestVoice() {
  const { t } = useTranslation();
  const [busy, setBusy] = useState(false);
  const showToast = useStore((s) => s.showToast);
  const run = async () => {
    setBusy(true);
    try {
      const sp = await api.speak(t("settings.local.sample"));
      if (sp.data) await new Audio(`data:${sp.mime};base64,${sp.data}`).play();
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <Button size="sm" onClick={() => void run()} disabled={busy}>
      {busy ? <Spinner size={12} /> : <Play size={12} />} {t("settings.local.test")}
    </Button>
  );
}

function ModelRow({ m, p, chosen, onUse, onChange, runtimeRow }: { m: LocalModel; p?: { phase: string; done: number; total: number; error: string | null }; chosen?: boolean; onUse?: () => void; onChange: () => void; runtimeRow?: boolean }) {
  const { t } = useTranslation();
  const showToast = useStore((s) => s.showToast);
  const working = p?.phase === "downloading" || p?.phase === "extracting";
  const pct = p && p.total > 0 ? Math.min(100, Math.round((p.done / p.total) * 100)) : 0;
  const act = (run: () => Promise<void>) => void run().then(onChange).catch((e) => showToast(String(e)));
  return (
    <div className="px-4 py-3">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-1.5">
            <span className="text-[13.5px] font-medium">{m.label}</span>
            <Badge>{formatBytes(m.size)}</Badge>
            <Badge>{m.license}</Badge>
            {m.installed && !runtimeRow && chosen && <Badge tone="success"><Check size={11} strokeWidth={3} /> {t("settings.local.inUse")}</Badge>}
            {m.installed && (runtimeRow || !chosen) && <Badge tone="success">{t("settings.local.downloaded")}</Badge>}
          </div>
          <p className="mt-0.5 text-[12px] text-muted">{m.note}</p>
          {m.languages.length > 0 && <p className="mt-0.5 truncate text-[11.5px] text-faint">{m.languages.length > 8 ? t("settings.local.languages", { n: m.languages.length }) : m.languages.join(" · ")} · {t("settings.local.ram", { gb: m.ram_gb })}</p>}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {working ? (
            <Button size="sm" onClick={() => act(() => api.modelCancel(m.id))}>
              <Square size={11} fill="currentColor" /> {t("common.cancel")}
            </Button>
          ) : m.installed ? (
            <>
              {onUse && !chosen && (
                <Button size="sm" variant="primary" onClick={onUse}>
                  {t("settings.local.use")}
                </Button>
              )}
              <Button size="sm" variant="ghost" onClick={() => act(() => api.modelDelete(m.id))} aria-label={t("common.remove")}>
                <Trash2 size={13} />
              </Button>
            </>
          ) : (
            <Button size="sm" onClick={() => act(() => api.modelInstall(m.id))} disabled={!m.supported}>
              <Download size={13} /> {t("settings.local.download")}
            </Button>
          )}
        </div>
      </div>
      {working && (
        <div className="mt-2.5">
          <div className="h-1.5 overflow-hidden rounded-full bg-hover">
            <div className={cx("h-full rounded-full bg-accent transition-[width]", p?.phase === "extracting" && "animate-pulse")} style={{ width: `${p?.phase === "extracting" ? 100 : pct}%` }} />
          </div>
          <p className="mt-1 text-[11.5px] text-muted">{p?.phase === "extracting" ? t("settings.local.unpacking") : `${formatBytes(p?.done ?? 0)} / ${formatBytes(p?.total ?? m.size)} · ${pct}%`}</p>
        </div>
      )}
      {p?.phase === "error" && p.error && <p className="selectable mt-2 line-clamp-3 text-[12px] text-danger">{p.error}</p>}
    </div>
  );
}

/** NVIDIA PersonaPlex: install it, start its server, open the call here or on a phone. */
function PersonaPlex() {
  const { t } = useTranslation();
  const [s, setS] = useState<PersonaStatus | null>(null);
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);
  const showToast = useStore((s2) => s2.showToast);
  const log = useRef<HTMLPreElement>(null);

  const refresh = useCallback(() => void api.personaStatus().then(setS).catch(() => {}), []);
  useEffect(() => {
    refresh();
    const timer = window.setInterval(refresh, 2000);
    return () => window.clearInterval(timer);
  }, [refresh]);
  useEffect(() => {
    if (log.current) log.current.scrollTop = log.current.scrollHeight;
  }, [s?.log]);
  if (!s) return null;

  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await fn();
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
      refresh();
    }
  };
  const configure = (patch: { lan?: boolean; offload?: boolean; token?: string; engine?: string }) => run(() => api.personaConfigure(patch.lan ?? s.lan, patch.offload ?? s.offload, patch.token, patch.engine));
  const mlx = s.engine === "moshi_mlx";

  return (
    <Card
      title={t("settings.local.persona.title")}
      action={
        <button onClick={() => void openUrl(mlx ? "https://github.com/kyutai-labs/moshi" : "https://github.com/NVIDIA/personaplex")} className="flex items-center gap-1 text-[12px] text-muted hover:text-accent">
          GitHub <ExternalLink size={12} />
        </button>
      }
    >
      <div className="grid grid-cols-2 gap-2 px-4 pt-3" role="radiogroup" aria-label={t("settings.local.persona.engine")}>
        {(["moshi_mlx", "personaplex"] as const).map((k) => {
          const off = k === "moshi_mlx" && !s.mlx_supported;
          return (
            <button
              key={k}
              role="radio"
              aria-checked={s.engine === k}
              disabled={busy || off || s.running || s.installing}
              onClick={() => s.engine !== k && void configure({ engine: k })}
              className={`rounded-xl border px-3 py-2 text-left transition-colors ${s.engine === k ? "border-accent bg-accent-soft" : "border-line hover:bg-hover"} ${off ? "opacity-50" : ""}`}
            >
              <span className="block text-[13px] font-medium">{t(`settings.local.persona.engines.${k}`)}</span>
              <span className="block text-[11.5px] leading-snug text-muted">{off ? t("settings.local.persona.needsMac") : t(`settings.local.persona.engines.${k}Hint`)}</span>
            </button>
          );
        })}
      </div>
      <div className="px-4 py-3">
        <p className="text-[12.5px] text-muted">{mlx ? t("settings.local.persona.aboutMlx") : t("settings.local.persona.about")}</p>
        <div className="mt-2 flex flex-wrap gap-1.5">
          {!mlx && <Badge tone={s.gpu ? "success" : "warning"}>{s.gpu ? s.gpu : t("settings.local.persona.noGpu")}</Badge>}
          <Badge tone={s.python ? "success" : "warning"}>{s.python ? `Python ${s.python}` : t("settings.local.persona.noPython")}</Badge>
          <Badge>{mlx ? "CC-BY 4.0 · Kyutai" : "MIT · NVIDIA Open Model"}</Badge>
        </div>
      </div>
      {!mlx && (
      <CardRow label={t("settings.local.persona.token")} hint={s.has_token ? t("settings.local.persona.tokenSaved") : t("settings.local.persona.tokenHint")}>
        <div className="flex gap-1.5">
          <Input type="password" className="w-52" value={token} placeholder={s.has_token ? "••••••••" : "hf_…"} onChange={(e) => setToken(e.target.value)} />
          <Button disabled={!token || busy} onClick={() => void configure({ token }).then(() => setToken(""))}>
            {t("common.save")}
          </Button>
          <Button variant="ghost" onClick={() => void openUrl("https://huggingface.co/nvidia/personaplex-7b-v1")}>
            <ExternalLink size={13} />
          </Button>
        </div>
      </CardRow>
      )}
      {!mlx && (
        <>
          <CardRow label={t("settings.local.persona.lan")} hint={t("settings.local.persona.lanHint")}>
            <Toggle checked={s.lan} onChange={(v) => void configure({ lan: v })} label={t("settings.local.persona.lan")} />
          </CardRow>
          <CardRow label={t("settings.local.persona.offload")} hint={t("settings.local.persona.offloadHint")}>
            <Toggle checked={s.offload} onChange={(v) => void configure({ offload: v })} label={t("settings.local.persona.offload")} />
          </CardRow>
        </>
      )}
      <div className="flex flex-wrap items-center gap-2 px-4 py-3">
        {!s.installed ? (
          <Button variant="primary" disabled={busy || s.installing || !s.python} onClick={() => void run(() => api.personaInstall())}>
            {s.installing ? <Spinner size={13} /> : <Download size={13} />} {s.installing ? t("settings.local.persona.installing") : t("settings.local.persona.install")}
          </Button>
        ) : s.running ? (
          <>
            <Button onClick={() => void run(() => api.personaStop())}>
              <Square size={11} fill="currentColor" /> {t("settings.local.persona.stop")}
            </Button>
            <Button variant="primary" disabled={!s.ready} onClick={() => s.url && void openUrl(s.url)}>
              {s.ready ? <ExternalLink size={13} /> : <Spinner size={13} />} {s.ready ? t("settings.local.persona.open") : t("settings.local.persona.starting")}
            </Button>
          </>
        ) : (
          <Button variant="primary" disabled={busy || (!mlx && !s.has_token)} onClick={() => void run(() => api.personaStart())}>
            <Play size={13} /> {t("settings.local.persona.start")}
          </Button>
        )}
        {s.installed && !s.running && !s.installing && (
          <Button variant="ghost" onClick={() => void run(() => api.personaInstall())}>
            {t("settings.local.persona.reinstall")}
          </Button>
        )}
      </div>
      {s.lan && s.running && s.lan_url && (
        <div className="flex items-center gap-4 px-4 py-3">
          {s.qr_svg && <div className="size-28 shrink-0 overflow-hidden rounded-lg bg-white [&_svg]:size-full" dangerouslySetInnerHTML={{ __html: s.qr_svg }} />}
          <div className="min-w-0">
            <p className="text-[13px] font-medium">{t("settings.local.persona.phone")}</p>
            <p className="selectable mt-0.5 truncate font-mono text-[12px] text-muted">{s.lan_url}</p>
            <p className="mt-1 text-[11.5px] text-faint">{t("settings.local.persona.phoneHint")}</p>
          </div>
        </div>
      )}
      {s.log && (
        <pre ref={log} className="selectable max-h-40 overflow-auto bg-hover/50 px-4 py-2.5 font-mono text-[11px] leading-[1.5] whitespace-pre-wrap text-muted">
          {s.log}
        </pre>
      )}
    </Card>
  );
}
