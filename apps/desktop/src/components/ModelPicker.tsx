import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { api } from "../lib/api";
import { isConfigured, useModels } from "../lib/models";
import { useStore } from "../lib/store";
import type { ProviderInfo } from "../lib/types";
import { IconButton, Input, Select } from "./ui";

/** Provider + model chooser used when creating an agent, in its profile and in the walkthrough.
 *  Hidden models (Settings → Models) are left out; the current one always stays selectable. */
export function ModelPicker({
  providerId,
  model,
  onChange,
  defaultLabel = true,
}: {
  providerId: string | null;
  model: string | null;
  onChange: (providerId: string | null, model: string | null) => void;
  /** Offer "Provider default" as the first model. */
  defaultLabel?: boolean;
}) {
  const { t } = useTranslation();
  const providers = useStore((s) => s.providers);
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);
  useEffect(() => void api.providerCatalog().then(setCatalog), []);

  const usable = providers.filter((p) => isConfigured(p, catalog.find((c) => c.kind === p.kind)));
  const provider = providers.find((p) => p.id === providerId);
  const { visible, loading, error, reload } = useModels(providerId);

  const current = model ?? "";
  const options = [
    ...(defaultLabel ? [{ value: "", label: `${t("profile.modelDefault")}${provider?.default_model ? ` (${provider.default_model})` : ""}` }] : []),
    ...visible.map((m) => ({ value: m, label: m })),
  ];
  if (current && !visible.includes(current)) options.push({ value: current, label: current });

  const providerOptions = [...usable];
  if (provider && !providerOptions.some((p) => p.id === provider.id)) providerOptions.push(provider);

  return (
    <div className="space-y-2">
      <Select
        value={providerId ?? ""}
        onChange={(v) => onChange(v || null, null)}
        placeholder={t("models.noProvider")}
        options={providerOptions.map((p) => ({ value: p.id, label: p.name }))}
        disabled={providerOptions.length === 0}
      />
      <div className="flex items-center gap-1.5">
        {visible.length > 0 || current ? (
          <Select className="flex-1" value={current} onChange={(v) => onChange(providerId, v || null)} options={options} placeholder={t("models.pick")} disabled={!providerId} />
        ) : (
          <Input
            key={providerId ?? "none"}
            defaultValue={current}
            disabled={!providerId}
            placeholder={provider?.default_model ?? t("profile.customModel")}
            onBlur={(e) => e.target.value !== current && onChange(providerId, e.target.value.trim() || null)}
          />
        )}
        <IconButton label={t("profile.refreshModels")} onClick={() => void reload()} disabled={!providerId} className="shrink-0 border border-line">
          <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
        </IconButton>
      </div>
      {error && <p className="line-clamp-3 text-[11.5px] text-danger">{error}</p>}
      {providerOptions.length === 0 && <p className="text-[12px] text-muted">{t("models.noProviderHint")}</p>}
    </div>
  );
}
