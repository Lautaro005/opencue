import type { ReactNode } from "react";
import type { Avatar } from "../lib/types";
import { cx } from "../lib/format";

/* Avatars are a colored shape with a little face on it, drawn on a 100×100 canvas. */

const INK = "#1d1d1f";

function starPoints(cx: number, cy: number, outer: number, inner: number, n = 5) {
  const pts: string[] = [];
  for (let i = 0; i < n * 2; i++) {
    const r = i % 2 === 0 ? outer : inner;
    const a = (Math.PI / n) * i - Math.PI / 2;
    pts.push(`${(cx + r * Math.cos(a)).toFixed(1)},${(cy + r * Math.sin(a)).toFixed(1)}`);
  }
  return pts.join(" ");
}

function ring(n: number, radius: number, r: number, cy = 50) {
  return Array.from({ length: n }, (_, i) => {
    const a = ((Math.PI * 2) / n) * i;
    return <circle key={i} cx={50 + radius * Math.cos(a)} cy={cy + radius * Math.sin(a)} r={r} />;
  });
}

interface ShapeDef {
  color: string;
  /** Where the eyes sit and how big the face is. */
  face: { y: number; scale: number };
  draw: (fill: string) => ReactNode;
}

export const SHAPE_DEFS: Record<string, ShapeDef> = {
  square: { color: "#8e8e93", face: { y: 48, scale: 1 }, draw: (f) => <rect x="9" y="9" width="82" height="82" rx="20" fill={f} /> },
  circle: { color: "#f5b82e", face: { y: 48, scale: 1 }, draw: (f) => <circle cx="50" cy="50" r="42" fill={f} /> },
  bubble: {
    color: "#2f86eb",
    face: { y: 46, scale: 1 },
    draw: (f) => <path d="M50 8C75 8 92 25 92 49V86Q92 92 86 92H50C25 92 8 75 8 50S25 8 50 8Z" fill={f} />,
  },
  heart: {
    color: "#f04848",
    face: { y: 42, scale: 0.9 },
    draw: (f) => <path d="M50 89C22 70 7 55 7 35 7 20 18 10 31 10c8 0 15 4 19 11 4-7 11-11 19-11 13 0 24 10 24 25 0 20-15 35-43 54Z" fill={f} />,
  },
  star: {
    color: "#f7c32f",
    face: { y: 55, scale: 0.78 },
    draw: (f) => <polygon points={starPoints(50, 55, 44, 22)} fill={f} stroke={f} strokeWidth="9" strokeLinejoin="round" />,
  },
  triangle: {
    color: "#2f86eb",
    face: { y: 64, scale: 0.72 },
    draw: (f) => <polygon points="50,14 90,84 10,84" fill={f} stroke={f} strokeWidth="12" strokeLinejoin="round" />,
  },
  drop: {
    color: "#f6ad2b",
    face: { y: 62, scale: 0.85 },
    draw: (f) => <path d="M50 5C61 25 84 42 84 63 84 81 69 94 50 94S16 81 16 63C16 42 39 25 50 5Z" fill={f} />,
  },
  flower: {
    color: "#a78bfa",
    face: { y: 50, scale: 0.85 },
    draw: (f) => (
      <g fill={f}>
        {ring(8, 29, 15)}
        <circle cx="50" cy="50" r="32" />
      </g>
    ),
  },
  ghost: {
    color: "#b7713f",
    face: { y: 44, scale: 0.9 },
    draw: (f) => <path d="M16 46C16 25 31 9 50 9s34 16 34 37v42l-11-8-12 8-11-8-11 8-11-8-12 8Z" fill={f} stroke={f} strokeWidth="5" strokeLinejoin="round" />,
  },
  cloud: {
    color: "#5ec2f2",
    face: { y: 58, scale: 0.85 },
    draw: (f) => (
      <g fill={f}>
        <circle cx="34" cy="56" r="24" />
        <circle cx="58" cy="42" r="28" />
        <circle cx="72" cy="62" r="20" />
        <rect x="14" y="56" width="74" height="26" rx="13" />
      </g>
    ),
  },
  bot: {
    color: "#34b27b",
    face: { y: 58, scale: 0.9 },
    draw: (f) => (
      <g fill={f}>
        <rect x="13" y="26" width="74" height="64" rx="18" />
        <rect x="47" y="10" width="6" height="18" rx="3" />
        <circle cx="50" cy="10" r="7" />
      </g>
    ),
  },
  hexagon: {
    color: "#ff7a45",
    face: { y: 50, scale: 0.95 },
    draw: (f) => <polygon points="50,9 86,29 86,71 50,91 14,71 14,29" fill={f} stroke={f} strokeWidth="10" strokeLinejoin="round" />,
  },
};

export const SHAPES = Object.keys(SHAPE_DEFS);
const LEGACY_SHAPES: Record<string, string> = { squircle: "square", blob: "bubble" };

