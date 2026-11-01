import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { api } from "../../lib/api";
import type { BrainInfo } from "../../lib/types";
import { Button, Input, Toggle } from "../ui";
import { FamilyLogo } from "../settings/WorkStudioSection";

/** A folder name the way brain's folders are named (matches the app's `brain::slug`). */
export function brainSlug(text: string): string {
  const s = text
    .toLowerCase()
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60)
    .replace(/-+$/, "");
  return s || "note";
}

/**
 * An agent's or a group's folder in brain: on or off, and where. Shown only while brain keeps the
 * agents' folders (Settings → Apps → WorkStudio).
 */
export function BrainFolder({
  on,
  folder,
  fallback,
  onChange,
}: {
  on: boolean;
  folder: string;
  /** Where it goes when the user leaves the folder empty. */
  fallback: (root: string) => string;
  onChange: (on: boolean, folder: string) => void | Promise<void>;
}) {
  const { t } = useTranslation();
  const [info, setInfo] = useState<BrainInfo | null>(null);
  const [typed, setTyped] = useState(folder);
  useEffect(() => void api.brainInfo().then(setInfo).catch(() => {}), []);
  useEffect(() => setTyped(folder), [folder]);
  if (!info?.enabled) return null;

  return (
    <div className="mt-3 rounded-xl border border-line p-3">
      <div className="flex items-center gap-3">
        <FamilyLogo id="brain" size={28} />
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-medium">{t("profile.brain.title")}</p>
          <p className="text-[11.5px] text-muted">{t("profile.brain.hint")}</p>
        </div>
        <Toggle checked={on} onChange={(v) => void onChange(v, folder)} label={t("profile.brain.title")} />
      </div>
      {on && (
        <div className="mt-2.5 flex gap-2">
          <Input value={typed} onChange={(e) => setTyped(e.target.value)} placeholder={fallback(info.root)} className="h-8 font-mono text-[12px]" />
          <Button size="sm" disabled={typed.trim() === folder} onClick={() => void onChange(on, typed.trim())}>
            {t("common.save")}
          </Button>
        </div>
      )}
    </div>
  );
}
