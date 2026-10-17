// In-browser stand-in for the Rust core, used by `npm run dev` outside Tauri so the UI can be
// developed and previewed. Data lives in memory only.

import type { Agent, CoreEvent, Group, McpServerConfig, Message, ProviderConfig, ProviderInfo } from "./types";

const listeners = new Set<(ev: CoreEvent) => void>();
const emit = (ev: CoreEvent) => listeners.forEach((l) => l(ev));
const id = () => Math.random().toString(36).slice(2, 12);
const ago = (min: number) => new Date(Date.now() - min * 60_000).toISOString();

const defaults = {
  approval_mode: "ask" as const,
  web_access: true,
  mcp_all: true,
  mcp_servers: [],
  use_global_memory: true,
  auto_memory: false,
  temperature: null,
};

const providers: ProviderConfig[] = [
  { id: "p-ollama", kind: "ollama", name: "Ollama (local)", base_url: null, default_model: "llama3.2", enabled: true, has_key: false },
  { id: "p-anthropic", kind: "anthropic", name: "Anthropic (Claude)", base_url: null, default_model: "claude-sonnet-5-5", enabled: true, has_key: true },
];

const agents: Agent[] = [
  { id: "a1", name: "Nova", avatar: { emoji: "🦊", color: "#ff8a65", shape: "circle" }, provider_id: "p-anthropic", model: "claude-sonnet-5-5", settings: { ...defaults }, created_at: ago(5000), updated_at: ago(10) },
  { id: "a2", name: "Atlas", avatar: { emoji: "🪐", color: "#7c8cff", shape: "circle" }, provider_id: "p-ollama", model: "llama3.2", settings: { ...defaults }, created_at: ago(4000), updated_at: ago(10) },
  { id: "a3", name: "Chef Lu", avatar: { emoji: "🍋", color: "#ffb74d", shape: "squircle" }, provider_id: "p-ollama", model: null, settings: { ...defaults }, created_at: ago(3000), updated_at: ago(10) },
];

const groups: Group[] = [
  { id: "g1", name: "Weekend trip", max_auto_turns: 6, created_at: ago(2000), members: [{ agent_id: "a1", is_owner: true }, { agent_id: "a2", is_owner: false }] },
];

const instructions: Record<string, string> = { a1: "You are my travel and admin assistant. Keep answers short." };
const memory: Record<string, string[]> = { a1: ["Prefers window seats", "Lives in Buenos Aires"], global: ["User's name is Lauti"] };

const msg = (thread_id: string, role: Message["role"], content: string, minAgo: number, agent_id: string | null = null, extra: Partial<Message> = {}): Message => ({
  id: id(), thread_id, role, agent_id, content, tool_calls: null, tool_call_id: null, tool_name: null, status: "done", input_tokens: 0, output_tokens: 0, created_at: ago(minAgo), ...extra,
});

const messages: Message[] = [
  msg("a1", "user", "Can you find me a quiet hotel in Montevideo for next weekend?", 50),
  msg("a1", "assistant", "", 49, "a1", { tool_calls: [{ id: "t1", name: "web_search", arguments: { query: "quiet boutique hotel Montevideo Ciudad Vieja" } }] }),
  msg("a1", "tool", "1. [Hotel Casa Sarandí](https://example.com)\n   Boutique guesthouse in Ciudad Vieja…", 49, "a1", { tool_call_id: "t1", tool_name: "web_search" }),
  msg("a1", "assistant", "Here are two quiet options:\n\n1. **Casa Sarandí** — boutique, Ciudad Vieja, ~US$120/night\n2. **Alma Histórica** — rooftop, very calm street\n\nWant me to check availability for Friday–Sunday?", 48, "a1"),
  msg("a1", "event", "Received an email: “Your booking request at Casa Sarandí”", 20),
  msg("a2", "user", "Summarize today's tech news", 300),
  msg("a2", "assistant", "Three things stood out today: a new open-weights model release, a major browser update, and a chip export rule change.", 299, "a2"),
  msg("g1", "user", "@Nova @Atlas plan a 2-day itinerary", 30),
  msg("g1", "assistant", "Day 1: Ciudad Vieja walk, Mercado del Puerto lunch, sunset on the Rambla. @Atlas can you add a museum?", 29, "a1"),
  msg("g1", "assistant", "Add Museo Torres García on Day 2 morning, then Parque Rodó.", 28, "a2"),
];

