import { forwardRef, useEffect, useId, useLayoutEffect, useMemo, useRef, useState, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { Check, ChevronDown, Search, X } from "lucide-react";
import { cx } from "../../lib/format";

type Variant = "primary" | "secondary" | "ghost" | "danger";

export function Button({ variant = "secondary", size = "md", className, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant; size?: "sm" | "md" }) {
  return (
    <button
      {...props}
      className={cx(
        "inline-flex items-center justify-center gap-1.5 rounded-lg font-medium transition-colors disabled:opacity-45 disabled:pointer-events-none whitespace-nowrap",
        size === "sm" ? "h-7 px-2.5 text-[12.5px]" : "h-8.5 px-3.5 text-[13px]",
        variant === "primary" && "bg-accent text-white hover:bg-accent-hover",
        variant === "secondary" && "bg-elevated text-ink border border-line hover:bg-hover",
        variant === "ghost" && "text-muted hover:text-ink hover:bg-hover",
        variant === "danger" && "text-danger hover:bg-danger-soft",
        className,
      )}
    />
  );
}

export function IconButton({ label, className, children, active, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; active?: boolean }) {
  return (
    <button
      {...props}
      aria-label={label}
      data-tip={label}
      className={cx(
        "inline-flex size-8 items-center justify-center rounded-lg text-muted transition-colors hover:bg-hover hover:text-ink disabled:opacity-40 disabled:pointer-events-none",
        active && "bg-hover text-ink",
        className,
      )}
    >
      {children}
    </button>
  );
}

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function Input({ className, ...props }, ref) {
  return (
    <input
      ref={ref}
      {...props}
      className={cx(
        "h-9 w-full rounded-lg border border-line bg-bg px-3 text-[13.5px] text-ink placeholder:text-faint outline-none transition-colors focus:border-accent",
        className,
      )}
    />
  );
});

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(function Textarea({ className, ...props }, ref) {
  return (
    <textarea
      ref={ref}
      {...props}
      className={cx(
        "w-full resize-none rounded-lg border border-line bg-bg px-3 py-2 text-[13.5px] leading-relaxed text-ink placeholder:text-faint outline-none transition-colors focus:border-accent",
        className,
      )}
    />
  );
});

export interface SelectOption {
  value: string;
  label: string;
  hint?: string;
}

/** Dropdown drawn by the app (never the system one). The list floats above everything, flips when
 *  there is no room below, and gets a search box when it is long. */
