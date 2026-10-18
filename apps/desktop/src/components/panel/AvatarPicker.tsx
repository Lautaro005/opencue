import { useTranslation } from "react-i18next";
import { RotateCcw } from "lucide-react";
import type { Avatar } from "../../lib/types";
import { cx } from "../../lib/format";
import { AvatarView, FACES, FACE_IDS, SHAPES, SHAPE_DEFS, normalizeAvatar } from "../AvatarView";
import { Button, DialogHeader, Modal } from "../ui";

export const COLORS = ["#8e8e93", "#f5b82e", "#ff7a45", "#f04848", "#f472b6", "#a78bfa", "#2f86eb", "#5ec2f2", "#34b27b", "#b7713f"];
export { SHAPES };

export function randomAvatar(): Avatar {
  const pick = <T,>(xs: T[]) => xs[Math.floor(Math.random() * xs.length)];
  const shape = pick(SHAPES);
  return { face: pick(FACE_IDS), shape, color: SHAPE_DEFS[shape].color };
}

/** The avatar editor in its own floating window, so it can open over any dialog or panel. */
export function AvatarPickerDialog({ open, value, onChange, onClose }: { open: boolean; value: Avatar; onChange: (a: Avatar) => void; onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <Modal open={open} onClose={onClose} className="w-[372px]" label={t("profile.avatar")}>
      <DialogHeader title={t("profile.avatar")} onClose={onClose} />
      <AvatarPicker value={value} onChange={onChange} />
      <div className="flex justify-end px-5 pb-4">
        <Button variant="primary" onClick={onClose}>
          {t("common.done")}
        </Button>
      </div>
    </Modal>
  );
}

export function AvatarPicker({ value, onChange }: { value: Avatar; onChange: (a: Avatar) => void }) {
  const { t } = useTranslation();
  const a = normalizeAvatar(value);
  return (
    <div className="px-5 pt-1 pb-4">
      <div className="mb-3 flex items-center gap-3">
        <AvatarView avatar={value} size={56} />
        <div className="min-w-0 flex-1 text-[12.5px] text-muted">{t("profile.avatarHint")}</div>
        <button onClick={() => onChange(randomAvatar())} className="rounded-lg p-2 text-muted hover:bg-hover hover:text-ink" aria-label={t("profile.randomize")} data-tip={t("profile.randomize")}>
          <RotateCcw size={16} />
        </button>
      </div>

      <div className="mb-1.5 text-[12px] font-medium text-muted">{t("profile.face")}</div>
      <div className="grid grid-cols-5 gap-1.5">
        {FACE_IDS.map((f) => (
          <button
            key={f}
            onClick={() => onChange({ ...a, face: f })}
            className={cx("flex aspect-square items-center justify-center rounded-xl border transition-colors hover:bg-hover", a.face === f && !a.legacy ? "border-accent bg-accent-soft" : "border-transparent")}
            aria-label={f}
          >
            {/* The face sits on the avatar's own color, so it is always easy to see. */}
            <svg viewBox="0 0 100 100" width="100%" height="100%" className="p-1.5" aria-hidden>
              <circle cx="50" cy="50" r="44" fill={a.color} />
              <g transform="translate(50 50) scale(1.05)">{FACES[f]}</g>
            </svg>
          </button>
        ))}
      </div>

      <div className="mt-3 mb-1.5 text-[12px] font-medium text-muted">{t("profile.shape")}</div>
      <div className="grid grid-cols-6 gap-1.5">
        {SHAPES.map((s) => (
          <button
            key={s}
            onClick={() => onChange({ ...a, shape: s, color: SHAPE_DEFS[s].color })}
            className={cx("flex aspect-square items-center justify-center rounded-xl border transition-colors hover:bg-hover", a.shape === s ? "border-accent bg-accent-soft" : "border-transparent")}
            aria-label={s}
          >
            <svg viewBox="0 0 100 100" width="100%" height="100%" className="overflow-visible p-2" aria-hidden>
              {SHAPE_DEFS[s].draw(a.shape === s ? a.color : SHAPE_DEFS[s].color)}
            </svg>
          </button>
        ))}
      </div>

      <div className="mt-3 mb-1.5 text-[12px] font-medium text-muted">{t("profile.color")}</div>
      <div className="flex items-center justify-between">
        {COLORS.map((c) => (
          <button
            key={c}
            onClick={() => onChange({ ...a, color: c })}
            className={cx("size-6 rounded-full transition-transform hover:scale-110", a.color.toLowerCase() === c && "ring-2 ring-offset-2 ring-offset-[var(--panel)]")}
            style={{ background: c, ["--tw-ring-color" as string]: c }}
            aria-label={c}
          />
        ))}
      </div>
    </div>
  );
}
