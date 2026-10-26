import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronLeft, ChevronRight, Code2, Copy, Download, Eye, Pencil } from "lucide-react";
import { api, isTauri, saveAttachmentAs } from "../../lib/api";
import { useStore } from "../../lib/store";
import { cx } from "../../lib/format";
import { KIND_LABEL, RENDERED_FROM_SOURCE, inlinePieces, parseCsv, parseSheets, parseSlides } from "../../lib/artifacts";
import type { Artifact, ArtifactVersion } from "../../lib/types";
import { IconButton, Select, Spinner } from "../ui";
import { Markdown } from "../chat/Markdown";
import { ArtifactIcon } from "../chat/Attachments";
import { PanelShell } from "./PanelShell";

/** Where sandboxed frames (HTML artifacts, the dashboard) load from; empty outside the desktop app. */
let viewBase: Promise<string> | null = null;
export function frameBase(): Promise<string> {
  viewBase ??= isTauri ? api.viewBase().catch(() => "") : Promise.resolve("");
  return viewBase;
}

/** The side panel that shows an artifact the way it will look, with its versions and its source.
 * A canvas can also be edited by hand here: every pause saves a new version the agent reads back. */
export function ArtifactPanel() {
  const { t } = useTranslation();
  const view = useStore((s) => s.artifactView);
  const showToast = useStore((s) => s.showToast);
  const [artifact, setArtifact] = useState<Artifact | null>(null);
  const [error, setError] = useState("");
  const [version, setVersion] = useState(0);
  const [mode, setMode] = useState<"preview" | "code" | "edit">("preview");
  const [copied, setCopied] = useState(false);
  const [draft, setDraft] = useState("");
  const [dirty, setDirty] = useState(false);
  const [saved, setSaved] = useState<number | null>(null);
  const [saving, setSaving] = useState(false);
  const dirtyRef = useRef(false);
  dirtyRef.current = dirty;

  const load = (id: string, wanted?: number, fresh = false) =>
    api
      .getArtifact(id)
      .then((a) => {
        setArtifact(a);
        const latest = a.versions[a.versions.length - 1];
        setVersion(wanted ?? latest?.version ?? 1);
        if (!dirtyRef.current || fresh) {
          setDraft(latest?.source ?? "");
          setDirty(false);
        }
        if (fresh) setMode(a.canvas ? (a.kind === "code" || a.kind === "text" || a.kind === "markdown" ? "edit" : "preview") : "preview");
      })
      .catch((e) => setError(String(e)));

  useEffect(() => {
    if (!view) return;
    setError("");
    setSaved(null);
    void load(view.id, view.version, true);
  }, [view?.id, view?.version]); // eslint-disable-line react-hooks/exhaustive-deps

  // The agent wrote a new version of what is open: show it.
  useEffect(() => {
    const onChanged = (e: Event) => {
      const d = (e as CustomEvent<{ id: string }>).detail;
      if (view && d.id === view.id) void load(view.id);
    };
    window.addEventListener("opencue:artifact-changed", onChanged);
    return () => window.removeEventListener("opencue:artifact-changed", onChanged);
  }, [view?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  // Typing in a canvas saves a new version after a short pause.
  useEffect(() => {
    if (!dirty || !artifact?.canvas) return;
    const timer = window.setTimeout(() => {
      setSaving(true);
      api
        .saveCanvas(artifact.id, draft)
        .then((a) => {
          setArtifact(a);
          const latest = a.versions[a.versions.length - 1];
          setVersion(latest.version);
          setSaved(latest.version);
          setDirty(false);
        })
        .catch((e) => showToast(String(e)))
        .finally(() => setSaving(false));
    }, 900);
    return () => window.clearTimeout(timer);
  }, [draft, dirty, artifact?.id, artifact?.canvas]); // eslint-disable-line react-hooks/exhaustive-deps

  const latestVersion = artifact?.versions[artifact.versions.length - 1];
  const current: ArtifactVersion | undefined = artifact?.versions.find((v) => v.version === version) ?? latestVersion;
  const isLatest = current?.version === latestVersion?.version;
  const textOnly = artifact?.kind === "code" || artifact?.kind === "text";
  const editing = !!artifact?.canvas && mode === "edit" && isLatest;
  const tabs = (artifact?.canvas ? ["preview", "edit"] : ["preview", "code"]) as ("preview" | "code" | "edit")[];

  const copy = async () => {
    if (!current) return;
    try {
      await navigator.clipboard.writeText(editing ? draft : current.source);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch (e) {
      showToast(String(e));
    }
  };

  const actions = (
    <>
      <IconButton label={copied ? t("chat.copied") : t("artifact.copy")} onClick={() => void copy()} className="size-8">
        {copied ? <Check size={15} className="text-success" /> : <Copy size={15} />}
      </IconButton>
      {isTauri && current && (
        <IconButton label={t("files.save")} onClick={() => void saveAttachmentAs(current.attachment.id, current.attachment.name).catch((e) => showToast(String(e)))} className="size-8">
          <Download size={15} />
        </IconButton>
      )}
    </>
  );

  return (
    <PanelShell
      id={view?.id ?? ""}
      kind={artifact?.canvas ? "canvas" : "artifact"}
      title={artifact?.title ?? t("artifact.title")}
      subtitle={artifact ? `${artifact.canvas ? t("artifact.canvas") : KIND_LABEL[artifact.kind]}${artifact.language ? ` · ${artifact.language}` : ""}` : undefined}
      icon={
        artifact && (
          <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
            <ArtifactIcon kind={artifact.kind} size={15} />
          </span>
        )
      }
      actions={actions}
      widthKey="opencue:panel-artifact"
      initial={520}
    >
      {error ? (
        <p className="px-6 py-8 text-center text-[13px] text-muted">{error}</p>
      ) : !artifact || !current ? (
        <div className="flex flex-1 items-center justify-center">
          <Spinner />
        </div>
      ) : (
        <>
          <div className="flex shrink-0 items-center gap-1.5 border-b border-line px-4 pb-2">
            {(!textOnly || artifact.canvas) && (
              <div className="flex rounded-lg bg-hover p-0.5">
                {tabs.map((m) => (
                  <button key={m} onClick={() => setMode(m)} className={cx("inline-flex items-center gap-1 rounded-md px-2.5 py-1 text-[12px] font-medium", mode === m ? "bg-panel text-ink shadow-sm" : "text-muted hover:text-ink")}>
                    {m === "preview" ? <Eye size={12} /> : m === "edit" ? <Pencil size={12} /> : <Code2 size={12} />}
                    {t(`artifact.${m}`)}
                  </button>
                ))}
              </div>
            )}
            {artifact.versions.length > 1 && (
              <Select
                className="w-[112px]"
                value={String(current.version)}
                onChange={(v) => setVersion(Number(v))}
                options={artifact.versions
                  .slice()
                  .reverse()
                  .map((v) => ({ value: String(v.version), label: t("artifact.version", { n: v.version }) + (v.by === "user" ? " ✎" : "") }))}
              />
            )}
          </div>
          <div className="min-h-0 flex-1 overflow-auto">
            {editing ? (
              <textarea
                value={draft}
                onChange={(e) => {
                  setDraft(e.target.value);
                  setDirty(true);
                }}
                spellCheck={artifact.kind === "markdown" || artifact.kind === "text"}
                aria-label={artifact.title}
                className="block size-full min-h-[360px] resize-none bg-transparent p-4 font-mono text-[12.5px] leading-[1.6] outline-none"
              />
            ) : mode === "code" || (textOnly && !artifact.canvas) || (textOnly && mode !== "preview") ? (
              <Source text={current.source} />
            ) : (
              <Preview artifact={artifact} version={current} />
            )}
          </div>
          <p className="shrink-0 truncate border-t border-line px-4 py-1.5 text-[11.5px] text-muted">
            {editing && (saving ? t("artifact.saving") : dirty ? "…" : saved ? t("artifact.saveCanvas", { n: saved }) : current.by === "user" ? t("artifact.youEdited") : current.attachment.name)}
            {!editing && current.attachment.name}
            {!editing && RENDERED_FROM_SOURCE.includes(artifact.kind) ? ` · ${t("artifact.previewNote")}` : ""}
          </p>
        </>
      )}
    </PanelShell>
  );
}

function Source({ text }: { text: string }) {
  return <pre className="selectable min-h-full p-4 font-mono text-[12px] leading-[1.6] break-words whitespace-pre-wrap">{text}</pre>;
}

function Preview({ artifact, version }: { artifact: Artifact; version: ArtifactVersion }) {
  const src = version.source;
  switch (artifact.kind) {
    case "html":
      return <HtmlFrame artifactId={artifact.id} version={version.version} source={src} title={artifact.title} />;
    case "svg":
      return (
        <div className="flex min-h-full items-center justify-center bg-white p-4">
          <img src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(src)}`} alt={artifact.title} className="max-h-full max-w-full" />
        </div>
      );
    case "markdown":
      return (
        <div className="selectable p-5 text-[14px] leading-relaxed">
          <Markdown text={src} />
        </div>
      );
    case "docx":
    case "pdf":
      return (
        <div className="min-h-full bg-hover p-4">
          <div className="selectable mx-auto max-w-[680px] rounded-sm bg-white px-8 py-9 text-[13.5px] leading-relaxed text-[#1c1d21] shadow-md [&_a]:text-blue-700 [&_h1]:mb-2 [&_h1]:text-[22px] [&_h2]:mb-1.5 [&_h2]:text-[17px] [&_table]:text-[12.5px]">
            <Markdown text={src} />
          </div>
        </div>
      );
    case "mermaid":
      return <MermaidView source={src} />;
    case "json":
    case "csv":
      return artifact.kind === "csv" ? <Table rows={parseCsv(src)} header /> : <Source text={prettyJson(src)} />;
    case "xlsx":
      return <Sheets source={src} />;
    case "pptx":
      return <Slides source={src} />;
    default:
      return <Source text={src} />;
  }
}

function prettyJson(src: string) {
  try {
    return JSON.stringify(JSON.parse(src), null, 2);
  } catch {
    return src;
  }
}

/** An HTML artifact in a sandboxed frame: it can run scripts but cannot touch the app. */
function HtmlFrame({ artifactId, version, source, title }: { artifactId: string; version: number; source: string; title: string }) {
  const [base, setBase] = useState<string | null>(null);
  useEffect(() => void frameBase().then(setBase), []);
  if (base === null) return null;
  return (
    <iframe
      key={`${artifactId}-${version}`}
      title={title}
      sandbox="allow-scripts allow-modals allow-forms"
      {...(base ? { src: `${base}/artifact/${artifactId}/${version}` } : { srcDoc: source })}
      className="size-full min-h-[480px] border-0 bg-white"
    />
  );
}

let mermaidReady: Promise<typeof import("mermaid").default> | null = null;

function MermaidView({ source }: { source: string }) {
  const { t } = useTranslation();
  const [svg, setSvg] = useState("");
  const [error, setError] = useState("");
  const id = useRef(`mmd-${Math.random().toString(36).slice(2)}`);
  useEffect(() => {
    let live = true;
    setError("");
    mermaidReady ??= import("mermaid").then((m) => {
      const dark = document.documentElement.classList.contains("dark");
      m.default.initialize({ startOnLoad: false, securityLevel: "strict", theme: dark ? "dark" : "default" });
      return m.default;
    });
    void mermaidReady
      .then((m) => m.render(`${id.current}-${Date.now()}`, source))
      .then((r) => live && setSvg(r.svg))
      .catch((e) => live && setError(String(e?.message ?? e)));
    return () => {
      live = false;
    };
  }, [source]);
  if (error) {
    return (
      <div className="p-4">
        <p className="mb-2 text-[12.5px] text-danger">{t("artifact.diagramError")}</p>
        <Source text={source} />
      </div>
    );
  }
  if (!svg) {
    return (
      <div className="flex h-40 items-center justify-center">
        <Spinner />
      </div>
    );
  }
  return <div className="flex min-h-full items-center justify-center bg-white p-4 [&_svg]:h-auto [&_svg]:max-w-full" dangerouslySetInnerHTML={{ __html: svg }} />;
}

function Table({ rows, header }: { rows: (string | number | boolean | null)[][]; header: boolean }) {
  const cols = Math.max(1, ...rows.map((r) => r.length));
  return (
    <table className="selectable w-full border-collapse text-[12.5px]">
      <tbody>
        {rows.map((r, i) => (
          <tr key={i} className={cx(header && i === 0 && "sticky top-0 bg-hover font-semibold")}>
            <td className="w-8 border border-line bg-hover px-1.5 py-1 text-center text-[11px] text-faint">{i + 1}</td>
            {Array.from({ length: cols }, (_, c) => (
              <td key={c} className={cx("max-w-[260px] border border-line px-2 py-1 align-top break-words", typeof r[c] === "number" && "text-right tabular-nums")}>
                {r[c] === null || r[c] === undefined ? "" : String(r[c])}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Sheets({ source }: { source: string }) {
  const sheets = useMemo(() => parseSheets(source), [source]);
  const [i, setI] = useState(0);
  const sheet = sheets[Math.min(i, sheets.length - 1)];
  if (!sheet) return <Source text={source} />;
  return (
    <div className="flex min-h-full flex-col">
      <div className="min-h-0 flex-1 overflow-auto">
        <Table rows={sheet.rows} header={sheet.header} />
      </div>
      <div className="sticky bottom-0 flex gap-1 border-t border-line bg-panel px-3 py-1.5">
        {sheets.map((s, n) => (
          <button key={n} onClick={() => setI(n)} className={cx("rounded-md px-2.5 py-1 text-[12px]", n === i ? "bg-accent-soft font-medium text-accent" : "text-muted hover:bg-hover")}>
            {s.name}
          </button>
        ))}
      </div>
    </div>
  );
}

function Slides({ source }: { source: string }) {
  const { t } = useTranslation();
  const slides = useMemo(() => parseSlides(source), [source]);
  const [i, setI] = useState(0);
  const slide = slides[Math.min(i, slides.length - 1)];
  if (!slide) return <Source text={source} />;
  const onlyTitle = slide.lines.length === 0;
  return (
    <div className="flex min-h-full flex-col gap-3 bg-hover p-4">
      <div className="@container aspect-video w-full overflow-hidden rounded-lg bg-white shadow-md">
        <div className={cx("flex h-full flex-col px-[6%] py-[5%] text-[#1f2328]", onlyTitle && "items-center justify-center text-center")}>
          <h3 className={cx("font-bold", onlyTitle ? "text-[7cqw]" : "mb-[3%] text-[4.6cqw] leading-tight")}>{slide.title}</h3>
          <ul className="space-y-[1.4cqw] text-[2.7cqw] leading-snug">
            {slide.lines.map((l, n) => (
              <li key={n} className="flex gap-[1.4cqw]">
                <span aria-hidden>•</span>
                <span>
                  {inlinePieces(l).map((p, k) => (
                    <span key={k} className={cx(p.bold && "font-bold", p.italic && "italic", p.code && "rounded bg-black/5 px-1 font-mono")}>
                      {p.text}
                    </span>
                  ))}
                </span>
              </li>
            ))}
          </ul>
        </div>
      </div>
      <div className="flex items-center justify-center gap-3 text-[12.5px] text-muted">
        <IconButton label={t("artifact.prevSlide")} onClick={() => setI((n) => Math.max(0, n - 1))} disabled={i === 0} className="size-7">
          <ChevronLeft size={15} />
        </IconButton>
        <span className="tabular-nums">{Math.min(i, slides.length - 1) + 1} / {slides.length}</span>
        <IconButton label={t("artifact.nextSlide")} onClick={() => setI((n) => Math.min(slides.length - 1, n + 1))} disabled={i >= slides.length - 1} className="size-7">
          <ChevronRight size={15} />
        </IconButton>
      </div>
      {slide.notes && (
        <p className="selectable rounded-lg border border-line bg-panel px-3 py-2 text-[12.5px] text-muted">
          <b className="text-ink">{t("artifact.notes")}:</b> {slide.notes}
        </p>
      )}
    </div>
  );
}
