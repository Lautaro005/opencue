import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Code2, Download, FileArchive, FileImage, FileSpreadsheet, FileText, FolderOpen, Globe, GitBranch, Presentation, Braces } from "lucide-react";
import type { ArtifactKind, Attachment } from "../../lib/types";
import { KIND_LABEL } from "../../lib/artifacts";
import { api, isTauri, saveAttachmentAs } from "../../lib/api";
import { cx, formatBytes } from "../../lib/format";
import { useStore } from "../../lib/store";

const previews = new Map<string, Promise<string>>();
const isImage = (a: Attachment) => /^image\/(png|jpeg|gif|webp)$/.test(a.mime);

function preview(id: string) {
  if (!previews.has(id)) previews.set(id, api.readAttachment(id).catch(() => ""));
  return previews.get(id)!;
}

export function FileIcon({ a, size = 18 }: { a: Attachment; size?: number }) {
  if (a.mime.startsWith("image/")) return <FileImage size={size} />;
  if (/csv|sheet|excel|tab-separated/.test(a.mime)) return <FileSpreadsheet size={size} />;
  if (/zip|tar|gzip/.test(a.mime)) return <FileArchive size={size} />;
  return <FileText size={size} />;
}

/** The icon of an artifact type. */
export function ArtifactIcon({ kind, size = 18 }: { kind: ArtifactKind; size?: number }) {
  switch (kind) {
    case "html":
      return <Globe size={size} />;
    case "svg":
      return <FileImage size={size} />;
    case "mermaid":
      return <GitBranch size={size} />;
    case "code":
      return <Code2 size={size} />;
    case "json":
      return <Braces size={size} />;
    case "xlsx":
    case "csv":
      return <FileSpreadsheet size={size} />;
    case "pptx":
      return <Presentation size={size} />;
    default:
      return <FileText size={size} />;
  }
}

const toastError = (e: unknown) => useStore.getState().showToast(String(e));

export function FileActions({ a }: { a: Attachment }) {
  const { t } = useTranslation();
  if (!isTauri) return null;
  return (
    <span className="flex shrink-0 items-center opacity-0 transition-opacity group-hover/file:opacity-100">
      <button
        data-tip={t("files.save")}
        aria-label={t("files.save")}
        onClick={(e) => {
          e.stopPropagation();
          void saveAttachmentAs(a.id, a.name).catch(toastError);
        }}
        className="rounded-md p-1 text-muted hover:bg-active hover:text-ink"
      >
        <Download size={14} />
      </button>
      <button
        data-tip={t("files.reveal")}
        aria-label={t("files.reveal")}
        onClick={(e) => {
          e.stopPropagation();
          void api.revealAttachment(a.id).catch(toastError);
        }}
        className="rounded-md p-1 text-muted hover:bg-active hover:text-ink"
      >
        <FolderOpen size={14} />
      </button>
    </span>
  );
}

function ImageThumb({ a }: { a: Attachment }) {
  const [src, setSrc] = useState("");
  useEffect(() => {
    let live = true;
    void preview(a.id).then((s) => live && setSrc(s));
    return () => {
      live = false;
    };
  }, [a.id]);
  return (
    <button
      onClick={() => void api.openAttachment(a.id).catch(toastError)}
      data-tip={a.name}
      className="group/file relative block overflow-hidden rounded-2xl border border-line bg-hover"
    >
      {src ? <img src={src} alt={a.name} className="block max-h-64 max-w-[280px] object-cover" /> : <div className="h-40 w-56 animate-pulse" />}
      <span className="absolute top-1.5 right-1.5 rounded-lg bg-elevated/90 shadow-sm">
        <FileActions a={a} />
      </span>
    </button>
  );
}

/** What an agent made with `create_artifact`: a card that opens it in the side panel. */
export function ArtifactCard({ a }: { a: Attachment }) {
  const { t } = useTranslation();
  const openArtifact = useStore((s) => s.openArtifact);
  const art = a.artifact!;
  const open = () => openArtifact(art.id, art.version, art.canvas);
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={open}
      onKeyDown={(e) => e.key === "Enter" && open()}
      data-busy="false"
      data-tip={t("artifact.open")}
      className="art-card group/file flex w-[300px] max-w-full cursor-pointer items-center gap-3 px-3 py-3 text-left transition-colors hover:bg-hover"
    >
      <span className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-accent-soft text-accent">
        <ArtifactIcon kind={art.kind} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13.5px] font-medium">{art.title}</span>
        <span className="block truncate text-[11.5px] text-muted">
          {art.canvas ? t("artifact.canvas") : KIND_LABEL[art.kind]} · {t("artifact.version", { n: art.version })} · {formatBytes(a.size)}
        </span>
      </span>
      <FileActions a={a} />
    </div>
  );
}

export function FileCard({ a }: { a: Attachment }) {
  if (a.artifact) return <ArtifactCard a={a} />;
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => void api.openAttachment(a.id).catch(toastError)}
      onKeyDown={(e) => e.key === "Enter" && void api.openAttachment(a.id).catch(toastError)}
      className="group/file flex w-[260px] cursor-pointer items-center gap-2.5 rounded-2xl border border-line bg-bg px-3 py-2.5 text-left hover:bg-hover"
    >
      <span className="flex size-9 shrink-0 items-center justify-center rounded-xl bg-accent-soft text-accent">
        <FileIcon a={a} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] font-medium">{a.name}</span>
        <span className="block text-[11.5px] text-muted">{formatBytes(a.size)}</span>
      </span>
      <FileActions a={a} />
    </div>
  );
}

export function AttachmentList({ items, align = "start" }: { items: Attachment[]; align?: "start" | "end" }) {
  if (!items.length) return null;
  return (
    <div className={cx("flex flex-wrap items-start gap-1.5", align === "end" ? "justify-end" : "justify-start")}>
      {items.map((a) => (isImage(a) ? <ImageThumb key={a.id} a={a} /> : <FileCard key={a.id} a={a} />))}
    </div>
  );
}
