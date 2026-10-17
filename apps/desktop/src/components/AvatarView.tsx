import type { Avatar } from "../lib/types";
import { cx } from "../lib/format";

const RADIUS: Record<string, string> = {
  circle: "50%",
  squircle: "32%",
  blob: "42% 58% 55% 45% / 48% 42% 58% 52%",
};

export function AvatarView({ avatar, name, size = 36, className, ring }: { avatar: Avatar; name?: string; size?: number; className?: string; ring?: boolean }) {
  const emoji = avatar.emoji?.trim();
  return (
    <span
      className={cx("inline-flex shrink-0 select-none items-center justify-center font-semibold text-white", ring && "ring-2 ring-[var(--sidebar)]", className)}
      style={{
        width: size,
        height: size,
        borderRadius: RADIUS[avatar.shape] ?? RADIUS.circle,
        background: emoji ? `color-mix(in srgb, ${avatar.color} 26%, var(--bg))` : avatar.color,
        fontSize: emoji ? size * 0.52 : size * 0.42,
        lineHeight: 1,
      }}
      aria-hidden
    >
      {emoji || (name?.[0]?.toUpperCase() ?? "?")}
    </span>
  );
}

/** Overlapping faces for a group chat. */
export function GroupAvatar({ avatars, size = 36 }: { avatars: Avatar[]; size?: number }) {
  const shown = avatars.slice(0, 3);
  if (shown.length === 0) {
    return <AvatarView avatar={{ emoji: "👥", color: "#9ca3af", shape: "circle" }} size={size} />;
  }
  if (shown.length === 1) return <AvatarView avatar={shown[0]} size={size} />;
  const small = Math.round(size * 0.66);
  return (
    <span className="relative inline-block shrink-0" style={{ width: size, height: size }} aria-hidden>
      <AvatarView avatar={shown[0]} size={small} className="absolute top-0 left-0" ring />
      <AvatarView avatar={shown[1]} size={small} className="absolute right-0 bottom-0" ring />
      {shown[2] && <AvatarView avatar={shown[2]} size={Math.round(size * 0.44)} className="absolute bottom-0 left-0" ring />}
    </span>
  );
}
