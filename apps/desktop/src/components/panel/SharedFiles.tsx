import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { SharedFile } from "../../lib/types";
import { api } from "../../lib/api";
import { formatBytes, shortTime } from "../../lib/format";
import { useStore } from "../../lib/store";
import { FileActions, FileIcon } from "../chat/Attachments";

/** Files shared in a chat (by the user or by agents), newest first. */
export function SharedFiles({ threadId }: { threadId: string }) {
  const { t } = useTranslation();
  const agents = useStore((s) => s.agents);
  const count = useStore((s) => s.messages[threadId]?.length ?? 0);
  const [files, setFiles] = useState<SharedFile[] | null>(null);

  useEffect(() => {
    void api.threadFiles(threadId).then(setFiles).catch(() => setFiles([]));
  }, [threadId, count]);

  if (!files) return null;
  if (!files.length) {
    return <div className="rounded-xl border border-dashed border-line-strong px-4 py-5 text-center text-[12.5px] text-muted">{t("files.empty")}</div>;
  }
  return (
    <div className="-mx-2 space-y-0.5">
      {files.map((f) => (
        <div
          key={f.attachment.id}
          role="button"
          tabIndex={0}
          onClick={() => void api.openAttachment(f.attachment.id).catch((e) => useStore.getState().showToast(String(e)))}
          className="group/file flex cursor-pointer items-center gap-2.5 rounded-lg px-2 py-1.5 hover:bg-hover"
        >
          <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
            <FileIcon a={f.attachment} size={16} />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[13px]">{f.attachment.name}</span>
            <span className="block truncate text-[11.5px] text-muted">
              {formatBytes(f.attachment.size)} · {f.agent_id ? agents[f.agent_id]?.name ?? "" : t("files.you")} · {shortTime(f.created_at)}
            </span>
          </span>
          <FileActions a={f.attachment} />
        </div>
      ))}
    </div>
  );
}
