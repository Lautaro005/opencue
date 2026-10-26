import { SquareTerminal } from "lucide-react";
import { ProviderLogo } from "../ProviderLogo";
import type { ProviderKind } from "../../lib/types";

const KIND: Record<string, ProviderKind> = { claude: "claude_cli", codex: "codex_cli", grok: "grok_cli", gemini: "gemini", agy: "antigravity_cli", vibe: "vibe_cli", muse: "muse_cli" };

/** The logo of a terminal tool, or a terminal glyph for a plain shell. */
export function TermIcon({ tool, size = 16 }: { tool: string; size?: number }) {
  const kind = KIND[tool];
  return kind ? <ProviderLogo kind={kind} size={size} /> : <SquareTerminal size={size} />;
}