const catalog: ProviderInfo[] = [
  { kind: "openai", label: "OpenAI (ChatGPT)", api: "openai", default_base_url: "https://api.openai.com/v1", default_model: "gpt-5", requires_key: true, supports_tools: true, note: "API key from platform.openai.com" },
  { kind: "anthropic", label: "Anthropic (Claude)", api: "anthropic", default_base_url: "https://api.anthropic.com", default_model: "claude-sonnet-5-5", requires_key: true, supports_tools: true, note: "API key from console.anthropic.com" },
  { kind: "xai", label: "xAI (Grok)", api: "openai", default_base_url: "https://api.x.ai/v1", default_model: "grok-4", requires_key: true, supports_tools: true, note: "API key from console.x.ai" },
  { kind: "openrouter", label: "OpenRouter", api: "openai", default_base_url: "https://openrouter.ai/api/v1", default_model: "openrouter/auto", requires_key: true, supports_tools: true, note: "Hundreds of models behind one key" },
  { kind: "cheapestinference", label: "CheapestInference", api: "openai", default_base_url: "https://api.cheapestinference.com/v1", default_model: null, requires_key: true, supports_tools: true, note: "Flat-rate open models" },
  { kind: "ollama", label: "Ollama (local)", api: "openai", default_base_url: "http://localhost:11434/v1", default_model: "llama3.2", requires_key: false, supports_tools: true, note: "Runs models on this machine" },
  { kind: "ollama_cloud", label: "Ollama Cloud", api: "openai", default_base_url: "https://ollama.com/v1", default_model: null, requires_key: true, supports_tools: true, note: "API key from ollama.com" },
  { kind: "lmstudio", label: "LM Studio", api: "openai", default_base_url: "http://localhost:1234/v1", default_model: null, requires_key: false, supports_tools: true, note: "Start the local server first" },
  { kind: "openai_compatible", label: "Custom (OpenAI-compatible)", api: "openai", default_base_url: null, default_model: null, requires_key: false, supports_tools: true, note: "Any /v1/chat/completions server" },
  { kind: "codex_cli", label: "ChatGPT via Codex CLI", api: "cli", default_base_url: null, default_model: null, requires_key: false, supports_tools: false, note: "Uses your installed codex CLI login" },
  { kind: "claude_cli", label: "Claude via Claude Code", api: "cli", default_base_url: null, default_model: null, requires_key: false, supports_tools: false, note: "Uses your installed claude CLI login" },
];

const mcp: McpServerConfig[] = [
  { id: "m1", name: "filesystem", transport: "stdio", command: "npx", args: ["-y", "@modelcontextprotocol/server-filesystem", "~/Documents"], env: {}, url: null, headers: {}, enabled: true },
];
const settings: Record<string, string> = {};
const busy = new Set<string>();

function threads() {
  const last = (tid: string) => [...messages].reverse().find((m) => m.thread_id === tid && m.role !== "tool" && m.content);
  const rows = [
    ...agents.map((a) => ({ kind: "agent" as const, id: a.id, name: a.name, avatars: [a.avatar] })),
    ...groups.map((g) => ({ kind: "group" as const, id: g.id, name: g.name, avatars: g.members.map((m) => agents.find((a) => a.id === m.agent_id)!.avatar).filter(Boolean) })),
  ].map((r) => ({ ...r, last_message: last(r.id)?.content.slice(0, 120) ?? null, last_at: last(r.id)?.created_at ?? null, busy: busy.has(r.id) }));
  return rows.sort((a, b) => (b.last_at ?? "").localeCompare(a.last_at ?? ""));
}