/** Faces are drawn around (0,0), eyes 15 units apart from the center. */
export const FACES: Record<string, ReactNode> = {
  dots: (
    <g fill={INK}>
      <ellipse cx="-14" cy="0" rx="5" ry="6.5" />
      <ellipse cx="14" cy="0" rx="5" ry="6.5" />
    </g>
  ),
  googly: (
    <g>
      <circle cx="-14" cy="0" r="10" fill="#fff" />
      <circle cx="14" cy="0" r="10" fill="#fff" />
      <circle cx="-12" cy="1.5" r="5" fill={INK} />
      <circle cx="16" cy="1.5" r="5" fill={INK} />
    </g>
  ),
  wink: (
    <g fill="none" stroke={INK} strokeWidth="4.5" strokeLinecap="round" strokeLinejoin="round">
      <ellipse cx="-14" cy="0" rx="4.5" ry="6" fill={INK} stroke="none" />
      <path d="M18 -7 9 0l9 7" />
    </g>
  ),
  happy: (
    <g fill="none" stroke={INK} strokeWidth="4.5" strokeLinecap="round">
      <path d="M-20 3q6-9 12 0" />
      <path d="M8 3q6-9 12 0" />
    </g>
  ),
  smile: (
    <g>
      <g fill={INK}>
        <circle cx="-13" cy="-3" r="5" />
        <circle cx="13" cy="-3" r="5" />
      </g>
      <path d="M-6 8q6 6 12 0" fill="none" stroke={INK} strokeWidth="4" strokeLinecap="round" />
    </g>
  ),
  stern: (
    <g>
      <g fill={INK}>
        <circle cx="-13" cy="2" r="5" />
        <circle cx="13" cy="2" r="5" />
      </g>
      <g stroke={INK} strokeWidth="4.5" strokeLinecap="round">
        <path d="M-21 -11 -7 -7" />
        <path d="M21 -11 7 -7" />
      </g>
    </g>
  ),
  curious: (
    <g>
      <g fill={INK}>
        <circle cx="-13" cy="2" r="5" />
        <circle cx="13" cy="2" r="5" />
      </g>
      <g stroke={INK} strokeWidth="4.5" strokeLinecap="round" fill="none">
        <path d="M-20 -10h12" />
        <path d="M8 -13q6-3 12 0" />
      </g>
    </g>
  ),
  sleepy: (
    <g stroke={INK} strokeWidth="4.5" strokeLinecap="round">
      <path d="M-20 0h12" />
      <path d="M8 0h12" />
    </g>
  ),
  surprised: (
    <g>
      <g fill={INK}>
        <circle cx="-13" cy="-4" r="5" />
        <circle cx="13" cy="-4" r="5" />
      </g>
      <ellipse cx="0" cy="10" rx="4" ry="5" fill={INK} />
    </g>
  ),
  shades: (
    <g fill={INK}>
      <rect x="-24" y="-7" width="19" height="13" rx="5" />
      <rect x="5" y="-7" width="19" height="13" rx="5" />
      <rect x="-6" y="-4" width="12" height="3.5" rx="1.75" />
    </g>
  ),
};

export const FACE_IDS = Object.keys(FACES);

export function normalizeAvatar(a: Avatar): { face: string; color: string; shape: string; legacy?: string } {
  const shape = SHAPE_DEFS[a.shape] ? a.shape : (LEGACY_SHAPES[a.shape] ?? "circle");
  const known = FACES[a.face];
  // Older agents stored an emoji here: keep showing it until the user picks a face.
  const legacy = !known && a.face?.trim() ? a.face.trim() : undefined;
  return { face: known ? a.face : "dots", color: a.color || SHAPE_DEFS[shape].color, shape, legacy };
}

function AvatarSvg({ avatar, size }: { avatar: Avatar; size: number }) {
  const a = normalizeAvatar(avatar);
  const def = SHAPE_DEFS[a.shape];
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" aria-hidden className="block overflow-visible">
      {def.draw(a.color)}
      {a.legacy ? (
        <text x="50" y={def.face.y + 4} textAnchor="middle" dominantBaseline="middle" fontSize={44 * def.face.scale}>
          {a.legacy}
        </text>
      ) : (
        <g transform={`translate(50 ${def.face.y}) scale(${def.face.scale})`}>{FACES[a.face]}</g>
      )}
    </svg>
  );
}

export function AvatarView({ avatar, size = 36, className }: { avatar: Avatar; name?: string; size?: number; className?: string; ring?: boolean }) {
  return (
    <span className={cx("inline-flex shrink-0 select-none", className)} style={{ width: size, height: size }} aria-hidden>
      <AvatarSvg avatar={avatar} size={size} />
    </span>
  );
}

/** Up to three faces stacked like a little crowd, for group chats. */
export function GroupAvatar({ avatars, size = 36 }: { avatars: Avatar[]; size?: number }) {
  const shown = avatars.slice(0, 3);
  if (shown.length === 0) return <AvatarView avatar={{ face: "dots", color: "#c7c7cc", shape: "circle" }} size={size} />;
  if (shown.length === 1) return <AvatarView avatar={shown[0]} size={size} />;
  const small = Math.round(size * 0.52);
  return (
    <span className="relative inline-block shrink-0" style={{ width: size, height: size }} aria-hidden>
      <AvatarView avatar={shown[0]} size={small} className="absolute top-0 left-1/2 -translate-x-1/2" />
      <AvatarView avatar={shown[1]} size={small} className="absolute bottom-0 left-0" />
      {shown[2] && <AvatarView avatar={shown[2]} size={small} className="absolute right-0 bottom-0" />}
    </span>
  );
}

/** The person using the app: an initial on a colored circle, like Cue's user chip. */
export function UserAvatar({ name, size = 28 }: { name: string; size?: number }) {
  return (
    <span
      className="inline-flex shrink-0 items-center justify-center rounded-full bg-[#17a589] font-semibold text-white select-none"
      style={{ width: size, height: size, fontSize: size * 0.46 }}
      aria-hidden
    >
      {name.trim()[0]?.toUpperCase() ?? "?"}
    </span>
  );
}
