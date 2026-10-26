import { api, pickFolder } from "./api";
import { greetNewAgent } from "./greet";
import { isConfigured } from "./models";
import { AGENT_TEMPLATES, templateAvatar, type AgentTemplate, type GroupTemplate } from "./templates";
import { useStore } from "./store";
import { FACE_IDS, SHAPES, SHAPE_DEFS, normalizeAvatar } from "../components/AvatarView";
import { COLORS } from "../components/panel/AvatarPicker";
import { TEMPLATE_KITS, type TemplateKit } from "./templateKits";
import type { Agent, Avatar, GrokTemplate } from "./types";

/** The provider a new agent starts with: the default one, else the first that is set up. */
async function defaultProvider(): Promise<string | null> {
  const { providers, settings } = useStore.getState();
  const catalog = await api.providerCatalog();
  const usable = providers.filter((p) => isConfigured(p, catalog.find((x) => x.kind === p.kind)));
  return settings.default_provider && usable.some((p) => p.id === settings.default_provider) ? settings.default_provider : usable[0]?.id ?? null;
}

const hexRgb = (hex: string): [number, number, number] | null => {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
};

const NAMED: Record<string, string> = { red: "#f04848", orange: "#ff7a45", yellow: "#f5b82e", amber: "#f5b82e", green: "#34b27b", teal: "#34b27b", cyan: "#5ec2f2", blue: "#2f86eb", violet: "#a78bfa", purple: "#a78bfa", magenta: "#f472b6", pink: "#f472b6", brown: "#b7713f", gray: "#8e8e93", grey: "#8e8e93" };

/** The color of ours closest to one from outside (a name like "magenta" or a hex code). */
export function nearestColor(c: string): string | null {
  const named = NAMED[c.trim().toLowerCase()];
  if (named) return named;
  const rgb = hexRgb(c);
  if (!rgb) return null;
  let best = COLORS[0];
  let bestD = Infinity;
  for (const p of COLORS) {
    const q = hexRgb(p)!;
    const d = (q[0] - rgb[0]) ** 2 + (q[1] - rgb[1]) ** 2 + (q[2] - rgb[2]) ** 2;
    if (d < bestD) {
      bestD = d;
      best = p;
    }
  }
  return best;
}

const SHAPE_ALIASES: Record<string, string> = { teardrop: "drop", squircle: "square", blob: "bubble", round: "circle", rounded: "square" };

/** An avatar made only of our own shapes, faces and colors, whatever it was made from. */
export function safeAvatar(a: Avatar): Avatar {
  const shape = SHAPE_DEFS[a.shape] ? a.shape : SHAPE_ALIASES[a.shape] ?? "circle";
  const n = normalizeAvatar({ ...a, shape });
  const color = COLORS.includes(n.color.toLowerCase()) ? n.color.toLowerCase() : nearestColor(n.color) ?? SHAPE_DEFS[n.shape].color;
  return { shape: n.shape, face: n.face, color };
}

export function avatarFor(t: AgentTemplate): Avatar {
  return safeAvatar(templateAvatar(t.name, t.category, SHAPES, FACE_IDS));
}

/** Give a new agent the soul, skills and starting memories that come with its template. */
async function applyKit(agentId: string, kit: TemplateKit | undefined): Promise<void> {
  if (!kit) return;
  try {
    await api.setSoul(agentId, kit.soul);
  } catch {
    /* the agent keeps no soul yet */
  }
  for (const s of kit.skills) {
    try {
      await api.saveSkill(s.name, s.description, s.content);
    } catch {
      /* a skill that can't be saved is skipped */
    }
  }
  for (const m of kit.memories) {
    try {
      await api.addMemory("agent", m, agentId);
    } catch {
      /* skipped */
    }
  }
}

export async function createAgentFromTemplate(t: AgentTemplate, opts: { greet?: boolean } = {}): Promise<Agent> {
  const agent = await api.createAgent({ name: t.name, avatar: avatarFor(t), instructions: t.instructions, provider_id: await defaultProvider(), model: null });
  await applyKit(agent.id, TEMPLATE_KITS[t.key]);
  if (opts.greet !== false) await greetNewAgent(agent, true);
  return agent;
}

/** Create a group's agents (the ones the user doesn't have yet) and the group itself. */
export async function createGroupFromTemplate(g: GroupTemplate): Promise<string> {
  const existing = Object.values(useStore.getState().agents);
  const ids: string[] = [];
  for (const key of g.members) {
    const t = AGENT_TEMPLATES.find((x) => x.key === key);
    if (!t) continue;
    const have = existing.find((a) => a.name.toLowerCase() === t.name.toLowerCase());
    ids.push(have ? have.id : (await createAgentFromTemplate(t, { greet: false })).id);
  }
  const group = await api.createGroup(g.name, ids, g.kind ?? "chat");
  // A coding team needs its repository: ask for the folder right away (it can also be done later).
  if (g.kind === "coding") {
    const dir = await pickFolder();
    if (dir) await api.codingSetRepo(group.id, dir, true).catch((e) => useStore.getState().showToast(String(e)));
  }
  return group.id;
}

const slug = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60);

/** What an imported Grok Bot becomes: an agent with its instructions, memories and routines noted, plus its skills. */
export async function createAgentFromGrok(t: GrokTemplate): Promise<{ agent: Agent; skills: number; memories: number }> {
  const parts = [t.instructions || t.description];
  if (t.routines.length) parts.push("## Routines it had\n" + t.routines.map((r) => `- ${r.name}: ${r.summary}`).join("\n") + "\nSet them up as automations if the user wants them.");
  if (t.integrations.length) parts.push(`## Apps it used\n${t.integrations.join(", ")}. Connect the equivalents in Settings → Apps if you need them.`);
  let h = 0;
  for (const c of t.name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  const shape = SHAPE_ALIASES[t.shape.toLowerCase()] ?? (SHAPE_DEFS[t.shape.toLowerCase()] ? t.shape.toLowerCase() : SHAPES[h % SHAPES.length]);
  const avatar = safeAvatar({ shape, face: FACE_IDS[(h >> 3) % FACE_IDS.length], color: nearestColor(t.color) ?? COLORS[h % COLORS.length] });
  const agent = await api.createAgent({ name: t.name, avatar, instructions: parts.join("\n\n"), provider_id: await defaultProvider(), model: null });
  // The soul: who the bot is. Grok bots have no separate field, so it comes from their description.
  const soul = [t.description && `${t.name}: ${t.description}`, t.creator && `Made by ${t.creator}.`].filter(Boolean).join("\n");
  if (soul) {
    try {
      await api.setSoul(agent.id, soul);
    } catch {
      /* no soul yet */
    }
  }
  let skills = 0;
  for (const s of t.skills) {
    try {
      await api.saveSkill(slug(s.name) || "imported-skill", s.description || s.name, s.content || s.description);
      skills++;
    } catch {
      /* a skill that can't be saved is skipped */
    }
  }
  let memories = 0;
  for (const m of t.memories) {
    try {
      await api.addMemory("agent", m, agent.id);
      memories++;
    } catch {
      /* skipped */
    }
  }
  await greetNewAgent(agent, true);
  return { agent, skills, memories };
}
