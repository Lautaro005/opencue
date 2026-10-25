import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Plus, Trash2 } from "lucide-react";
import { ProviderLogo } from "../ProviderLogo";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import { confirmDialog } from "../../lib/confirm";
import { isConfigured } from "../../lib/models";
import type { ProviderConfig, ProviderInfo } from "../../lib/types";
import { cx } from "../../lib/format";
import { Badge, Button, DialogHeader, Field, Input, Modal, Spinner, Toggle } from "../ui";
import { PageTitle } from "./SettingsModal";

/** One container per provider the app supports; click one to set it up. */
export function ProvidersSection() {
  const { t } = useTranslation();
  const providers = useStore((s) => s.providers);
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);
  const [editing, setEditing] = useState<{ provider: ProviderConfig | null; info: ProviderInfo } | null>(null);

  useEffect(() => void api.providerCatalog().then(setCatalog), []);

  const tiles: { info: ProviderInfo; provider: ProviderConfig | null; key: string }[] = [];
  for (const info of catalog) {
    const mine = providers.filter((p) => p.kind === info.kind);
    for (const p of mine) tiles.push({ info, provider: p, key: p.id });
    if (mine.length === 0 || info.kind === "openai_compatible") tiles.push({ info, provider: null, key: `new-${info.kind}` });
  }

  return (
    <>
      <PageTitle title={t("settings.providers.title")} hint={t("settings.providers.hint")} />
      <div className="grid grid-cols-2 gap-2.5">
        {tiles.map(({ info, provider, key }) => {
          const ready = provider ? isConfigured(provider, info) : false;
          return (
            <button
              key={key}
              onClick={() => setEditing({ provider, info })}
              className={cx(
                "flex min-h-[92px] flex-col items-start gap-2 rounded-xl border p-3.5 text-left transition-colors hover:bg-hover",
                ready ? "border-line-strong" : "border-line",
              )}
            >
              <span className="flex w-full items-center gap-2.5">
                <span className={cx("flex size-8 shrink-0 items-center justify-center rounded-lg", ready ? "bg-accent-soft" : "bg-hover")}>
                  <ProviderLogo kind={info.kind} size={18} />
                </span>
                <span className="min-w-0 flex-1 truncate text-[13.5px] font-medium">{provider && provider.kind === "openai_compatible" ? provider.name : info.label}</span>
                {provider ? (
                  ready ? (
                    <Badge tone="success">
                      <Check size={11} strokeWidth={3} /> {t("settings.providers.connected")}
                    </Badge>
                  ) : !provider.enabled ? (
                    <Badge>{t("common.off")}</Badge>
                  ) : (
                    <Badge tone="warning">{t("settings.models.key")}</Badge>
                  )
                ) : (
                  <span className="flex items-center gap-1 text-[12px] text-muted">
                    <Plus size={13} /> {t("settings.providers.setUp")}
                  </span>
                )}
              </span>
              <span className="line-clamp-2 text-[12px] text-muted">{info.note}</span>
            </button>
          );
        })}
      </div>
      {editing && <ProviderDialog key={editing.provider?.id ?? editing.info.kind} {...editing} onClose={() => setEditing(null)} />}
    </>
  );
}

export function ProviderDialog({ provider, info, onClose, onSaved }: { provider: ProviderConfig | null; info: ProviderInfo; onClose: () => void; onSaved?: (p: ProviderConfig) => void }) {
  const { t } = useTranslation();
  const refreshProviders = useStore((s) => s.refreshProviders);
  const loadModels = useStore((s) => s.loadModels);
  const [form, setForm] = useState<ProviderConfig>(
    provider ?? { id: "", kind: info.kind, name: info.label, base_url: null, default_model: info.default_model, enabled: true, has_key: false },
  );
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ ok: boolean; text: string } | null>(null);

  const isCli = info.api === "cli";
  const remote = info.requires_key || (!isCli && !info.default_base_url?.startsWith("http://localhost"));
  const set = (p: Partial<ProviderConfig>) => setForm((f) => ({ ...f, ...p }));

  const save = async () => {
    setBusy(true);
    setResult(null);
    try {
      const saved = await api.saveProvider({ ...form, base_url: form.base_url?.trim() || null, default_model: form.default_model?.trim() || null }, key.trim() || undefined);
      setKey("");
      setForm(saved);
      await refreshProviders();
      try {
        // Checking the connection also fills the model list for the rest of the app.
        await loadModels(saved.id, true);
        const found = useStore.getState().models[saved.id];
        if (found?.error) throw new Error(found.error);
        setResult({ ok: true, text: t("settings.models.testOk", { n: found?.list.length ?? 0 }) });
      } catch (e) {
        setResult({ ok: false, text: String(e) });
      }
      onSaved?.(saved);
    } catch (e) {
      setResult({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!provider) return;
    if (await confirmDialog(t("settings.models.removeConfirm", { name: provider.name }), { confirmLabel: t("common.remove") })) {
      await api.deleteProvider(provider.id);
      await refreshProviders();
      onClose();
    }
  };

  return (
    <Modal open onClose={onClose} className="w-[480px]" label={info.label}>
      <DialogHeader title={info.label} subtitle={info.note} onClose={onClose} />
      <div className="max-h-[calc(100vh-170px)] space-y-3.5 overflow-y-auto px-6 pt-2 pb-5">
        {isCli && <p className="rounded-lg bg-hover px-3 py-2 text-[12.5px] text-muted">{t("settings.models.cliHint")}</p>}
        {info.kind === "openai_compatible" && (
          <Field label={t("dialogs.name")}>
            <Input value={form.name} onChange={(e) => set({ name: e.target.value })} />
          </Field>
        )}
        {remote && (
          <Field label={t("settings.models.key")} hint={form.has_key ? t("settings.models.keySaved") : undefined}>
            <Input type="password" value={key} onChange={(e) => setKey(e.target.value)} placeholder={form.has_key ? "••••••••••••" : t("settings.models.keyPlaceholder")} autoComplete="off" autoFocus />
          </Field>
        )}
        {!isCli && (
          <div className="grid grid-cols-2 gap-3">
            <Field label={t("settings.models.baseUrl")}>
              <Input value={form.base_url ?? ""} placeholder={info.default_base_url ?? "https://…/v1"} onChange={(e) => set({ base_url: e.target.value })} />
            </Field>
            <Field label={t("settings.models.defaultModel")}>
              <Input value={form.default_model ?? ""} placeholder={info.default_model ?? ""} onChange={(e) => set({ default_model: e.target.value })} />
            </Field>
          </div>
        )}
        <label className="flex items-center justify-between gap-2 text-[13px]">
          {t("settings.models.enabled")}
          <Toggle checked={form.enabled} onChange={(v) => set({ enabled: v })} />
        </label>
        {result && <p className={cx("selectable text-[12px]", result.ok ? "text-success" : "text-danger")}>{result.text}</p>}
        <div className="flex items-center gap-2 pt-1">
          {provider && (
            <Button variant="danger" onClick={() => void remove()}>
              <Trash2 size={14} /> {t("common.remove")}
            </Button>
          )}
          <div className="ml-auto flex gap-2">
            <Button onClick={onClose}>{t("common.close")}</Button>
            <Button variant="primary" onClick={() => void save()} disabled={busy || (!provider && remote && info.requires_key && !key.trim())}>
              {busy && <Spinner size={13} />} {provider ? t("settings.providers.saveTest") : t("settings.providers.connect")}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
