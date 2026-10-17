import { useTranslation } from "react-i18next";
import type { Avatar } from "../../lib/types";
import { cx } from "../../lib/format";
import { AvatarView } from "../AvatarView";

export const EMOJIS = [
  "🦊", "🐙", "🦉", "🐝", "🐳", "🦄", "🐼", "🐨", "🦁", "🐸", "🐧", "🦋",
  "🌵", "🌸", "🍋", "🍄", "🌙", "⭐", "🔥", "🌊", "🪐", "☕", "🎧", "📚",
  "✈️", "🧭", "🧪", "🎨", "💼", "🏃", "🧘", "🍳", "💡", "🤖", "👾", "",
];
export const COLORS = ["#7c8cff", "#3b93ff", "#4db6ac", "#81c784", "#ffb74d", "#ff8a65", "#f06292", "#ba68c8", "#9575cd", "#90a4ae"];
export const SHAPES = ["circle", "squircle", "blob"];

export function AvatarPicker({ value, onChange }: { value: Avatar; onChange: (a: Avatar) => void }) {
  const { t } = useTranslation();
  return (
    <div className="w-[300px] space-y-3 p-3">
      <div>
        <div className="mb-1.5 text-[11.5px] font-medium text-muted">{t("profile.emoji")}</div>
        <div className="grid grid-cols-9 gap-0.5">
          {EMOJIS.map((e) => (
            <button
              key={e || "none"}
              onClick={() => onChange({ ...value, emoji: e })}
              className={cx("flex size-7.5 items-center justify-center rounded-lg text-[17px] hover:bg-hover", value.emoji === e && "bg-active ring-1 ring-line-strong")}
              aria-label={e || "Initial"}
            >
              {e || <span className="text-[12px] font-semibold text-muted">Aa</span>}
            </button>
          ))}
        </div>
      </div>
      <div>
        <div className="mb-1.5 text-[11.5px] font-medium text-muted">{t("profile.color")}</div>
        <div className="flex flex-wrap gap-1.5">
          {COLORS.map((c) => (
            <button
              key={c}
              onClick={() => onChange({ ...value, color: c })}
              className={cx("size-6 rounded-full transition-transform hover:scale-110", value.color === c && "ring-2 ring-offset-2 ring-offset-[var(--elevated)]")}
              style={{ background: c, ["--tw-ring-color" as string]: c }}
              aria-label={c}
            />
          ))}
        </div>
      </div>
      <div>
        <div className="mb-1.5 text-[11.5px] font-medium text-muted">{t("profile.shape")}</div>
        <div className="flex gap-2">
          {SHAPES.map((s) => (
            <button key={s} onClick={() => onChange({ ...value, shape: s })} className={cx("rounded-xl p-1 hover:bg-hover", value.shape === s && "bg-active")} aria-label={s}>
              <AvatarView avatar={{ ...value, shape: s }} size={34} />
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
