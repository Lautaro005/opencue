import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronDown, KeyRound, Plus, Star, Terminal, Trash2 } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import type { ProviderConfig, ProviderInfo } from "../../lib/types";
import { cx } from "../../lib/format";
import { Badge, Button, Field, Input, Spinner, Toggle } from "../ui";
import { PageTitle } from "./SettingsModal";

export function ModelsSection() {
  const { t } = useTranslation();
  const providers = useStore((s) => s.providers);
  const refreshProviders = useStore((s) => s.refreshProviders);
  const settings = useStore((s) => s.settings);
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);
  const [adding, setAdding] = useState(false);
  const [expanded, setExpanded] = useState<string | null>(null);

  useEffect(() => void api.providerCatalog().then(setCatalog), []);

  const add = async (info: ProviderInfo) => {
    const p = await api.saveProvider({
      id: "",
      kind: info.kind,
      name: info.label,
      base_url: null,
      default_model: info.default_model,
      enabled: true,
      has_key: false,
    });
    await refreshProviders();
    setAdding(false);
    setExpanded(p.id);
  };

  return (
    <>
      <PageTitle title={t("settings.models.title")} hint={t("settings.models.hint")} />
      <div className="space-y-2">
        {providers.map((p) => (
          <ProviderCard
            key={p.id}
            provider={p}
            info={catalog.find((c) => c.kind === p.kind)}
            open={expanded === p.id}
            onToggle={() => setExpanded(expanded === p.id ? null : p.id)}
            isDefault={settings.default_provider ? settings.default_provider === p.id : providers.find((x) => x.enabled)?.id === p.id}
          />
        ))}
      </div>

      {adding ? (
        <div className="animate-pop mt-4 rounded-xl border border-line p-3">
          <div className="mb-2 flex items-center justify-between px-1">
            <span className="text-[12.5px] font-semibold text-muted">{t("settings.models.choose")}</span>
            <Button variant="ghost" size="sm" onClick={() => setAdding(false)}>
              {t("common.cancel")}
            </Button>
          </div>
          <div className="grid grid-cols-2 gap-1.5">
            {catalog.map((c) => (
              <button key={c.kind} onClick={() => void add(c)} className="flex items-start gap-2.5 rounded-lg border border-line px-3 py-2 text-left hover:border-line-strong hover:bg-hover">
                <span className="mt-0.5 text-muted">{c.api === "cli" ? <Terminal size={15} /> : <KeyRound size={15} />}</span>
                <span className="min-w-0">
                  <span className="block text-[13px] font-medium">{c.label}</span>
                  <span className="line-clamp-2 block text-[11.5px] text-muted">{c.note}</span>
                </span>
              </button>
            ))}
          </div>
        </div>
      ) : (
        <Button className="mt-4" onClick={() => setAdding(true)}>
          <Plus size={15} /> {t("settings.models.add")}
        </Button>
      )}
    </>
  );
}

