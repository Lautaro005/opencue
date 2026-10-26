import { api } from "./api";
import { useStore } from "./store";
import { randomAvatar } from "../components/panel/AvatarPicker";
import type { Agent, Group, Message, ProviderKind } from "./types";

/** The protocol words a teammate starts a line with (see the team prompt in the core). */
export type ProtocolWord = "ACK" | "STATUS" | "BLOCKED" | "DONE";

export function protocolLines(text: string): { word: ProtocolWord; rest: string }[] {
  const out: { word: ProtocolWord; rest: string }[] = [];
  for (const raw of text.split("\n")) {
    const line = raw.trim().replace(/^[*_`>\- ]+/, "");
    const i = line.indexOf(":");
    if (i < 0) continue;
    const word = line.slice(0, i).trim().replace(/[*_`]+$/, "").toUpperCase();
    if (word === "ACK" || word === "STATUS" || word === "BLOCKED" || word === "DONE") out.push({ word, rest: line.slice(i + 1).replace(/^[*_`\s]+/, "").trim() });
  }
  return out;
}

export type SessionState = "working" | "pending" | "blocked" | "done" | "idle";

export interface Session {
  agent: Agent;
  state: SessionState;
  /** The last protocol line this agent wrote, if any. */
  last: { word: ProtocolWord; rest: string } | null;
  isLead: boolean;
}

/**
 * Where each teammate stands, read from the chat: busy now, waiting on the user (an approval or a
 * question), or the last thing they said with a protocol word (BLOCKED, DONE, otherwise idle).
 */
export function sessionsOf(group: Group, agents: Record<string, Agent>, messages: Message[], working: string[], waiting: Set<string>): Session[] {
  return group.members
    .map((m) => ({ m, agent: agents[m.agent_id] }))
    .filter((x) => x.agent)
    .map(({ m, agent }) => {
      let last: Session["last"] = null;
      for (let i = messages.length - 1; i >= 0 && !last; i--) {
        const msg = messages[i];
        if (msg.role !== "assistant" || msg.agent_id !== agent.id) continue;
        const lines = protocolLines(msg.content);
        if (lines.length) last = lines[lines.length - 1];
      }
      const state: SessionState = working.includes(agent.id) ? "working" : waiting.has(agent.id) ? "pending" : last?.word === "BLOCKED" ? "blocked" : last?.word === "DONE" ? "done" : "idle";
      return { agent, state, last, isLead: m.is_owner };
    });
}

export const CODING_CLIS: { kind: ProviderKind; label: string }[] = [
  { kind: "claude_cli", label: "Claude Code" },
  { kind: "codex_cli", label: "Codex" },
  { kind: "grok_cli", label: "Grok Build" },
  { kind: "antigravity_cli", label: "Gemini (Antigravity)" },
  { kind: "vibe_cli", label: "Mistral Vibe" },
  { kind: "muse_cli", label: "Muse Code" },
];

/** The command-line coding tools an agent can run on (their terminal tool key → provider). */
export const CLI_AGENT_TOOLS: { tool: string; kind: ProviderKind; label: string }[] = [
  { tool: "claude", kind: "claude_cli", label: "Claude Code" },
  { tool: "codex", kind: "codex_cli", label: "Codex" },
  { tool: "grok", kind: "grok_cli", label: "Grok Build" },
  { tool: "agy", kind: "antigravity_cli", label: "Gemini (Antigravity)" },
  { tool: "vibe", kind: "vibe_cli", label: "Mistral Vibe" },
  { tool: "muse", kind: "muse_cli", label: "Muse Code" },
];

/** A new agent that runs on a command-line coding tool (it keeps that tool's own harness). */
export async function createCliAgent(kind: ProviderKind, label: string, instructions?: string): Promise<Agent> {
  const st = useStore.getState();
  let provider = st.providers.find((p) => p.kind === kind);
  if (!provider) {
    provider = await api.saveProvider({ id: "", kind, name: label, base_url: null, default_model: null, enabled: true, has_key: false });
    await st.refreshProviders();
  }
  const taken = new Set(Object.values(useStore.getState().agents).map((a) => a.name.toLowerCase()));
  let name = label;
  for (let n = 2; taken.has(name.toLowerCase()); n++) name = `${label} ${n}`;
  return api.createAgent({
    name,
    avatar: randomAvatar(),
    provider_id: provider.id,
    model: provider.default_model || "default",
    instructions: instructions ?? `You are ${label}, a coding agent. Make real changes in your working folder with your tools, run the project's checks before you report, and say plainly what you did and what you did not verify.`,
  });
}

/** A new agent backed by a command-line coding tool, added to a coding group. */
export async function addCodingAgent(group: Group, kind: ProviderKind, label: string): Promise<Agent> {
  const agent = await createCliAgent(
    kind,
    label,
    `You are ${label}, a coding agent on a team that works on one repository. Make real changes in your working folder with your tools, run the project's checks before you report, and follow the team protocol (ACK / STATUS / BLOCKED / DONE) from your system prompt. Work on a branch of your own and propose changes as a pull request; never push to the default branch.`,
  );
  await api.addGroupMember(group.id, agent.id);
  await useStore.getState().refresh();
  return agent;
}
