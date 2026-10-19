import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api, openUrl } from "../../lib/api";
import { useStore } from "../../lib/store";
import type { CatalogStatus } from "../../lib/types";
import { Button, Field, Input } from "../ui";
import { Card } from "./SettingsModal";

/** The catalog apps and their state; reloads when a sign-in finishes. */
export function useCatalog() {
  const [apps, setApps] = useState<CatalogStatus[]>([]);
  const load = useCallback(() => void api.catalogStatus().then(setApps), []);
  useEffect(() => {
    load();
    window.addEventListener("opencue:catalog-changed", load);
    window.addEventListener("opencue:mcp-changed", load);
    return () => {
      window.removeEventListener("opencue:catalog-changed", load);
      window.removeEventListener("opencue:mcp-changed", load);
    };
  }, [load]);
  return { apps, load };
}

export function CatalogCard() {
  const { t } = useTranslation();
  const showToast = useStore((s) => s.showToast);
  const { apps, load } = useCatalog();
  const [busy, setBusy] = useState<string | null>(null);
  // Providers that need an OAuth app registered by the user ask for its client ID.
  const [needsClient, setNeedsClient] = useState<string | null>(null);
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");

  const connect = async (slug: string, id?: string, secret?: string) => {
    setBusy(slug);
    try {
      const url = await api.catalogConnect(slug, id, secret);
      setNeedsClient(null);
      await openUrl(url);
      showToast(t("settings.catalog.opened"));
    } catch (e) {
      const msg = String(e);
      if (/client ID/i.test(msg)) setNeedsClient(slug);
      showToast(msg);
    } finally {
      setBusy(null);
      load();
    }
  };

  const label = (state: CatalogStatus["state"]) => t(`settings.catalog.state.${state}`);

  return (
    <Card title={t("settings.catalog.title")}>
      <div className="space-y-3 px-4 py-3.5">
        <p className="text-[12.5px] text-muted">{t("settings.catalog.hint")}</p>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {apps.map((a) => (
            <div key={a.slug} role="button" tabIndex={0} aria-label={`${a.name}: ${label(a.state)}`} data-state={a.state} onClick={() => busy === null && a.state !== "connected" && void connect(a.slug)} onKeyDown={(e) => e.key === "Enter" && a.state !== "connected" && void connect(a.slug)} className="app-card flex min-h-[92px] cursor-pointer flex-col gap-1 px-3.5 py-3 text-left hover:brightness-105">
              <span className="flex items-center justify-between gap-2">
                <span className="text-[14px] font-semibold">{a.name}</span>
                <span className={`text-[11px] font-medium ${a.state === "connected" ? "text-success" : a.state === "reconnect" ? "text-warning" : "text-danger"}`}>{busy === a.slug ? t("settings.catalog.waiting") : label(a.state)}</span>
              </span>
              <span className="text-[12px] leading-snug text-muted">{a.description}</span>
              {a.state !== "disconnected" && (
                <span className="mt-auto flex justify-end gap-3 pt-1 text-[11.5px]">
                  {a.state === "reconnect" && a.error && <span className="mr-auto line-clamp-1 text-faint">{a.error}</span>}
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      void api.catalogDisconnect(a.slug).then(load);
                    }}
                    className="text-muted hover:text-danger"
                  >
                    {t("settings.catalog.disconnect")}
                  </button>
                </span>
              )}
            </div>
          ))}
        </div>
        {needsClient && (
          <div className="space-y-2 rounded-xl border border-line p-3">
            <p className="text-[12.5px] text-muted">{t("settings.catalog.clientHint", { name: apps.find((a) => a.slug === needsClient)?.name ?? "" })}</p>
            <div className="grid grid-cols-2 gap-2">
              <Field label={t("settings.catalog.clientId")}>
                <Input value={clientId} onChange={(e) => setClientId(e.target.value)} spellCheck={false} />
              </Field>
              <Field label={t("settings.catalog.clientSecret")}>
                <Input type="password" value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} />
              </Field>
            </div>
            <div className="flex gap-2">
              <Button size="sm" variant="primary" disabled={!clientId.trim() || busy !== null} onClick={() => void connect(needsClient, clientId, clientSecret)}>
                {t("settings.catalog.connect")}
              </Button>
              <Button size="sm" onClick={() => setNeedsClient(null)}>
                {t("common.cancel")}
              </Button>
            </div>
          </div>
        )}
      </div>
    </Card>
  );
}