function ProviderCard({ provider, info, open, onToggle, isDefault }: { provider: ProviderConfig; info?: ProviderInfo; open: boolean; onToggle: () => void; isDefault: boolean }) {
  const { t } = useTranslation();
  const refreshProviders = useStore((s) => s.refreshProviders);
  const setSetting = useStore((s) => s.setSetting);
  const [form, setForm] = useState(provider);
  const [key, setKey] = useState("");
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  const [testing, setTesting] = useState(false);
  useEffect(() => setForm(provider), [provider]);

  const isCli = info?.api === "cli";
  const needsKey = info?.requires_key && !provider.has_key;

  const save = async (patch: Partial<ProviderConfig> = {}, apiKey?: string) => {
    const next = { ...form, ...patch };
    setForm(next);
    await api.saveProvider(next, apiKey);
    await refreshProviders();
  };

  const runTest = async () => {
    setTesting(true);
    setTest(null);
    try {
      const models = await api.listModels(provider.id);
      setTest({ ok: true, text: t("settings.models.testOk", { n: models.length }) });
    } catch (e) {
      setTest({ ok: false, text: String(e) });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className={cx("rounded-xl border transition-colors", open ? "border-line-strong" : "border-line")}>
      <button onClick={onToggle} className="flex w-full items-center gap-3 px-4 py-3 text-left">
        <span className={cx("flex size-8 items-center justify-center rounded-lg", provider.enabled ? "bg-accent-soft text-accent" : "bg-hover text-faint")}>
          {isCli ? <Terminal size={16} /> : <KeyRound size={16} />}
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-2">
            <span className="truncate text-[13.5px] font-medium">{provider.name}</span>
            {isDefault && <Badge tone="accent">{t("settings.models.isDefault")}</Badge>}
            {needsKey && <Badge tone="warning">{t("settings.models.key")}</Badge>}
          </span>
          <span className="block truncate text-[12px] text-muted">{provider.default_model || info?.default_base_url || info?.note}</span>
        </span>
        <ChevronDown size={16} className={cx("text-faint transition-transform", open && "rotate-180")} />
      </button>

      {open && (
        <div className="space-y-3.5 border-t border-line px-4 py-4">
          {isCli && <p className="rounded-lg bg-hover px-3 py-2 text-[12.5px] text-muted">{t("settings.models.cliHint")}</p>}
          {info?.requires_key || (!isCli && !info?.default_base_url?.startsWith("http://localhost")) ? (
            <Field label={t("settings.models.key")} hint={provider.has_key ? t("settings.models.keySaved") : info?.note}>
              <div className="flex gap-2">
                <Input type="password" value={key} onChange={(e) => setKey(e.target.value)} placeholder={provider.has_key ? "••••••••••••" : t("settings.models.keyPlaceholder")} autoComplete="off" />
                <Button
                  disabled={!key.trim()}
                  onClick={async () => {
                    await save({}, key.trim());
                    setKey("");
                  }}
                >
                  {provider.has_key ? t("settings.models.replaceKey") : t("common.save")}
                </Button>
              </div>
            </Field>
          ) : null}
          {!isCli && (
            <div className="grid grid-cols-2 gap-3">
              <Field label={t("settings.models.baseUrl")}>
                <Input value={form.base_url ?? ""} placeholder={info?.default_base_url ?? "https://…/v1"} onChange={(e) => setForm({ ...form, base_url: e.target.value })} onBlur={() => void save()} />
              </Field>
              <Field label={t("settings.models.defaultModel")}>
                <Input value={form.default_model ?? ""} placeholder={info?.default_model ?? ""} onChange={(e) => setForm({ ...form, default_model: e.target.value })} onBlur={() => void save()} />
              </Field>
            </div>
          )}
          <div className="flex flex-wrap items-center gap-2">
            <Button size="sm" onClick={() => void runTest()} disabled={testing}>
              {testing ? <Spinner size={12} /> : <Check size={13} />} {t("settings.models.test")}
            </Button>
            {!isDefault && (
              <Button size="sm" variant="ghost" onClick={() => void setSetting("default_provider", provider.id)}>
                <Star size={13} /> {t("settings.models.useDefault")}
              </Button>
            )}
            <label className="ml-auto flex items-center gap-2 text-[12.5px] text-muted">
              {t("settings.models.enabled")}
              <Toggle checked={form.enabled} onChange={(v) => void save({ enabled: v })} />
            </label>
            <Button
              size="sm"
              variant="danger"
              onClick={async () => {
                if (confirm(t("settings.models.removeConfirm", { name: provider.name }))) {
                  await api.deleteProvider(provider.id);
                  await refreshProviders();
                }
              }}
            >
              <Trash2 size={13} />
            </Button>
          </div>
          {test && <p className={cx("selectable text-[12px]", test.ok ? "text-success" : "text-danger")}>{test.text}</p>}
        </div>
      )}
    </div>
  );
}
