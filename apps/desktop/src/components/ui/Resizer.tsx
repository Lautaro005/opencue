import { useCallback, useEffect, useRef, useState } from "react";
import { cx } from "../../lib/format";

const read = (key: string, fallback: number, min: number, max: number) => {
  try {
    const v = Number(localStorage.getItem(key));
    if (v >= min && v <= max) return v;
  } catch {
    /* storage unavailable: use the default */
  }
  return fallback;
};

/**
 * A panel width the user can drag, remembered between launches. `edge` is the side of the panel
 * the handle sits on: "right" for a left sidebar, "left" for a panel on the right.
 */
export function useResizable(key: string, initial: number, min: number, max: number, edge: "left" | "right") {
  const [width, setWidth] = useState(() => read(key, initial, min, max));
  const [dragging, setDragging] = useState(false);
  const start = useRef({ x: 0, w: 0 });

  const onDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      start.current = { x: e.clientX, w: width };
      setDragging(true);
    },
    [width],
  );

  useEffect(() => {
    if (!dragging) return;
    const move = (e: PointerEvent) => {
      const dx = e.clientX - start.current.x;
      const next = start.current.w + (edge === "right" ? dx : -dx);
      setWidth(Math.min(max, Math.max(min, next)));
    };
    const up = () => setDragging(false);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    return () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  }, [dragging, edge, min, max]);

  useEffect(() => {
    if (dragging) return;
    try {
      localStorage.setItem(key, String(Math.round(width)));
    } catch {
      /* not persisted */
    }
  }, [dragging, key, width]);

  const reset = useCallback(() => setWidth(initial), [initial]);
  const handle = (
    <div
      role="separator"
      aria-orientation="vertical"
      onPointerDown={onDown}
      onDoubleClick={reset}
      className={cx("group absolute top-0 bottom-0 z-20 w-2 cursor-col-resize", edge === "right" ? "-right-1" : "-left-1")}
    >
      <span className={cx("absolute inset-y-0 left-1/2 w-px -translate-x-1/2 transition-colors", dragging ? "bg-accent" : "bg-transparent group-hover:bg-accent/60")} />
    </div>
  );
  return { width, handle };
}
