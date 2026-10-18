import { useEffect, useMemo } from "react";
import { parseHidden, useStore } from "./store";
import type { ProviderConfig, ProviderInfo } from "./types";

/** Models a provider reports, minus the ones the user hid in Settings → Models. */
export function useModels(providerId: string | null | undefined) {
  const entry = useStore((s) => (providerId ? s.models[providerId] : undefined));
  const hiddenRaw = useStore((s) => s.settings.hidden_models);
  const loadModels = useStore((s) => s.loadModels);
  useEffect(() => {
    if (providerId) void loadModels(providerId);
  }, [providerId, loadModels]);
  const hidden = useMemo(() => new Set(parseHidden(hiddenRaw)[providerId ?? ""] ?? []), [hiddenRaw, providerId]);
  const all = entry?.list ?? [];
  const visible = useMemo(() => all.filter((m) => !hidden.has(m)), [all, hidden]);
  return {
    all,
    visible,
    hidden,
    loading: !!entry?.loading,
    error: entry?.error ?? null,
    reload: () => (providerId ? loadModels(providerId, true) : Promise.resolve()),
  };
}

/** A provider is ready to use when it is on and has what it needs (a key, unless it is local or a CLI). */
export function isConfigured(p: ProviderConfig, info?: ProviderInfo): boolean {
  if (!p.enabled) return false;
  if (!info) return p.has_key;
  return !info.requires_key || p.has_key;
}
