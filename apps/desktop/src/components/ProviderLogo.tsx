import anthropic from "../assets/providers/anthropic.png";
import antigravity from "../assets/providers/antigravity.png";
import cheaperinference from "../assets/providers/cheaperinference.png";
import gemini from "../assets/providers/gemini.png";
import grok from "../assets/providers/grok.png";
import groq from "../assets/providers/groq.png";
import lmstudio from "../assets/providers/lmstudio.png";
import mistral from "../assets/providers/mistral.png";
import muse from "../assets/providers/muse.png";
import ollama from "../assets/providers/ollama.png";
import openai from "../assets/providers/openai.webp";
import openrouter from "../assets/providers/openrouter.svg";
import { Server } from "lucide-react";
import type { ProviderKind } from "../lib/types";
import { cx } from "../lib/format";

/** One logo per brand: the API and the CLI of the same company share it. */
const LOGOS: Partial<Record<ProviderKind, string>> = {
  openai,
  codex_cli: openai,
  anthropic,
  claude_cli: anthropic,
  xai: grok,
  grok_cli: grok,
  gemini,
  antigravity_cli: antigravity,
  muse,
  muse_cli: muse,
  mistral_api: mistral,
  vibe_cli: mistral,
  groq,
  openrouter,
  cheaperinference,
  ollama,
  ollama_cloud: ollama,
  lmstudio,
};

/** The provider's logo (black artwork, inverted in dark mode); a plain server icon for custom ones. */
export function ProviderLogo({ kind, size = 18, className }: { kind: ProviderKind; size?: number; className?: string }) {
  const src = LOGOS[kind];
  if (!src) return <Server size={size} className={className} />;
  return <img src={src} alt="" width={size} height={size} draggable={false} className={cx("shrink-0 object-contain dark:invert", className)} style={{ width: size, height: size }} />;
}
