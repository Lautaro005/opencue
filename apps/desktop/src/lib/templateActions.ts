import { api } from "./api";
import { greetNewAgent } from "./greet";
import { isConfigured } from "./models";
import { AGENT_TEMPLATES, templateAvatar, type AgentTemplate, type GroupTemplate } from "./templates";
import { useStore } from "./store";
import { FACE_IDS, SHAPES } from "../components/AvatarView";
import type { Agent, Avatar, GrokTemplate } from "./types";

/** The provider a new agent starts with: the default one, else the first that is set up. */
async function defaultProvider(): Promise<string | null> {
  const { providers, settings } = useStore.getState();
  const catalog = await api.providerCatalog();
  const usable = providers.filter((p) => isConfigured(p, catalog.find((x) => x.kind === p.kind)));
  return settings.default_provider && usable.some((p) => p.id === settings.default_provider) ? settings.default_provider : usable[0]?.id ?? null;
}

export function avatarFor(t: AgentTemplate): Avatar {
  return templateAvatar(t.name, t.category, SHAPES, FACE_IDS);
}

export async function createAgentFromTemplate(t: AgentTemplate, opts: { greet?: boolean } = {}): Promise<Agent> {
  const agent = await api.createAgent({ name: t.name, avatar: avatarFor(t), instructions: t.instructions, provider_id: await defaultProvider(), model: null });
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
  const group = await api.createGroup(g.name, ids);
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
  const palette = ["#2f86eb", "#34b27b", "#f5b82e", "#ff7a45", "#f04848", "#f472b6", "#a78bfa", "#5ec2f2"];
  let h = 0;
  for (const c of t.name) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  const avatar: Avatar = { shape: SHAPES[h % SHAPES.length], face: FACE_IDS[(h >> 3) % FACE_IDS.length], color: palette[h % palette.length] };
  const agent = await api.createAgent({ name: t.name, avatar, instructions: parts.join("\n\n"), provider_id: await defaultProvider(), model: null });
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
