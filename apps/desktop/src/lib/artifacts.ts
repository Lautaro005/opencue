import type { ArtifactKind } from "./types";

/** Short names for the card and the panel. */
export const KIND_LABEL: Record<ArtifactKind, string> = {
  html: "Web page",
  svg: "Image (SVG)",
  markdown: "Document",
  mermaid: "Diagram",
  code: "Code",
  text: "Text",
  json: "JSON",
  csv: "Table (CSV)",
  xlsx: "Excel",
  docx: "Word",
  pptx: "PowerPoint",
  pdf: "PDF",
};

/** Kinds whose file is not the text the agent sent: the panel previews them from the source. */
export const RENDERED_FROM_SOURCE: ArtifactKind[] = ["xlsx", "docx", "pptx", "pdf"];

/** CSV with quoted fields, commas or tabs. */
export function parseCsv(src: string): string[][] {
  const delim = /^[^\n]*\t/.test(src) && !/^[^\n]*,/.test(src) ? "\t" : ",";
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  for (let i = 0; i < src.length; i++) {
    const c = src[i];
    if (c === '"' && quoted && src[i + 1] === '"') {
      cell += '"';
      i++;
    } else if (c === '"') quoted = !quoted;
    else if (c === delim && !quoted) {
      row.push(cell);
      cell = "";
    } else if (c === "\r" && !quoted) continue;
    else if (c === "\n" && !quoted) {
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else cell += c;
  }
  if (cell || row.length) {
    row.push(cell);
    rows.push(row);
  }
  return rows;
}

export interface Sheet {
  name: string;
  rows: (string | number | boolean | null)[][];
  header: boolean;
}

/** What the agent sent for a spreadsheet: `{"sheets":[…]}`, `{"rows":[…]}`, `[[…]]` or CSV. */
export function parseSheets(src: string): Sheet[] {
  const t = src.trim();
  if (t.startsWith("{") || t.startsWith("[")) {
    try {
      const v = JSON.parse(t);
      const one = (s: unknown, n: number): Sheet => {
        const o = (s ?? {}) as { rows?: unknown; name?: string; header?: boolean };
        const rows = (Array.isArray(s) ? s : Array.isArray(o.rows) ? o.rows : []) as unknown[];
        return {
          name: (o.name ?? "").trim() || `Sheet${n}`,
          rows: rows.map((r) => (Array.isArray(r) ? r : [r])) as Sheet["rows"],
          header: o.header !== false,
        };
      };
      return Array.isArray(v?.sheets) ? v.sheets.map((s: unknown, i: number) => one(s, i + 1)) : [one(v, 1)];
    } catch {
      return [];
    }
  }
  return [{ name: "Sheet1", rows: parseCsv(t), header: true }];
}

export interface Slide {
  title: string;
  lines: string[];
  notes: string;
}

/** Slides from Markdown (`---` between slides) or `{"slides":[…]}`. */
export function parseSlides(src: string): Slide[] {
  const t = src.trim();
  if (t.startsWith("{") || t.startsWith("[")) {
    try {
      const v = JSON.parse(t);
      const list = Array.isArray(v) ? v : v?.slides;
      if (Array.isArray(list) && list.length) {
        return list.map((s: { title?: string; bullets?: unknown[]; content?: unknown[]; notes?: string }) => ({
          title: s.title ?? "",
          lines: (s.bullets ?? s.content ?? []).map(String),
          notes: s.notes ?? "",
        }));
      }
    } catch {
      /* fall through to Markdown */
    }
  }
  const slides: Slide[] = [];
  let cur: Slide = { title: "", lines: [], notes: "" };
  let started = false;
  const push = () => {
    if (started) slides.push(cur);
    cur = { title: "", lines: [], notes: "" };
    started = false;
  };
  for (const raw of src.split("\n")) {
    const line = raw.trim();
    if (/^-{3,}$/.test(line)) {
      push();
      continue;
    }
    if (!line) continue;
    started = true;
    const h = line.match(/^#{1,6}\s+(.*)$/);
    const notes = line.match(/^(?:Speaker notes|Notes):\s*(.*)$/);
    if (h) {
      if (!cur.title) cur.title = h[1].trim();
      else cur.lines.push(h[1].trim());
    } else if (notes) cur.notes += notes[1];
    else cur.lines.push(line.replace(/^[-*]\s+/, ""));
  }
  push();
  return slides;
}

/** Inline Markdown (**bold**, *italic*, `code`) as plain pieces for a slide. */
export function inlinePieces(line: string): { text: string; bold?: boolean; italic?: boolean; code?: boolean }[] {
  const out: { text: string; bold?: boolean; italic?: boolean; code?: boolean }[] = [];
  const re = /(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`)/g;
  let last = 0;
  for (const m of line.matchAll(re)) {
    if (m.index! > last) out.push({ text: line.slice(last, m.index) });
    const s = m[0];
    if (s.startsWith("**")) out.push({ text: s.slice(2, -2), bold: true });
    else if (s.startsWith("`")) out.push({ text: s.slice(1, -1), code: true });
    else out.push({ text: s.slice(1, -1), italic: true });
    last = m.index! + s.length;
  }
  if (last < line.length) out.push({ text: line.slice(last) });
  return out;
}
