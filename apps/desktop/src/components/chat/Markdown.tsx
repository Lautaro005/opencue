import { memo, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { ExternalLink } from "lucide-react";
import { openUrl } from "../../lib/api";

/** "https://www.reuters.com/world/x" → "reuters.com". */
function hostOf(href: string) {
  try {
    return new URL(href).hostname.replace(/^www\./, "");
  } catch {
    return href;
  }
}

/** Plain text of a link's children, to tell `[AP](…)` from a bare URL. */
function textOf(children: ReactNode): string {
  if (typeof children === "string") return children;
  if (Array.isArray(children)) return children.map(textOf).join("");
  return "";
}

/** Every link is a small pill: the site's name (or the link's own words) and an arrow out. */
function LinkPill({ href, children }: { href?: string; children: ReactNode }) {
  if (!href) return <>{children}</>;
  const text = textOf(children).trim();
  const bare = !text || text === href || text.replace(/\/$/, "") === href.replace(/\/$/, "") || /^https?:\/\//i.test(text);
  const label = bare ? hostOf(href) : text;
  return (
    <a
      href={href}
      title={href}
      data-tip={href}
      onClick={(e) => {
        e.preventDefault();
        void openUrl(href);
      }}
      className="link-pill"
    >
      <span className="truncate">{label}</span>
      <ExternalLink size={11} className="shrink-0 opacity-70" aria-hidden />
    </a>
  );
}

export const Markdown = memo(function Markdown({ text }: { text: string }) {
  return (
    <div className="prose-chat break-words">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={{ a: ({ href, children }) => <LinkPill href={href}>{children}</LinkPill> }}>
        {text}
      </ReactMarkdown>
    </div>
  );
});
