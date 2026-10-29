import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Download, Plus, Trash2 } from "lucide-react";
import { ProviderLogo } from "../ProviderLogo";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import { confirmDialog } from "../../lib/confirm";
import { isConfigured } from "../../lib/models";
import type { CliVersion, ProviderConfig, ProviderInfo } from "../../lib/types";
import { cx } from "../../lib/format";
import { Badge, Button, DialogHeader, Field, Input, Modal, Spinner, Toggle } from "../ui";

type Group = "connected" | "cloud" | "local" | "cli";
type Tile = { info: ProviderInfo; provider: ProviderConfig | null; key: string; ready: boolean; group: Group };
const GROUP_ORDER: Group[] = ["connected", "cloud", "local", "cli"];
const LOCAL_KINDS = ["ollama", "lmstudio"];

function groupOf(info: ProviderInfo): Group {
  if (info.api === "cli") return "cli";
  return LOCAL_KINDS.includes(info.kind) ? "local" : "cloud";
}

/** Under a configured CLI: a newer version is out, with the button that installs it. */
function CliUpdateRow({ kind, versions, onDone }: { kind: string; versions: CliVersion[]; onDone: () => void }) {
  const { t } = useTranslation();
  const showToast = useStore((st) => st.showToast);
  const [busy, setBusy] = useState(false);
  const v = versions.find((x) => x.kind === kind);
  if (!v || !v.outdated) return null;
  return (
    <div className="flex items-center gap-2 border-t border-line px-3.5 py-2 text-[12px]">
      <span className="min-w-0 flex-1 truncate text-muted">
        {t("settings.providers.newVersion", { from: v.installed, to: v.latest })}
      </span>
      <Button
        size="sm"
        variant="primary"
        disabled={busy}
        onClick={() => {
          setBusy(true);
          void api
            .cliUpdate(kind)
            .then(() => showToast(t("settings.providers.updated")))
            .catch((e) => showToast(String(e)))
            .finally(() => {
              setBusy(false);
              onDone();
            });
        }}
      >
        {busy ? <Spinner size={12} /> : <Download size={12} />} {t("settings.providers.update")}
      </Button>
    </div>
  );
}

/** One container per provider the app supports; click one to set it up (Settings → Models → Providers). */
export function ProvidersSection() {
  const { t } = useTranslation();
  const providers = useStore((s) => s.providers);
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);
  const [editing, setEditing] = useState<{ provider: ProviderConfig | null; info: ProviderInfo } | null>(null);

  useEffect(() => void api.providerCatalog().then(setCatalog), []);
  const [versions, setVersions] = useState<CliVersion[]>([]);
  const loadVersions = useCallback(() => void api.cliVersions().then(setVersions).catch(() => {}), []);
  useEffect(loadVersions, [loadVersions]);

  // Ordered so the page reads top to bottom: what is connected first, then the rest by kind
  // (cloud APIs, models on this computer, command-line tools), each A–Z.
  const tiles: Tile[] = [];
  for (const info of catalog) {
    const mine = providers.filter((p) => p.kind === info.kind);
    for (const p of mine) tiles.push({ info, provider: p, key: p.id, ready: isConfigured(p, info), group: "connected" });
    if (mine.length === 0 || info.kind === "openai_compatible") tiles.push({ info, provider: null, key: `new-${info.kind}`, ready: false, group: groupOf(info) });
  }
  for (const tile of tiles) if (tile.provider && !tile.ready) tile.group = groupOf(tile.info);
  const label = (tile: Tile) => (tile.provider && tile.provider.kind === "openai_compatible" ? tile.provider.name : tile.info.label);
  const groups = GROUP_ORDER.map((g) => ({ g, items: tiles.filter((x) => x.group === g).sort((a, b) => label(a).localeCompare(label(b))) })).filter((x) => x.items.length > 0);

  return (
    <>
      <div className="space-y-5">
        {groups.map(({ g, items }) => (
          <section key={g} aria-label={t(`settings.providers.group.${g}`)}>
            <h3 className="mb-2 flex items-center gap-2 text-[11.5px] font-semibold tracking-wide text-faint uppercase">
              {t(`settings.providers.group.${g}`)}
              <span className="rounded-full bg-hover px-1.5 py-px text-[10.5px] font-medium tracking-normal text-muted">{items.length}</span>
            </h3>
            <div className="grid grid-cols-2 gap-2.5">
              {items.map(({ info, provider, key, ready }) => (
                <div key={key} className={cx("surface surface-hover flex flex-col rounded-2xl", ready && "ring-1 ring-success/25")}>
                <button
                  onClick={() => setEditing({ provider, info })}
                  className="flex min-h-[92px] flex-1 flex-col items-start gap-2 rounded-2xl p-3.5 text-left"
                >
                  <span className="flex w-full items-center gap-2.5">
                    <span className={cx("flex size-9 shrink-0 items-center justify-center rounded-xl border border-line", ready ? "bg-accent-soft" : "bg-sidebar")}>
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
                {ready && info.api === "cli" && <CliUpdateRow kind={info.kind} versions={versions} onDone={loadVersions} />}
                </div>
              ))}
            </div>
          </section>
        ))}
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
