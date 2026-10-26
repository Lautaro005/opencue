import { useTranslation } from "react-i18next";
import { formatBytes } from "../../lib/format";
import { Spinner } from "../ui";
import { ArtifactIcon } from "./Attachments";
import type { ArtifactKind } from "../../lib/types";

/**
 * What an agent is making, while it makes it: the same card the finished artifact gets, with a
 * moving border and a bar, the title as soon as the model has written it, and how much is done.
 */
export function ArtifactProgress({ title, chars, canvas, kind }: { title?: string | null; chars?: number; canvas?: boolean; kind?: ArtifactKind }) {
  const { t } = useTranslation();
  return (
    <div className="art-card animate-pop flex w-[300px] max-w-full flex-col gap-2.5 px-3 py-3" data-busy="true" role="status" aria-live="polite">
      <div className="flex items-center gap-3">
        <span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-accent-soft text-accent">{kind ? <ArtifactIcon kind={kind} /> : <Spinner size={18} />}</span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-[13.5px] font-medium">{title || (canvas ? t("artifact.creatingCanvas") : t("artifact.creating"))}</span>
          <span className="block truncate text-[11.5px] text-muted">{chars ? t("artifact.writing", { size: formatBytes(chars) }) : canvas ? t("artifact.creatingCanvas") : t("artifact.creating")}</span>
        </span>
      </div>
      <div className="bar-indeterminate" />
    </div>
  );
}
