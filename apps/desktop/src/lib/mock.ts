// In-browser stand-in for the Rust core, used by `npm run dev` outside Tauri so the UI can be
// developed and previewed. Data lives in memory only.

import type { Agent, Automation, CoreEvent, EmailAccount, Group, McpServerConfig, Message, ProviderConfig, ProviderInfo } from "./types";

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
  computer: "none" as const,
  skills: true,
  composio: false,
  voice_id: null,
};

const providers: ProviderConfig[] = [
  { id: "p-ollama", kind: "ollama", name: "Ollama (local)", base_url: null, default_model: "llama3.2", enabled: true, has_key: false },
  { id: "p-anthropic", kind: "anthropic", name: "Anthropic (Claude)", base_url: null, default_model: "claude-sonnet-5-5", enabled: true, has_key: true },
];

const agent = (id: string, name: string, shape: string, face: string, color: string, extra: Partial<Agent["settings"]> = {}): Agent => ({
  id, name, avatar: { face, color, shape }, provider_id: "p-anthropic", model: "claude-sonnet-5-5", settings: { ...defaults, ...extra }, created_at: ago(5000), updated_at: ago(10),
});

const agents: Agent[] = [
  agent("a1", "Asistente", "square", "googly", "#8e8e93", { computer: "docker" }),
  agent("a2", "Recordatorios", "drop", "wink", "#f6ad2b"),
  agent("a3", "IA Research", "star", "googly", "#f7c32f"),
  agent("a4", "Asistente Dev", "bubble", "curious", "#2f86eb", { computer: "host" }),
  agent("a5", "Finance", "bot", "smile", "#2f86eb"),
  agent("a6", "Research Assistant", "heart", "dots", "#f04848"),
];

const groups: Group[] = [
  { id: "g1", name: "Asistentes", max_auto_turns: 6, created_at: ago(20000), members: [{ agent_id: "a1", is_owner: true }, { agent_id: "a2", is_owner: false }, { agent_id: "a6", is_owner: false }] },
];

const instructions: Record<string, string> = { a1: "You are my travel and admin assistant. Keep answers short." };
const memory: Record<string, string[]> = { a1: ["Prefers window seats", "Lives in Buenos Aires"], global: ["User's name is Lauti"] };

const msg = (thread_id: string, role: Message["role"], content: string, minAgo: number, agent_id: string | null = null, extra: Partial<Message> = {}): Message => ({
  id: id(), thread_id, role, agent_id, content, tool_calls: null, tool_call_id: null, tool_name: null, status: "done", input_tokens: 0, output_tokens: 0, created_at: ago(minAgo), ...extra,
});

