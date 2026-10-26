import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, RefreshCw, Search, Star } from "lucide-react";
import { parseHidden, useStore } from "../../lib/store";
import { api } from "../../lib/api";
import { isConfigured } from "../../lib/models";
import type { ProviderConfig, ProviderInfo } from "../../lib/types";
import { cx } from "../../lib/format";
import { Badge, Button, IconButton, Input, Spinner, Toggle } from "../ui";
import { PageTitle } from "./SettingsModal";
import { LocalModels } from "./LocalModels";

/** Models of the providers that are set up: search them, and choose which ones show in pickers. */
export function ModelsSection() {
  const { t } = useTranslation();
  const providers = useStore((s) => s.providers);
  const loadModels = useStore((s) => s.loadModels);
  const openSettings = useStore((s) => s.openSettings);
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<"cloud" | "local">("cloud");

  useEffect(() => void api.providerCatalog().then(setCatalog), []);
  const ready = providers.filter((p) => isConfigured(p, catalog.find((c) => c.kind === p.kind)));
  const readyKey = ready.map((p) => p.id).join(",");
  useEffect(() => {
    ready.forEach((p) => void loadModels(p.id));
  }, [readyKey]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <>
      <PageTitle title={t("settings.models.title")} hint={t("settings.models.hint")} />
      <div className="mb-5 inline-flex rounded-lg bg-hover p-0.5" role="tablist">
        {(["cloud", "local"] as const).map((k) => (
          <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)} className={cx("rounded-md px-3.5 py-1.5 text-[12.5px] font-medium", tab === k ? "bg-bg text-ink shadow-sm" : "text-muted hover:text-ink")}>
            {t(`settings.models.tab.${k}`)}
          </button>
        ))}
      </div>
      {tab === "local" ? (
        <LocalModels />
      ) : catalog.length > 0 && ready.length === 0 ? (
        <div className="rounded-xl border border-dashed border-line px-5 py-8 text-center">
          <p className="text-[13px] text-muted">{t("settings.models.noneReady")}</p>
          <Button className="mt-3" variant="primary" onClick={() => openSettings("providers")}>
            {t("settings.models.goProviders")}
          </Button>
        </div>
      ) : (
        <>
          <div className="relative mb-4">
            <Search size={15} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-faint" />
            <Input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("settings.models.search")} className="pl-9" />
          </div>
          <div className="space-y-3">
            {ready.map((p) => (
              <ProviderModels key={p.id} provider={p} query={query.trim().toLowerCase()} />
            ))}
          </div>
        </>
      )}
    </>
  );
}

function ProviderModels({ provider, query }: { provider: ProviderConfig; query: string }) {
  const { t } = useTranslation();
  const entry = useStore((s) => s.models[provider.id]);
  const hiddenRaw = useStore((s) => s.settings.hidden_models);
  const loadModels = useStore((s) => s.loadModels);
  const setModelHidden = useStore((s) => s.setModelHidden);
  const setSetting = useStore((s) => s.setSetting);
  const refreshProviders = useStore((s) => s.refreshProviders);
  const [open, setOpen] = useState(true);

  const all = entry?.list ?? [];
  const hidden = useMemo(() => new Set(parseHidden(hiddenRaw)[provider.id] ?? []), [hiddenRaw, provider.id]);
  const providerMatches = !!query && provider.name.toLowerCase().includes(query);
  const rows = query && !providerMatches ? all.filter((m) => m.toLowerCase().includes(query)) : all;
  const shown = all.length - all.filter((m) => hidden.has(m)).length;

  if (query && rows.length === 0 && !providerMatches) return null;

  const setAll = (hide: boolean) => {
    const map = parseHidden(hiddenRaw);
    map[provider.id] = hide ? all : [];
    void setSetting("hidden_models", JSON.stringify(map));
  };
  const makeDefault = async (m: string) => {
    await api.saveProvider({ ...provider, default_model: m });
    await refreshProviders();
  };

  return (
    <div className="rounded-xl border border-line">
      <div className="flex items-center gap-2 px-4 py-2.5">
        <button onClick={() => setOpen((v) => !v)} className="flex min-w-0 flex-1 items-center gap-2 text-left">
          <ChevronDown size={15} className={cx("shrink-0 text-faint transition-transform", !open && "-rotate-90")} />
          <span className="truncate text-[13.5px] font-medium">{provider.name}</span>
          {all.length > 0 && <Badge>{t("settings.models.visible", { shown, total: all.length })}</Badge>}
        </button>
        {all.length > 0 && (
          <>
            <Button size="sm" variant="ghost" onClick={() => setAll(false)} disabled={hidden.size === 0}>
              {t("settings.models.showAll")}
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setAll(true)} disabled={shown === 0}>
              {t("settings.models.hideAll")}
            </Button>
          </>
        )}
        <IconButton label={t("profile.refreshModels")} onClick={() => void loadModels(provider.id, true)} className="size-7">
          <RefreshCw size={13} className={entry?.loading ? "animate-spin" : ""} />
        </IconButton>
      </div>
      {open && (
        <div className="border-t border-line">
          {entry?.loading && all.length === 0 && (
            <div className="flex items-center gap-2 px-4 py-3 text-[12.5px] text-muted">
              <Spinner size={13} /> {t("common.loading")}
            </div>
          )}
          {entry?.error && <p className="selectable line-clamp-3 px-4 py-3 text-[12px] text-danger">{entry.error}</p>}
          {!entry?.loading && !entry?.error && all.length === 0 && <p className="px-4 py-3 text-[12.5px] text-faint">{t("settings.models.noModels")}</p>}
          <ul className="max-h-72 divide-y divide-line overflow-y-auto">
            {rows.map((m) => {
              const off = hidden.has(m);
              return (
                <li key={m} className="flex items-center gap-2 px-4 py-1.5">
                  <span className={cx("selectable min-w-0 flex-1 truncate font-mono text-[12.5px]", off && "text-faint line-through")}>{m}</span>
                  {provider.default_model === m ? (
                    <Badge tone="accent">{t("settings.models.isDefault")}</Badge>
                  ) : (
                    <IconButton label={t("settings.models.makeDefault")} onClick={() => void makeDefault(m)} className="size-6">
                      <Star size={13} />
                    </IconButton>
                  )}
                  <Toggle checked={!off} onChange={(v) => void setModelHidden(provider.id, m, !v)} label={m} />
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </div>
  );
}