export function Select({
  value,
  onChange,
  options,
  className,
  disabled,
  placeholder,
  searchable,
  align = "left",
}: {
  value: string;
  onChange: (v: string) => void;
  options: SelectOption[];
  className?: string;
  disabled?: boolean;
  placeholder?: string;
  searchable?: boolean;
  align?: "left" | "right";
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [pos, setPos] = useState<{ left: number; top: number; width: number; maxHeight: number } | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const showSearch = searchable ?? options.length > 8;
  const selected = options.find((o) => o.value === value);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? options.filter((o) => o.label.toLowerCase().includes(q) || o.value.toLowerCase().includes(q)) : options;
  }, [options, query]);

  const close = () => {
    setOpen(false);
    setQuery("");
    button.current?.focus();
  };

  const place = () => {
    const b = button.current?.getBoundingClientRect();
    if (!b) return;
    const width = Math.max(b.width, 200);
    const left = align === "right" ? Math.max(8, b.right - width) : Math.min(b.left, window.innerWidth - width - 8);
    const below = window.innerHeight - b.bottom - 12;
    const above = b.top - 12;
    const h = menu.current?.offsetHeight ?? 0;
    const flip = h > below && above > below;
    const maxHeight = Math.min(320, flip ? above : below);
    setPos({ left, width, maxHeight, top: flip ? Math.max(8, b.top - 4 - Math.min(h, maxHeight)) : b.bottom + 4 });
  };

  useLayoutEffect(() => {
    if (open) place();
  }, [open, shown.length]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (!open) return;
    setActive(Math.max(0, shown.findIndex((o) => o.value === value)));
    const away = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!menu.current?.contains(t) && !button.current?.contains(t)) setOpen(false);
    };
    const onScroll = (e: Event) => {
      if (!menu.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", away);
    window.addEventListener("resize", place);
    window.addEventListener("scroll", onScroll, true);
    if (showSearch) requestAnimationFrame(() => search.current?.focus());
    else requestAnimationFrame(() => menu.current?.focus());
    return () => {
      document.removeEventListener("mousedown", away);
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", onScroll, true);
    };
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    menu.current?.querySelector<HTMLElement>(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const pick = (o: SelectOption) => {
    onChange(o.value);
    close();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      close();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      if (shown.length) setActive((i) => (i + (e.key === "ArrowDown" ? 1 : -1) + shown.length) % shown.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (shown[active]) pick(shown[active]);
    } else if (e.key === "Tab") {
      setOpen(false);
    }
  };

  return (
    <span className={cx("relative block", className)}>
      <button
        ref={button}
        type="button"
        disabled={disabled}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        onKeyDown={(e) => {
          if (!open && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
            e.preventDefault();
            setOpen(true);
          }
        }}
        className={cx(
          "flex h-9 w-full items-center rounded-lg border bg-bg pr-8 pl-2.5 text-left text-[13.5px] outline-none transition-colors hover:border-line-strong focus-visible:border-accent disabled:pointer-events-none disabled:opacity-50",
          open ? "border-accent" : "border-line",
        )}
      >
        <span className={cx("truncate", !selected && "text-faint")}>{selected?.label ?? placeholder ?? ""}</span>
        <ChevronDown size={15} className={cx("pointer-events-none absolute top-1/2 right-2.5 -translate-y-1/2 text-muted transition-transform", open && "rotate-180")} />
      </button>
      {open &&
        createPortal(
          <div
            ref={menu}
            tabIndex={-1}
            role="listbox"
            onKeyDown={onKeyDown}
            onMouseDown={(e) => e.target !== search.current && e.preventDefault()}
            style={{ position: "fixed", left: pos?.left ?? 0, top: pos?.top ?? 0, width: pos?.width, maxHeight: pos?.maxHeight, visibility: pos ? "visible" : "hidden" }}
            className="animate-pop z-[80] flex flex-col overflow-hidden rounded-xl border border-line bg-elevated p-1 shadow-pop outline-none"
          >
            {showSearch && (
              <div className="relative mb-1 shrink-0">
                <Search size={13} className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-faint" />
                <input
                  ref={search}
                  value={query}
                  onChange={(e) => {
                    setQuery(e.target.value);
                    setActive(0);
                  }}
                  placeholder={t("common.search")}
                  className="h-8 w-full rounded-lg bg-hover pr-2 pl-8 text-[13px] text-ink outline-none placeholder:text-faint"
                />
              </div>
            )}
            <div className="min-h-0 flex-1 overflow-y-auto">
              {shown.length === 0 && <div className="px-2.5 py-2 text-[12.5px] text-faint">—</div>}
              {shown.map((o, i) => (
                <button
                  key={o.value}
                  type="button"
                  role="option"
                  aria-selected={o.value === value}
                  data-index={i}
                  onMouseEnter={() => setActive(i)}
                  onClick={() => pick(o)}
                  className={cx("flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[13px]", i === active && "bg-hover")}
                >
                  <span className="min-w-0 flex-1">
                    <span className="block truncate">{o.label}</span>
                    {o.hint && <span className="block truncate text-[11.5px] text-faint">{o.hint}</span>}
                  </span>
                  {o.value === value && <Check size={14} className="shrink-0 text-accent" />}
                </button>
              ))}
            </div>
          </div>,
          document.body,
        )}
    </span>
  );
}

export function Toggle({ checked, onChange, disabled, label }: { checked: boolean; onChange: (v: boolean) => void; disabled?: boolean; label?: string }) {
  return (
    <button
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx("relative h-5 w-8.5 shrink-0 rounded-full transition-colors disabled:opacity-40", checked ? "bg-accent" : "bg-line-strong")}
    >
      <span className={cx("absolute top-0.5 left-0 size-4 rounded-full bg-white shadow transition-transform", checked ? "translate-x-4" : "translate-x-0.5")} />
    </button>
  );
}

export function Field({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <label className="block space-y-1.5">
      <span className="block text-[12.5px] font-medium text-muted">{label}</span>
      {children}
      {hint && <span className="block text-[12px] text-faint">{hint}</span>}
    </label>
  );
}

export function Badge({ children, tone = "neutral" }: { children: ReactNode; tone?: "neutral" | "accent" | "success" | "warning" | "danger" }) {
  return (
    <span
      className={cx(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium",
        tone === "neutral" && "bg-hover text-muted",
        tone === "accent" && "bg-accent-soft text-accent",
        tone === "success" && "bg-hover text-success",
        tone === "warning" && "bg-warning-soft text-warning",
        tone === "danger" && "bg-danger-soft text-danger",
      )}
    >
      {children}
    </span>
  );
}

const modalStack: string[] = [];

export function Modal({ open, onClose, children, className, label }: { open: boolean; onClose: () => void; children: ReactNode; className?: string; label: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const id = useId();
  const close = useRef(onClose);
  close.current = onClose;
  useEffect(() => {
    if (!open) return;
    modalStack.push(id);
    // Only the window on top answers Escape, so a picker over a dialog closes alone.
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && modalStack[modalStack.length - 1] === id && close.current();
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      const i = modalStack.indexOf(id);
      if (i >= 0) modalStack.splice(i, 1);
    };
  }, [open, id]);
  if (!open) return null;
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-[var(--overlay)] p-6" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} role="dialog" aria-label={label} className={cx("animate-pop relative max-h-full overflow-hidden rounded-2xl border border-line bg-panel shadow-pop", className)}>
        {children}
      </div>
    </div>,
    document.body,
  );
}