async function simulateReply(threadId: string, text: string) {
  const g = groups.find((x) => x.id === threadId);
  const agentId = g ? g.members[0].agent_id : threadId;
  busy.add(threadId);
  emit({ type: "threads_changed" });
  emit({ type: "busy", thread_id: threadId, agent_id: agentId, busy: true });
  const m = msg(threadId, "assistant", "", 0, agentId, { status: "streaming" });
  messages.push(m);
  emit({ type: "message", message: { ...m } });
  const reply = `This is a preview build without the Rust core, so I can't really think yet — but you said: “${text}”. Run \`npm run tauri dev\` to talk to a real model.`;
  for (const word of reply.split(/(?<= )/)) {
    await new Promise((r) => setTimeout(r, 25));
    if (!busy.has(threadId)) break;
    m.content += word;
    emit({ type: "delta", thread_id: threadId, message_id: m.id, agent_id: agentId, text: word });
  }
  m.status = "done";
  emit({ type: "message", message: { ...m } });
  busy.delete(threadId);
  emit({ type: "busy", thread_id: threadId, agent_id: agentId, busy: false });
  emit({ type: "threads_changed" });
}

type Args = Record<string, any>;
const handlers: Record<string, (a: Args) => unknown> = {
  app_info: () => ({ version: "0.1.0-preview", platform: "browser", data_dir: "(in-memory)", space_supported: false }),
  list_threads: threads,
  list_agents: () => agents,
  get_agent: ({ id }) => agents.find((a) => a.id === id),
  create_agent: ({ agent }) => {
    const a: Agent = { id: id(), name: agent.name, avatar: agent.avatar ?? { emoji: "🐙", color: "#4db6ac", shape: "circle" }, provider_id: agent.provider_id ?? providers[0].id, model: agent.model ?? null, settings: agent.settings ?? { ...defaults }, created_at: ago(0), updated_at: ago(0) };
    agents.push(a);
    instructions[a.id] = agent.instructions ?? "";
    emit({ type: "threads_changed" });
    return a;
  },
  update_agent: ({ id, patch }) => {
    const a = agents.find((x) => x.id === id)!;
    Object.assign(a, Object.fromEntries(Object.entries(patch).filter(([, v]) => v !== undefined)));
    emit({ type: "threads_changed" });
    return a;
  },
  delete_agent: ({ id }) => {
    agents.splice(agents.findIndex((a) => a.id === id), 1);
    emit({ type: "threads_changed" });
  },
  get_instructions: ({ id }) => instructions[id] ?? "",
  set_instructions: ({ id, text }) => void (instructions[id] = text),
  list_memory: ({ scope, agentId }) => (memory[scope === "global" ? "global" : agentId] ?? []).map((text, i) => ({ id: i + 1, text })),
  add_memory: ({ scope, agentId, text }) => {
    const k = scope === "global" ? "global" : agentId;
    (memory[k] ??= []).push(text);
    return { id: memory[k].length, text };
  },
  delete_memory: ({ scope, agentId, id }) => void memory[scope === "global" ? "global" : agentId]?.splice(id - 1, 1),
  get_memory_raw: ({ scope, agentId }) => "# Memory\n\n" + (memory[scope === "global" ? "global" : agentId] ?? []).map((t) => `- ${t}`).join("\n"),
  set_memory_raw: ({ scope, agentId, text }) => void (memory[scope === "global" ? "global" : agentId] = String(text).split("\n").filter((l) => l.startsWith("- ")).map((l) => l.slice(2))),
  list_groups: () => groups,
  get_group: ({ id }) => groups.find((g) => g.id === id) ?? null,
  create_group: ({ name, agentIds }) => {
    const g: Group = { id: id(), name, max_auto_turns: 6, created_at: ago(0), members: agentIds.map((a: string, i: number) => ({ agent_id: a, is_owner: i === 0 })) };
    groups.push(g);
    emit({ type: "threads_changed" });
    return g;
  },
  update_group: ({ id, name, maxAutoTurns }) => {
    const g = groups.find((x) => x.id === id)!;
    if (name) g.name = name;
    if (maxAutoTurns != null) g.max_auto_turns = maxAutoTurns;
    emit({ type: "threads_changed" });
  },
  delete_group: ({ id }) => {
    groups.splice(groups.findIndex((g) => g.id === id), 1);
    emit({ type: "threads_changed" });
  },
  add_group_member: ({ groupId, agentId }) => {
    groups.find((g) => g.id === groupId)!.members.push({ agent_id: agentId, is_owner: false });
    emit({ type: "threads_changed" });
  },
  remove_group_member: ({ groupId, agentId }) => {
    const g = groups.find((x) => x.id === groupId)!;
    g.members = g.members.filter((m) => m.agent_id !== agentId);
    emit({ type: "threads_changed" });
  },
  get_messages: ({ threadId }) => messages.filter((m) => m.thread_id === threadId),
  send_message: ({ threadId, text }) => {
    const m = msg(threadId, "user", text, 0);
    messages.push(m);
    emit({ type: "message", message: m });
    void simulateReply(threadId, text);
    return m;
  },
  stop_thread: ({ threadId }) => void busy.delete(threadId),
  clear_thread: ({ threadId }) => {
    for (let i = messages.length - 1; i >= 0; i--) if (messages[i].thread_id === threadId) messages.splice(i, 1);
    emit({ type: "threads_changed" });
  },
  resolve_approval: () => true,
  provider_catalog: () => catalog,
  list_providers: () => providers,
  save_provider: ({ provider, apiKey }) => {
    const p = { ...provider, id: provider.id || id(), has_key: provider.has_key || !!apiKey };
    const i = providers.findIndex((x) => x.id === p.id);
    if (i >= 0) providers[i] = p;
    else providers.push(p);
    return p;
  },
  delete_provider: ({ id }) => void providers.splice(providers.findIndex((p) => p.id === id), 1),
  list_models: ({ providerId }) => (providerId === "p-anthropic" ? ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5"] : ["llama3.2", "qwen3:8b", "gpt-oss:20b"]),
  list_mcp_servers: () => mcp,
  mcp_status: () => mcp.map((s) => ({ id: s.id, name: s.name, state: "ready", error: null, tools: ["read_file", "write_file", "list_directory"] })),
  save_mcp_server: ({ server }) => {
    const s = { ...server, id: server.id || id() };
    const i = mcp.findIndex((x) => x.id === s.id);
    if (i >= 0) mcp[i] = s;
    else mcp.push(s);
    emit({ type: "mcp_changed" });
    return s;
  },
  delete_mcp_server: ({ id }) => {
    mcp.splice(mcp.findIndex((s) => s.id === id), 1);
    emit({ type: "mcp_changed" });
  },
  import_mcp_json: () => [],
  import_mcp_file: () => [],
  web_search: ({ query }) => [{ title: `Results for “${query}” appear in the desktop app`, url: "https://duckduckgo.com/?q=" + encodeURIComponent(query), snippet: "The browser preview has no network backend.", source: "preview" }],
  get_settings: () => settings,
  set_setting: ({ key, value }) => void (settings[key] = value),
  usage: () => agents.map((a, i) => ({ agent_id: a.id, agent_name: a.name, messages: 12 - i * 3, input_tokens: 48210 - i * 9000, output_tokens: 9120 - i * 2000 })),
};

export async function invoke<T>(cmd: string, args: Args = {}): Promise<T> {
  const h = handlers[cmd];
  if (!h) throw new Error(`mock: unknown command ${cmd}`);
  return structuredClone(await h(args)) as T;
}

export async function listen(cb: (ev: CoreEvent) => void) {
  listeners.add(cb);
  return () => listeners.delete(cb);
}