const messages: Message[] = [
  msg("a1", "assistant", "Envié la consulta al Juzgado 9 por la audiencia del 9 de octubre de 13:00 a 13:30, preguntando si pueden asistir vos y tus dos compañeros y qué requisitos hay.", 400, "a1"),
  msg("a1", "user", "manda un recordatorio a los ultimos 4 que mandaste", 300),
  msg("a1", "assistant", "Voy a revisar cuáles fueron los últimos cuatro correos y les mando un recordatorio.", 299, "a1"),
  msg("a1", "assistant", "", 298, "a1", { tool_calls: [{ id: "t1", name: "read_emails", arguments: { limit: 4 } }] }),
  msg("a1", "tool", "#41 Juzgado 15 …\n#40 Juzgado 2 …", 298, "a1", { tool_call_id: "t1", tool_name: "read_emails" }),
  msg("a1", "assistant", "Listo, mandé recordatorios a los juzgados 15, 2, 1 y 9. Al 15 le remarqué que la audiencia es hoy; a los otros tres, las audiencias del 9 de octubre.", 297, "a1"),
  msg("a1", "event", "Received an email:\nFrom: Juzgado 9\nSubject: Audiencia 9/10", 200),
  msg("a1", "assistant", "El Juzgado 9 respondió que el juicio del 9 de octubre no se realizará.", 199, "a1"),
  msg("a1", "user", "prueba usar el conector de trello y dime si funciona", 130),
  msg("a1", "assistant", "Dale, lo pruebo", 129, "a1"),
  msg("a1", "assistant", "No pude confirmar si funciona: el acceso a Trello falló antes de mostrar tu espacio. No hice ningún cambio. Si querés, lo intento de nuevo más tarde", 125, "a1"),
  msg("a2", "user", "Dale, pruebo", 200),
  msg("a2", "assistant", "Todavía falla antes de leer Trello; el entorno desde el que accedo sigue sin estar disponible, así que no pude completar la prueba", 152, "a2"),
  msg("a3", "assistant", "Sí, probaría esta idea antes de escribir el paper: armá un benchmark chico primero.", 210, "a3"),
  msg("a4", "assistant", "Ya están las notas de hoy en tu carpeta de proyectos.", 211, "a4"),
  msg("a5", "assistant", "Mi hipótesis para dos años sería mantener el fondo de emergencia en dólares.", 1500, "a5"),
  msg("a6", "assistant", "El correo no se envió: se canceló la aprobación.", 22000, "a6"),
  msg("g1", "event", "Asistente, Recordatorios, and Research Assistant joined the chat", 25000),
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

const skills = [
  { name: "weekly-report", description: "Write the Friday status report from this week's notes and emails.", body: "1. Read notes\n2. Summarize", path: "skills/weekly-report/SKILL.md" },
];
const automations: Automation[] = [
  { id: "au1", agent_id: "a2", name: "Vencimientos de la semana", trigger: { kind: "schedule", cron: "0 9 * * 1" }, prompt: "Revisá Trello y avisame qué vence esta semana.", enabled: true, last_run_at: ago(60 * 24 * 4), last_status: "ok", created_at: ago(9000), next_run_at: ago(-60 * 24 * 2) },
];
const emails: Record<string, EmailAccount> = {
  a2: { agent_id: "a2", kind: "imap", address: "recordatorios@example.com", display_name: "Recordatorios", imap_host: "imap.example.com", imap_port: 993, smtp_host: "smtp.example.com", smtp_port: 465, username: null, last_seen_uid: 0, has_password: true },
};
const computer = { docker_image: "ubuntu:24.04", space_url: "", space_template: "", has_space_token: false, os: "linux", docker_available: true, wsl_available: false, webhook_port: 7777 };
const voice = { configured: false, voice_id: "", auto_speak: true };

type Args = Record<string, any>;
const handlers: Record<string, (a: Args) => unknown> = {
  app_info: () => ({ version: "0.2.0-preview", platform: "browser", data_dir: "(in-memory)", space_supported: false }),
  list_threads: threads,
  list_agents: () => agents,
  get_agent: ({ id }) => agents.find((a) => a.id === id),
  create_agent: ({ agent }) => {
    const a: Agent = { id: id(), name: agent.name, avatar: agent.avatar ?? { face: "smile", color: "#2f86eb", shape: "bubble" }, provider_id: agent.provider_id ?? providers[0].id, model: agent.model ?? null, settings: agent.settings ?? { ...defaults }, created_at: ago(0), updated_at: ago(0) };
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
  list_skills: () => skills.map(({ body: _b, ...k }) => k),
  get_skill: ({ name }) => skills.find((k) => k.name === name) ?? null,
  save_skill: ({ name, description, body }) => {
    const k = { name, description, body, path: `skills/${name}/SKILL.md` };
    const i = skills.findIndex((x) => x.name === name);
    if (i >= 0) skills[i] = k;
    else skills.push(k);
    return k;
  },
  delete_skill: ({ name }) => void skills.splice(skills.findIndex((k) => k.name === name), 1),
  import_skill: ({ url }) => {
    const name = String(url).split("/").filter(Boolean).slice(-2, -1)[0] ?? "imported";
    const k = { name, description: "Imported from " + url, body: "# " + name, path: `skills/${name}/SKILL.md` };
    skills.push(k);
    return k;
  },
  list_automations: ({ agentId }) => automations.filter((a) => !agentId || a.agent_id === agentId),
  save_automation: ({ automation }) => {
    const a: Automation = { last_run_at: null, last_status: null, created_at: ago(0), next_run_at: automation.trigger.kind === "schedule" ? ago(-60 * 12) : null, ...automation, id: automation.id || id() };
    const i = automations.findIndex((x) => x.id === a.id);
    if (i >= 0) automations[i] = a;
    else automations.push(a);
    emit({ type: "automations_changed" });
    return a;
  },
  delete_automation: ({ id }) => {
    automations.splice(automations.findIndex((a) => a.id === id), 1);
    emit({ type: "automations_changed" });
  },
  run_automation: () => undefined,
  new_webhook_token: () => id() + id() + id(),
  get_email_account: ({ agentId }) => emails[agentId] ?? null,
  save_email_account: ({ account, password }) => (emails[account.agent_id] = { ...account, has_password: account.has_password || !!password }),
  delete_email_account: ({ agentId }) => void delete emails[agentId],
  test_email: () => undefined,
  composio_status: () => ({ configured: false, user_id: "", accounts: [], error: null }),
  save_composio: ({ userId }) => ({ configured: true, user_id: userId, accounts: [{ id: "ca_1", toolkit: "gmail", status: "ACTIVE" }], error: null }),
  computer_settings: () => computer,
  save_computer_settings: ({ settings }) => Object.assign(computer, settings),
  test_space: () => 0,
  computer_exec: ({ agentId, command }) => {
    const out = `$ ${command}\n(preview) commands run in the desktop app\n[exit 0]\n`;
    emit({ type: "computer_output", agent_id: agentId, text: out, stderr: false });
    return { exit_code: 0, output: out, timed_out: false };
  },
  computer_list: () => [
    { name: "notas", path: "notas", is_dir: true, size: 0 },
    { name: "audiencias.csv", path: "audiencias.csv", is_dir: false, size: 2310 },
    { name: "README.md", path: "README.md", is_dir: false, size: 512 },
  ],
  computer_read: ({ path }) => `# ${path}\n\n(preview file)`,
  computer_log: () => "$ ls\nnotas  audiencias.csv  README.md\n[exit 0]\n",
  voice_settings: () => voice,
  save_voice_settings: ({ settings, apiKey }) => Object.assign(voice, settings, { configured: voice.configured || !!apiKey }),
  list_voices: () => [{ id: "21m00Tcm4TlvDq8ikWAM", name: "Rachel" }, { id: "pNInz6obpgDQGcFmaJgB", name: "Adam" }],
  transcribe: () => "Hola, ¿qué tengo en la agenda hoy?",
  speak: () => "",
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