export function DialogHeader({ title, subtitle, onClose }: { title: string; subtitle?: string; onClose: () => void }) {
  return (
    <div className="flex items-start justify-between gap-4 px-6 pt-5 pb-2">
      <div>
        <h2 className="text-[17px] font-semibold tracking-tight">{title}</h2>
        {subtitle && <p className="mt-0.5 text-[13px] text-muted">{subtitle}</p>}
      </div>
      <IconButton label="Close" onClick={onClose} className="-mr-2 -mt-1">
        <X size={17} />
      </IconButton>
    </div>
  );
}

export function Spinner({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" className="animate-spin" aria-hidden>
      <circle cx="12" cy="12" r="9" stroke="currentColor" strokeOpacity="0.2" strokeWidth="3" fill="none" />
      <path d="M21 12a9 9 0 0 0-9-9" stroke="currentColor" strokeWidth="3" fill="none" strokeLinecap="round" />
    </svg>
  );
}

export function Typing() {
  return (
    <span className="inline-flex items-center gap-1" aria-label="typing">
      <span className="typing-dot size-1.5 rounded-full bg-current" />
      <span className="typing-dot size-1.5 rounded-full bg-current" />
      <span className="typing-dot size-1.5 rounded-full bg-current" />
    </span>
  );
}

/** Close a popover when clicking outside of it. */
export function useClickOutside(ref: React.RefObject<HTMLElement | null>, onOutside: () => void, active = true) {
  useEffect(() => {
    if (!active) return;
    const h = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onOutside();
    };
    document.addEventListener("mousedown", h);
    return () => document.removeEventListener("mousedown", h);
  }, [ref, onOutside, active]);
}
