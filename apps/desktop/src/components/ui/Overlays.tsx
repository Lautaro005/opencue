import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { ClipboardPaste, Copy, MousePointerSquareDashed, Scissors } from "lucide-react";
import { useConfirm } from "../../lib/confirm";
import { cx } from "../../lib/format";
import { Button, Modal } from "./index";

/** One confirmation window for the whole app, driven by `confirmDialog()`. */
export function ConfirmHost() {
  const { t } = useTranslation();
  const req = useConfirm((s) => s.req);
  const answer = (v: boolean) => {
    if (!req) return;
    req.resolve(v);
    useConfirm.setState({ req: null });
  };
  return (
    <Modal open={!!req} onClose={() => answer(false)} className="w-[400px]" label={req?.title ?? t("common.confirm")}>
      <div className="px-6 pt-5 pb-5">
        <h2 className="text-[16px] font-semibold tracking-tight">{req?.title ?? t("common.confirm")}</h2>
        <p className="mt-1.5 text-[13.5px] leading-relaxed text-muted">{req?.message}</p>
        <div className="mt-5 flex justify-end gap-2">
          <Button onClick={() => answer(false)}>{t("common.cancel")}</Button>
          <Button
            variant="primary"
            className={cx(req?.danger !== false && "bg-danger hover:bg-danger hover:opacity-90")}
            onClick={() => answer(true)}
            autoFocus
          >
            {req?.confirmLabel ?? t("common.confirm")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/** Tooltips for anything with a `data-tip`, drawn by the app instead of the system. */
export function TooltipHost() {
  const [tip, setTip] = useState<{ text: string; x: number; y: number; below: boolean } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const el = useRef<HTMLDivElement>(null);
  const target = useRef<HTMLElement | null>(null);

  useEffect(() => {
    const hide = () => {
      if (timer.current) clearTimeout(timer.current);
      target.current = null;
      setTip(null);
    };
    const over = (e: MouseEvent) => {
      const t = (e.target as HTMLElement | null)?.closest?.("[data-tip]") as HTMLElement | null;
      if (t === target.current) return;
      hide();
      if (!t || !t.dataset.tip) return;
      target.current = t;
      timer.current = setTimeout(() => {
        if (!t.isConnected) return;
        const r = t.getBoundingClientRect();
        const below = r.top < 44;
        setTip({ text: t.dataset.tip!, x: r.left + r.width / 2, y: below ? r.bottom + 6 : r.top - 6, below });
      }, 450);
    };
    document.addEventListener("mouseover", over);
    document.addEventListener("mousedown", hide, true);
    document.addEventListener("keydown", hide, true);
    window.addEventListener("scroll", hide, true);
    return () => {
      document.removeEventListener("mouseover", over);
      document.removeEventListener("mousedown", hide, true);
      document.removeEventListener("keydown", hide, true);
      window.removeEventListener("scroll", hide, true);
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);

  useLayoutEffect(() => {
    // Keep it inside the window horizontally.
    const node = el.current;
    if (!node || !tip) return;
    const w = node.offsetWidth;
    const left = Math.min(Math.max(8, tip.x - w / 2), window.innerWidth - w - 8);
    node.style.left = left + "px";
  }, [tip]);

  if (!tip) return null;
  return createPortal(
    <div
      ref={el}
      role="tooltip"
      style={{ position: "fixed", top: tip.y, transform: tip.below ? undefined : "translateY(-100%)", left: tip.x }}
      className="pointer-events-none z-[90] max-w-[260px] rounded-lg bg-ink px-2 py-1 text-[11.5px] leading-snug font-medium text-bg shadow-pop"
    >
      {tip.text}
    </div>,
    document.body,
  );
}

type MenuItem = { label: string; icon: React.ReactNode; run: () => void; disabled?: boolean };

function isTextField(el: Element | null): el is HTMLInputElement | HTMLTextAreaElement {
  if (!el) return false;
  if (el instanceof HTMLTextAreaElement) return true;
  if (el instanceof HTMLInputElement) return ["text", "search", "url", "email", "tel", "password", "number", ""].includes(el.type);
  return false;
}

/** Replaces the system right-click menu with a small Copy / Cut / Paste one. */
export function ContextMenuHost() {
  const { t } = useTranslation();
  const [menu, setMenu] = useState<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onContext = (e: MouseEvent) => {
      e.preventDefault();
      const target = e.target as HTMLElement;
      const field = target.closest("input, textarea");
      const items: MenuItem[] = [];
      if (isTextField(field)) {
        field.focus();
        const has = (field.selectionStart ?? 0) !== (field.selectionEnd ?? 0) && field.type !== "password";
        const editable = !field.readOnly && !field.disabled;
        if (editable) items.push({ label: t("context.cut"), icon: <Scissors size={14} />, disabled: !has, run: () => cut(field) });
        items.push({ label: t("context.copy"), icon: <Copy size={14} />, disabled: !has, run: () => void copyText(selectedIn(field)) });
        if (editable) items.push({ label: t("context.paste"), icon: <ClipboardPaste size={14} />, run: () => void paste(field) });
        items.push({ label: t("context.selectAll"), icon: <MousePointerSquareDashed size={14} />, run: () => field.select() });
      } else {
        const sel = window.getSelection()?.toString() ?? "";
        const block = target.closest(".selectable, .prose-chat");
        if (sel) items.push({ label: t("context.copy"), icon: <Copy size={14} />, run: () => void copyText(sel) });
        if (block) {
          items.push({
            label: t("context.selectAll"),
            icon: <MousePointerSquareDashed size={14} />,
            run: () => {
              const range = document.createRange();
              range.selectNodeContents(block);
              const s = window.getSelection();
              s?.removeAllRanges();
              s?.addRange(range);
            },
          });
        }
      }
      setMenu(items.length ? { x: e.clientX, y: e.clientY, items } : null);
    };
    const hide = (e: Event) => {
      if (ref.current?.contains(e.target as Node)) return;
      setMenu(null);
    };
    const key = (e: KeyboardEvent) => e.key === "Escape" && setMenu(null);
    document.addEventListener("contextmenu", onContext);
    document.addEventListener("mousedown", hide, true);
    window.addEventListener("blur", () => setMenu(null));
    window.addEventListener("keydown", key);
    window.addEventListener("scroll", hide, true);
    return () => {
      document.removeEventListener("contextmenu", onContext);
      document.removeEventListener("mousedown", hide, true);
      window.removeEventListener("keydown", key);
      window.removeEventListener("scroll", hide, true);
    };
  }, [t]);

  useLayoutEffect(() => {
    const node = ref.current;
    if (!node || !menu) return;
    node.style.left = Math.min(menu.x, window.innerWidth - node.offsetWidth - 8) + "px";
    node.style.top = Math.min(menu.y, window.innerHeight - node.offsetHeight - 8) + "px";
  }, [menu]);

  if (!menu) return null;
  return createPortal(
    <div
      ref={ref}
      role="menu"
      onMouseDown={(e) => e.preventDefault()}
      style={{ position: "fixed", left: menu.x, top: menu.y }}
      className="animate-pop z-[95] w-48 rounded-xl border border-line bg-elevated p-1 shadow-pop"
    >
      {menu.items.map((i) => (
        <button
          key={i.label}
          role="menuitem"
          disabled={i.disabled}
          onClick={() => {
            i.run();
            setMenu(null);
          }}
          className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-1.5 text-left text-[13px] hover:bg-hover disabled:opacity-40 disabled:hover:bg-transparent"
        >
          <span className="text-muted">{i.icon}</span>
          {i.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}

function selectedIn(el: HTMLInputElement | HTMLTextAreaElement) {
  return el.value.slice(el.selectionStart ?? 0, el.selectionEnd ?? 0);
}

async function copyText(text: string) {
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    document.execCommand("copy");
  }
}

function cut(el: HTMLInputElement | HTMLTextAreaElement) {
  const text = selectedIn(el);
  if (!text) return;
  void copyText(text);
  el.focus();
  document.execCommand("delete");
}

async function paste(el: HTMLInputElement | HTMLTextAreaElement) {
  try {
    const text = await navigator.clipboard.readText();
    el.focus();
    // insertText keeps undo history and fires the input event React listens to.
    if (!document.execCommand("insertText", false, text)) {
      const start = el.selectionStart ?? el.value.length;
      const end = el.selectionEnd ?? start;
      const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(el), "value")?.set;
      setter?.call(el, el.value.slice(0, start) + text + el.value.slice(end));
      el.dispatchEvent(new Event("input", { bubbles: true }));
    }
  } catch {
    /* clipboard blocked: nothing to paste */
  }
}
