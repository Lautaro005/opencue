import { useTranslation } from "react-i18next";
import { RotateCcw } from "lucide-react";
import type { Avatar } from "../../lib/types";
import { cx } from "../../lib/format";
import { AvatarView, FACES, FACE_IDS, SHAPES, SHAPE_DEFS, normalizeAvatar } from "../AvatarView";

export const COLORS = ["#8e8e93", "#f5b82e", "#ff7a45", "#f04848", "#f472b6", "#a78bfa", "#2f86eb", "#5ec2f2", "#34b27b", "#b7713f"];
export { SHAPES };

export function randomAvatar(): Avatar {
  const pick = <T,>(xs: T[]) => xs[Math.floor(Math.random() * xs.length)];
  const shape = pick(SHAPES);
  return { face: pick(FACE_IDS), shape, color: SHAPE_DEFS[shape].color };
}

export function AvatarPicker({ value, onChange, onReset }: { value: Avatar; onChange: (a: Avatar) => void; onReset?: () => void }) {
  const { t } = useTranslation();
  const a = normalizeAvatar(value);
  return (
    <div className="w-[332px] p-3">
      <div className="mb-2 flex items-center justify-between px-1">
        <span className="text-[13px] font-medium">{t("profile.avatar")}</span>
        <button onClick={() => (onReset ? onReset() : onChange(randomAvatar()))} className="rounded-md p-1 text-muted hover:bg-hover hover:text-fg" title={t("profile.randomize")}>
          <RotateCcw size={15} />
        </button>
      </div>
      <div className="grid grid-cols-5 gap-1.5">
        {FACE_IDS.map((f) => (
          <button
            key={f}
            onClick={() => onChange({ ...a, face: f })}
            className={cx("flex aspect-square items-center justify-center rounded-xl hover:bg-hover", a.face === f && !a.legacy && "bg-active")}
            aria-label={f}
          >
            <svg viewBox="0 0 100 100" width="48" height="48" aria-hidden>
              <circle cx="50" cy="50" r="40" fill="var(--hover)" />
              <g transform="translate(50 50) scale(0.95)">{FACES[f]}</g>
            </svg>
          </button>
        ))}
      </div>
      <div className="my-2.5 border-t border-line" />
      <div className="grid grid-cols-6 gap-1">
        {SHAPES.map((s) => (
          <button
            key={s}
            onClick={() => onChange({ ...a, shape: s, color: SHAPE_DEFS[s].color })}
            className={cx("flex aspect-square items-center justify-center rounded-xl hover:bg-hover", a.shape === s && "bg-active")}
            aria-label={s}
          >
            <svg viewBox="0 0 100 100" width="38" height="38" aria-hidden className="overflow-visible">
              {SHAPE_DEFS[s].draw(a.shape === s ? a.color : SHAPE_DEFS[s].color)}
            </svg>
          </button>
        ))}
      </div>
      <div className="mt-2.5 flex items-center justify-between gap-1 px-1">
        {COLORS.map((c) => (
          <button
            key={c}
            onClick={() => onChange({ ...a, color: c })}
            className={cx("size-5 rounded-full transition-transform hover:scale-110", a.color.toLowerCase() === c && "ring-2 ring-offset-2 ring-offset-[var(--elevated)]")}
            style={{ background: c, ["--tw-ring-color" as string]: c }}
            aria-label={c}
          />
        ))}
      </div>
      <div className="mt-3 flex justify-center">
        <AvatarView avatar={value} size={44} />
      </div>
    </div>
  );
}
