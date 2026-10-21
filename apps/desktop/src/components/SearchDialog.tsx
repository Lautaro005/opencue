import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { CornerDownLeft, Search } from "lucide-react";
import { api } from "../lib/api";
import { cx, splitThink } from "../lib/format";
import { useStore } from "../lib/store";
import type { SearchHit } from "../lib/types";
import { AvatarView, GroupAvatar } from "./AvatarView";
import { Spinner } from "./ui";

/** A short piece of the message around the first match, with the match marked. */
function Snippet({ text, query }: { text: string; query: string }) {
  const flat = splitThink(text).text.replace(/\s+/g, " ").trim();
  const i = flat.toLowerCase().indexOf(query.toLowerCase());
  if (i < 0) return <>{flat.slice(0, 140)}</>;
  const start = Math.max(0, i - 40);
  const end = Math.min(flat.length, i + query.length + 90);
  return (
    <>
      {start > 0 && "…"}
      {flat.slice(start, i)}
      <mark className="rounded bg-warning-soft px-0.5 text-ink">{flat.slice(i, i + query.length)}</mark>
      {flat.slice(i + query.length, end)}
      {end < flat.length && "…"}
    </>
  );
}

/** Search every chat for a message, like a Spotlight bar: type, arrow keys, Enter to jump to it. */
export function SearchDialog() {
  const { t } = useTranslation();
  const open = useStore((s) => s.searchOpen);
  const setOpen = useStore((s) => s.setSearchOpen);
  const threads = useStore((s) => s.threads);
  const agents = useStore((s) => s.agents);
  const openMessage = useStore((s) => s.openMessage);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [busy, setBusy] = useState(false);
  const [index, setIndex] = useState(0);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setHits([]);
      setIndex(0);
      requestAnimationFrame(() => input.current?.focus());
    }
  }, [open]);

  useEffect(() => {
    const q = query.trim();
    if (!open || !q) {
      setHits([]);
      setBusy(false);
      return;
    }
    setBusy(true);
    const timer = window.setTimeout(() => {
      void api
        .searchMessages(q)
        .then((r) => {
          setHits(r);
          setIndex(0);
        })
        .catch(() => setHits([]))
        .finally(() => setBusy(false));
    }, 160);
    return () => window.clearTimeout(timer);
  }, [query, open]);

  const byThread = useMemo(() => Object.fromEntries(threads.map((th) => [th.id, th])), [threads]);
  if (!open) return null;

  const go = (h: SearchHit) => {
    setOpen(false);
    void openMessage(h.thread_id, h.message_id);
  };

  return createPortal(
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-[var(--overlay)] px-6 pt-[14vh]" onMouseDown={(e) => e.target === e.currentTarget && setOpen(false)}>
      <div role="dialog" aria-label={t("search.title")} className="animate-pop w-full max-w-[600px] overflow-hidden rounded-2xl border border-line bg-panel shadow-pop">
        <div className="flex items-center gap-3 px-4 py-3">
          <Search size={18} className="shrink-0 text-muted" />
          <input
            ref={input}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") setOpen(false);
              else if (e.key === "ArrowDown") {
                e.preventDefault();
                setIndex((i) => Math.min(hits.length - 1, i + 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setIndex((i) => Math.max(0, i - 1));
              } else if (e.key === "Enter" && hits[index]) go(hits[index]);
            }}
            placeholder={t("search.placeholder")}
            spellCheck={false}
            className="min-w-0 flex-1 bg-transparent text-[17px] outline-none placeholder:text-faint"
          />
          {busy && <Spinner size={15} />}
        </div>
        {query.trim() && (
          <ul className="max-h-[52vh] overflow-y-auto border-t border-line p-1.5">
            {hits.length === 0 && !busy && <li className="px-3 py-6 text-center text-[13px] text-muted">{t("search.none")}</li>}
            {hits.map((h, i) => {
              const th = byThread[h.thread_id];
              const who = h.role === "user" ? t("search.you") : h.agent_id ? agents[h.agent_id]?.name : "";
              return (
                <li key={h.message_id}>
                  <button onClick={() => go(h)} onMouseEnter={() => setIndex(i)} className={cx("flex w-full items-start gap-3 rounded-xl px-3 py-2 text-left", i === index && "bg-hover")}>
                    <span className="mt-0.5 shrink-0">
                      {th ? th.kind === "group" ? <GroupAvatar avatars={th.avatars} size={30} /> : <AvatarView avatar={th.avatars[0]} name={th.name} size={30} /> : null}
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="flex items-baseline gap-2">
                        <span className="truncate text-[13.5px] font-medium">{th?.name ?? "—"}</span>
                        <span className="shrink-0 text-[11px] text-faint">{new Date(h.created_at).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })}</span>
                      </span>
                      <span className="line-clamp-2 block text-[12.5px] leading-snug text-muted">
                        {who && <span className="font-medium text-ink/80">{who}: </span>}
                        <Snippet text={h.content} query={query.trim()} />
                      </span>
                    </span>
                    {i === index && <CornerDownLeft size={14} className="mt-1 shrink-0 text-faint" />}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </div>,
    document.body,
  );
}
