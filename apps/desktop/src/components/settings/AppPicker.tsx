import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Search } from "lucide-react";
import { api } from "../../lib/api";
import type { ComposioToolkit } from "../../lib/types";
import { cx } from "../../lib/format";
import { Spinner, useClickOutside } from "../ui";

/**
 * "Connect an app": touch the field to see the list of apps (most popular first), type to narrow
 * it by name. Choosing one starts its sign-in.
 */
export function AppPicker({ busy, onPick }: { busy: boolean; onPick: (slug: string) => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<ComposioToolkit[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  const seq = useRef(0);
  useClickOutside(box, () => setOpen(false), open);

  // Fetch when the list opens and as the user types (debounced; the newest answer wins).
  useEffect(() => {
    if (!open) return;
    const id = ++seq.current;
    setLoading(true);
    const timer = setTimeout(
      () => {
        api
          .composioToolkits(query.trim())
          .then((list) => {
            if (id !== seq.current) return;
            setItems(list);
            setError(null);
            setActive(0);
          })
          .catch((e) => id === seq.current && setError(String(e)))
          .finally(() => id === seq.current && setLoading(false));
      },
      query ? 220 : 0,
    );
    return () => clearTimeout(timer);
  }, [open, query]);

  const pick = (slug: string) => {
    setOpen(false);
    setQuery("");
    onPick(slug);
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setOpen(true);
      setActive((i) => Math.min(items.length - 1, i + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => Math.max(0, i - 1));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (open && items[active]) pick(items[active].slug);
      else if (query.trim()) pick(query.trim().toLowerCase());
    } else if (e.key === "Escape" && open) {
      e.stopPropagation();
      setOpen(false);
    }
  };

  return (
    <div ref={box} className="relative">
      <div className={cx("flex items-center gap-2 rounded-lg border bg-bg px-2.5", open ? "border-accent" : "border-line")}>
        <Search size={14} className="shrink-0 text-faint" />
        <input
          value={query}
          disabled={busy}
          onChange={(e) => {
            setQuery(e.target.value);
            setOpen(true);
          }}
          onFocus={() => setOpen(true)}
          onClick={() => setOpen(true)}
          onKeyDown={onKey}
          placeholder={t("settings.composio.searchApps")}
          aria-label={t("settings.composio.connect")}
          className="h-9 min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-faint"
        />
        {loading && <Spinner size={13} />}
      </div>
      {open && (
        <div role="listbox" className="animate-pop absolute right-0 left-0 z-30 mt-1 max-h-64 overflow-y-auto rounded-xl border border-line bg-elevated p-1 shadow-pop">
          {error ? (
            <p className="px-3 py-2 text-[12.5px] text-danger">{error}</p>
          ) : items.length === 0 && !loading ? (
            <p className="px-3 py-2 text-[12.5px] text-muted">{t("settings.composio.noApps")}</p>
          ) : (
            items.map((a, i) => (
              <button
                key={a.slug}
                role="option"
                aria-selected={i === active}
                onMouseEnter={() => setActive(i)}
                onClick={() => pick(a.slug)}
                className={cx("flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left", i === active ? "bg-hover" : "")}
              >
                {a.logo ? (
                  <img src={a.logo} alt="" className="size-5 shrink-0 rounded object-contain" loading="lazy" />
                ) : (
                  <span className="flex size-5 shrink-0 items-center justify-center rounded bg-hover text-[10px] font-semibold text-muted uppercase">{a.name.slice(0, 1)}</span>
                )}
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-[13px] font-medium">{a.name || a.slug}</span>
                  {a.description && <span className="block truncate text-[11.5px] text-muted">{a.description}</span>}
                </span>
                <Plus size={14} className="shrink-0 text-faint" />
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}
