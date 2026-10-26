// In-browser stand-in for the Rust core, used by `npm run dev` outside Tauri so the UI can be
// developed and previewed. Data lives in memory only.

import dashboardHtml from "../../../../crates/opencue-core/src/dashboard_default.html?raw";
import type { Agent, Artifact, ArtifactKind, Attachment, Automation, CoreEvent, EmailAccount, Group, McpServerConfig, Message, ProviderConfig, ProviderInfo } from "./types";

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
  auto_memory: true,
  auto_profile: true,
  temperature: null,
  computer: "none" as const,
  skills: true,
  browser: false,
  catalog: [] as string[],
  voice_id: null,
  cli_tools: true,
  vmpal: [] as string[],
  working_dir: null as string | null,
  vm: { desktop: null, memory_mb: null, cpus: null, data_dir: null, runtime: null } as { desktop: boolean | null; memory_mb: number | null; cpus: number | null; data_dir: string | null; runtime: "docker" | "podman" | null },
};

/** Files "uploaded" in the preview, kept as data URLs. */
const blobs: Record<string, { name: string; mime: string; url: string }> = {};
const mimeOf = (name: string) => {
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  return ({ png: "image/png", jpg: "image/jpeg", jpeg: "image/jpeg", gif: "image/gif", webp: "image/webp", pdf: "application/pdf", csv: "text/csv", md: "text/markdown", txt: "text/plain", json: "application/json" } as Record<string, string>)[ext] ?? "application/octet-stream";
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

const souls: Record<string, string> = { a1: "Calm, precise and a little dry. Never pads an answer." };
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
  msg("a1", "user", "cuanto es 4*4/2", 20),
  msg("a1", "assistant", "4 × 4 ÷ 2 = **8**", 19, "a1"),
  msg("a1", "assistant", "Estas son las noticias de hoy:\n\n- **Protestas**: hubo cientos de detenciones. [AP](https://apnews.com/article/protests)\n- **Macron** condenó la violencia, según [Le Monde](https://www.lemonde.fr/politique/article.html).\n- Más en https://www.reuters.com/world/europe/ y en [esta nota muy larga de un medio con un nombre larguísimo](https://example.com/a/very/long/path).", 18, "a1"),
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
  { kind: "gemini", label: "Google (Gemini API)", api: "openai", default_base_url: "https://generativelanguage.googleapis.com/v1beta/openai", default_model: "gemini-3-flash-preview", requires_key: true, supports_tools: true, note: "API key from aistudio.google.com (Gemini's OpenAI-compatible endpoint)" },
  { kind: "muse", label: "Meta (Muse API)", api: "openai", default_base_url: "https://api.meta.ai/v1", default_model: "muse-spark-1.3", requires_key: true, supports_tools: true, note: "API key from dev.meta.ai (Muse Spark)" },
  { kind: "mistral_api", label: "Mistral API", api: "openai", default_base_url: "https://api.mistral.ai/v1", default_model: "mistral-large-latest", requires_key: true, supports_tools: true, note: "API key from console.mistral.ai" },
  { kind: "groq", label: "Groq", api: "openai", default_base_url: "https://api.groq.com/openai/v1", default_model: "llama-3.3-70b-versatile", requires_key: true, supports_tools: true, note: "API key from console.groq.com (fast open models)" },
  { kind: "openrouter", label: "OpenRouter", api: "openai", default_base_url: "https://openrouter.ai/api/v1", default_model: "openrouter/auto", requires_key: true, supports_tools: true, note: "Hundreds of models behind one key" },
  { kind: "cheaperinference", label: "CheaperInference", api: "openai", default_base_url: "https://api.cheaperinference.com/v1", default_model: null, requires_key: true, supports_tools: true, note: "Flat-rate open models" },
  { kind: "ollama", label: "Ollama (local)", api: "openai", default_base_url: "http://localhost:11434/v1", default_model: "llama3.2", requires_key: false, supports_tools: true, note: "Runs models on this machine" },
  { kind: "ollama_cloud", label: "Ollama Cloud", api: "openai", default_base_url: "https://ollama.com/v1", default_model: null, requires_key: true, supports_tools: true, note: "API key from ollama.com" },
  { kind: "lmstudio", label: "LM Studio", api: "openai", default_base_url: "http://localhost:1234/v1", default_model: null, requires_key: false, supports_tools: true, note: "Start the local server first" },
  { kind: "openai_compatible", label: "Custom (OpenAI-compatible)", api: "openai", default_base_url: null, default_model: null, requires_key: false, supports_tools: true, note: "Any /v1/chat/completions server" },
  { kind: "codex_cli", label: "ChatGPT via Codex CLI", api: "cli", default_base_url: null, default_model: null, requires_key: false, supports_tools: false, note: "Uses your installed codex CLI login" },
  { kind: "grok_cli", label: "Grok via Grok Build CLI", api: "cli", default_base_url: null, default_model: null, requires_key: false, supports_tools: false, note: "Uses your installed grok CLI login" },
  { kind: "muse_cli", label: "Muse via Muse Code CLI", api: "cli", default_base_url: null, default_model: null, requires_key: false, supports_tools: false, note: "Uses your installed muse CLI login" },
  { kind: "vibe_cli", label: "Mistral via Vibe CLI", api: "cli", default_base_url: null, default_model: null, requires_key: false, supports_tools: false, note: "Uses your installed vibe CLI login" },
  { kind: "antigravity_cli", label: "Gemini via Antigravity CLI", api: "cli", default_base_url: null, default_model: null, requires_key: false, supports_tools: false, note: "Uses your installed agy CLI login" },
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

const artifacts: Record<string, Artifact> = {};

async function simulateReply(threadId: string, text: string) {
  // Preview-only triggers for the cards that need a real agent to show up.
  if (/^\/(ask|permission)\b/.test(text)) {
    const permission = text.startsWith("/permission");
    emit({
      type: "question_requested",
      question: {
        id: id(),
        thread_id: threadId,
        agent_id: threadId,
        kind: permission ? "permission" : "question",
        title: "",
        body: permission ? "Run COMPOSIO_SEARCH_TOOLS on Trello to find the cards in the brain column of cue-agents." : "Which Trello board should I read, cue-agents or personal?",
        options: permission ? [] : ["cue-agents", "personal"],
      },
    });
    return;
  }
  // Preview-only: "/artifact html|xlsx|pptx|mermaid|docx|code" shows what an agent-made file looks like.
  const art = text.match(/^\/artifact\s*(\w+)?/);
  if (art) {
    const kind = (art[1] ?? "html") as ArtifactKind;
    const samples: Record<string, [string, string]> = {
      html: ["Habit tracker", "<!doctype html><html><body style=\"font-family:system-ui;padding:24px\"><h1>Habits</h1><button id=b>+1 day</button> <b id=n>0</b><script>b.onclick=()=>n.textContent=+n.textContent+1</script></body></html>"],
      xlsx: ["Sales table", JSON.stringify({ sheets: [{ name: "Q1", rows: [["Item", "Qty", "Price"], ["Pen", 3, 1.5], ["Book", 10, 12], ["Total", "=SUM(B2:B3)", ""]] }, { name: "Notes", rows: [["Prices in USD"]] }] })],
      pptx: ["Launch plan", "# Launch plan\n---\n# Goals\n- Ship **fast**\n- Keep quality\nNotes: keep it short\n---\n# Next steps\n- Review\n- Release"],
      mermaid: ["Release flow", "flowchart LR\n  A[Idea] --> B{Ready?}\n  B -- yes --> C[Ship]\n  B -- no --> A"],
      docx: ["Quarterly report", "# Quarterly report\n\nRevenue grew **12%**.\n\n- North\n- South\n\n| Region | Sales |\n|---|---|\n| North | 10 |"],
      code: ["Fibonacci", "def fib(n):\n    return n if n < 2 else fib(n - 1) + fib(n - 2)"],
    };
    const [title, source] = samples[kind] ?? samples.html;
    const aid = id();
    const att: Attachment = { id: id(), name: `${title.toLowerCase().replace(/\s+/g, "-")}.${kind}`, mime: "text/plain", size: source.length, artifact: { id: aid, version: 1, kind, title } };
    artifacts[aid] = { id: aid, thread_id: threadId, agent_id: threadId, title, kind, language: kind === "code" ? "python" : null, versions: [{ version: 1, attachment: att, created_at: new Date().toISOString(), source }] };
    const m = msg(threadId, "assistant", "", 0, threadId);
    m.attachments = [att];
    messages.push(m);
    emit({ type: "message", message: { ...m } });
    return;
  }
  if (text.startsWith("/dm")) {
    const other = agents.find((a) => a.id !== threadId);
    if (!other) return;
    const dm = threadId <= other.id ? `dm_${threadId}_${other.id}` : `dm_${other.id}_${threadId}`;
    const call = { id: id(), name: "send_to_agent", arguments: { agent: other.name, message: "Give me three SEO ideas." } };
    const a = msg(threadId, "assistant", "", 0, threadId, { tool_calls: [call] });
    const t = msg(threadId, "tool", "[" + other.name + "]: 1. Long-tail posts. 2. Comparison pages. 3. FAQ schema.", 0, threadId, { tool_call_id: call.id, tool_name: "send_to_agent" });
    messages.push(a, t);
    messages.push(msg(dm, "assistant", "Give me three SEO ideas.", 0, threadId), msg(dm, "assistant", "1. Long-tail posts. 2. Comparison pages. 3. FAQ schema.", 0, other.id));
    emit({ type: "message", message: { ...a } });
    emit({ type: "message", message: { ...t } });
    return;
  }
  const g = groups.find((x) => x.id === threadId);
  const agentId = g ? g.members[0].agent_id : threadId;
  busy.add(threadId);
  emit({ type: "threads_changed" });
  emit({ type: "busy", thread_id: threadId, agent_id: agentId, busy: true });
  const m = msg(threadId, "assistant", "", 0, agentId, { status: "streaming" });
  messages.push(m);
  emit({ type: "message", message: { ...m } });
  const reply = (text.startsWith("/think") ? "<think>The user wants a quick check. I should look at the files first, then answer in one line.</think>" : "") + `This is a preview build without the Rust core, so I can't really think yet — but you said: “${text}”. Run \`npm run tauri dev\` to talk to a real model.`;
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
const catalogApps: { slug: string; name: string; description: string; state: "disconnected" | "reconnect" | "connected"; needs_client: boolean; via_composio: boolean }[] = [
  { slug: "composio", name: "Composio", description: "One sign-in for 500+ apps: Gmail, Slack, Calendar…", state: "connected", needs_client: false, via_composio: false },
  { slug: "gmail", name: "Gmail", description: "Search, read and draft email", state: "connected", needs_client: true, via_composio: true },
  { slug: "googledrive", name: "Google Drive", description: "Search, read and upload files", state: "disconnected", needs_client: true, via_composio: false },
  { slug: "notion", name: "Notion", description: "Search and edit pages and databases", state: "reconnect", needs_client: false, via_composio: false },
  { slug: "trello", name: "Trello", description: "Read and update boards and cards", state: "disconnected", needs_client: false, via_composio: false },
  { slug: "github", name: "GitHub", description: "Repositories, issues and pull requests", state: "disconnected", needs_client: true, via_composio: false },
  { slug: "supabase", name: "Supabase", description: "Databases, auth and storage", state: "disconnected", needs_client: false, via_composio: false },
  { slug: "vercel", name: "Vercel", description: "Projects and deployments", state: "disconnected", needs_client: false, via_composio: false },
  { slug: "outlook", name: "Outlook", description: "Mail and calendar in Microsoft 365", state: "disconnected", needs_client: true, via_composio: false },
];
const computer = { docker_desktop: true, desktop_image: "lscr.io/linuxserver/webtop:ubuntu-xfce", desktop_port: 3000, docker_image: "ubuntu:24.04", space_url: "", space_template: "", has_space_token: false, os: "linux", docker_available: true, podman_available: true, runtime: "docker", shared_vm: false, idle_minutes: 480, wsl_available: false, webhook_port: 7777 };
const dockerState = { installed: true, running: true, image_ready: false, vm: "none" as const };
let vmpalToml = "";
const vmpalMachines = () => [...vmpalToml.matchAll(/^\[mcp_servers\.([^\]\s]+)\]/gm)].map((m) => ({ name: m[1], command: "/Applications/VMPal.app/Contents/Helpers/VMPalMachine.app/Contents/MacOS/VMPalMachine", args: ["--mcp", "E7D-preview"], env: {} }));
const MB = 1_000_000;
const localModels = [
  { id: "parakeet-v3", kind: "stt", label: "Parakeet TDT 0.6B v3 (8-bit)", size: 465 * MB, license: "CC-BY-4.0", source: "nvidia/parakeet-tdt-0.6b-v3", languages: ["en", "es", "fr", "de", "it", "pt"], note: "Fast on any CPU. 25 European languages.", ram_gb: 1.5, installed: false, supported: true },
  { id: "whisper-small", kind: "stt", label: "Whisper small", size: 610 * MB, license: "MIT", source: "openai/whisper-small", languages: ["en", "es", "ja"], note: "99 languages, a good balance.", ram_gb: 1, installed: false, supported: true },
  { id: "kokoro-v1", kind: "tts", label: "Kokoro 82M v1.0 (multilingual)", size: 330 * MB, license: "Apache-2.0", source: "hexgrad/Kokoro-82M", languages: ["en", "es", "fr"], note: "Natural voices in 8 languages.", ram_gb: 1, installed: false, supported: true },
  { id: "sherpa-runtime", kind: "runtime", label: "sherpa-onnx runtime (v2.0.0)", size: 27 * MB, license: "Apache-2.0", source: "github.com/k2-fsa/sherpa-onnx", languages: [], note: "Runs the models.", ram_gb: 0, installed: false, supported: true },
];
const persona = { installed: false, installing: false, running: false, ready: false, has_token: false, lan: false, offload: false, gpu: null as string | null, python: "3.12", url: null as string | null, lan_url: null as string | null, qr_svg: null as string | null, log: "" };
const voice = { configured: false, has_key: false, voice_id: "", auto_speak: true, stt_engine: "auto", tts_engine: "auto", local_stt: "", local_tts: "", local_voice: "" };

type Args = Record<string, any>;
const handlers: Record<string, (a: Args) => unknown> = {
  app_info: () => ({ version: "0.4.2-preview", platform: "browser", data_dir: "(in-memory)", space_supported: false }),
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
  get_soul: ({ id }) => souls[id] ?? "",
  set_soul: ({ id, text }) => void (souls[id] = text),
  test_notification: () => undefined,
  greet_agent: ({ id: agentId, text }) => {
    if (messages.some((m) => m.thread_id === agentId)) return;
    const m: Message = { id: id(), thread_id: agentId, role: "assistant", agent_id: agentId, content: text, tool_calls: null, tool_call_id: null, tool_name: null, status: "done", input_tokens: 0, output_tokens: 0, created_at: ago(0), attachments: [] };
    messages.push(m);
    emit({ type: "message", message: m });
  },
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
  send_message: ({ threadId, text, files }) => {
    const m = msg(threadId, "user", text, 0);
    m.attachments = (files ?? []).map((f: { name: string; data: string }) => {
      const aid = id();
      const mime = mimeOf(f.name);
      blobs[aid] = { name: f.name, mime, url: `data:${mime};base64,${f.data}` };
      return { id: aid, name: f.name, mime, size: Math.round((f.data.length * 3) / 4) };
    });
    messages.push(m);
    emit({ type: "message", message: m });
    void simulateReply(threadId, text);
    return m;
  },
  thread_files: ({ threadId }) =>
    messages
      .filter((m) => m.thread_id === threadId && m.attachments?.length)
      .reverse()
      .flatMap((m) => m.attachments!.map((attachment) => ({ attachment, message_id: m.id, agent_id: m.agent_id, created_at: m.created_at }))),
  read_attachment: ({ id }) => blobs[id]?.url ?? "",
  open_attachment: ({ id }) => void (blobs[id] && window.open(blobs[id].url, "_blank")),
  reveal_attachment: () => undefined,
  save_attachment: () => undefined,
  computer_desktop: () => {
    if (!computer.docker_desktop) throw new Error("turn on the desktop in Settings → Computers first");
    return "about:blank";
  },
  reset_computer: () => undefined,
  stop_computer: () => undefined,
  browser_place: () => undefined,
  browser_hide: () => undefined,
  browser_control: () => undefined,
  browser_state: () => ["", ""],
  browser_clear: () => undefined,
  get_artifact: ({ id }) => artifacts[id],
  list_artifacts: ({ agentId }) => Object.values(artifacts).filter((a) => a.agent_id === agentId),
  dashboard_data: () => ({
    now: new Date().toISOString(),
    user_name: "Lautaro",
    agents: { total: agents.length, running: busy.size, list: agents.map((a) => ({ id: a.id, name: a.name, model: a.model, busy: busy.has(a.id), avatar: a.avatar })) },
    running: [...busy].map((tid) => ({ thread_id: tid, name: agents.find((a) => a.id === tid)?.name ?? "Group", kind: "agent" })),
    automations: {
      total: 2,
      enabled: 1,
      upcoming: [],
      list: [
        { id: "a1", name: "Morning brief", agent_id: agents[0]?.id, agent_name: agents[0]?.name, enabled: true, trigger: "schedule", cron: "0 9 * * 1-5", next_run_at: new Date(Date.now() + 3600e3).toISOString(), last_run_at: null, last_status: null },
        { id: "a2", name: "Inbox triage", agent_id: agents[0]?.id, agent_name: agents[0]?.name, enabled: false, trigger: "email", cron: "", next_run_at: null, last_run_at: null, last_status: null },
      ],
    },
    tasks: [{ agent_id: agents[0]?.id, agent_name: agents[0]?.name, done: 1, total: 3, items: [{ text: "Collect sources", status: "done" }, { text: "Draft the summary", status: "doing" }, { text: "Send to the team", status: "todo" }] }],
    usage: { messages: 42, input_tokens: 120000, output_tokens: 18000, by_agent: agents.map((a, i) => ({ agent_id: a.id, name: a.name, messages: 10 - i, tokens: 60000 - i * 15000 })) },
    recent: messages.filter((m) => m.role === "assistant" && m.content).slice(-4).reverse().map((m) => ({ thread_id: m.thread_id, name: agents.find((a) => a.id === m.thread_id)?.name ?? "Chat", kind: "agent", last_message: m.content.slice(0, 80), last_at: m.created_at })),
  }),
  dashboard_info: () => ({ html: dashboardHtml, is_default: true }),
  reset_dashboard: () => undefined,
  view_base: () => "",
  browser_tabs: () => [],
  browser_tab_new: () => 1,
  browser_tab_select: () => undefined,
  browser_tab_close: () => undefined,
  docker_start_engine: () => undefined,
  stop_all_computers: () => 2,
  computer_running: () => true,
  stop_thread: ({ threadId }) => void busy.delete(threadId),
  regenerate_last: ({ threadId, text }) => {
    const i = messages.map((x) => x.role).lastIndexOf("user");
    if (i < 0) return;
    if (text) messages[i].content = text;
    const keep = messages.filter((x, j) => x.thread_id !== threadId || j <= i);
    messages.length = 0;
    messages.push(...keep);
    void simulateReply(threadId, messages[i].content);
  },
  clear_thread: ({ threadId }) => {
    for (let i = messages.length - 1; i >= 0; i--) if (messages[i].thread_id === threadId) messages.splice(i, 1);
    emit({ type: "threads_changed" });
  },
  resolve_approval: () => true,
  answer_question: () => true,
  pending_questions: () => [],
  docker_setup: () => dockerState,
  docker_prepare: () => {
    dockerState.image_ready = true;
  },
  docker_start_shared: () => {
    (dockerState as { vm: string }).vm = "running";
  },
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
  list_models: ({ providerId }) => (providerId === "p-anthropic" ? ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-sonnet-4-5", "claude-opus-4-1", "claude-haiku-4-5"] : ["llama3.2", "qwen3:8b", "gpt-oss:20b", "gemma3:12b", "mistral-small3.2", "deepseek-r1:14b", "phi4", "llava:13b", "codellama:7b", "nomic-embed-text"]),
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
  search_messages: ({ query }) => messages.filter((m) => (m.role === "user" || m.role === "assistant") && m.content.toLowerCase().includes(String(query).toLowerCase())).slice(-20).reverse().map((m) => ({ message_id: m.id, thread_id: m.thread_id, role: m.role, agent_id: m.agent_id, content: m.content, created_at: m.created_at })),
  cli_limits: () => { const n = Math.floor(Date.now() / 1000); return [{ cli: "claude", name: "Claude Code", plan: null, updated_at: n - 600, note: null, windows: [{ label: "5h", used_percent: 62, resets_at: n + 5400 }, { label: "weekly", used_percent: 23, resets_at: n + 3 * 86400 }] }, { cli: "codex", name: "Codex", plan: "plus", updated_at: n - 7200, note: null, windows: [{ label: "5h", used_percent: 91, resets_at: n + 900 }, { label: "weekly", used_percent: 48, resets_at: n + 5 * 86400 }] }]; },
  messenger_status: () => ({ state: "qr", qr_svg: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 8 8" width="240" height="240"><rect width="8" height="8" fill="#fff"/><path d="M1 1h2v2H1zM5 1h2v2H5zM1 5h2v2H1zM4 4h1v1H4zM6 5h1v2H6z" fill="#000"/></svg>', phone: null, agent_id: null, allow: "", error: null }),
  whatsapp_configure: () => undefined,
  whatsapp_connect: () => undefined,
  whatsapp_disconnect: () => undefined,
  import_grok_template: ({ input }) => ({ id: "qa-bot", name: "QA bot", creator: "Ulysses Ng", description: "Acts as your QA on the live deploy.", instructions: "Be strict.", color: "magenta", shape: "teardrop", memories: ["Never soft-pass a BLOCK item."], skills: [{ name: "fail-gate", description: "Use at ship time", content: "Step 1" }], routines: [{ name: "Post-merge", summary: "Re-run on merge." }], integrations: ["Vercel"], source: String(input).includes("marketplace") ? "marketplace" : "shared" }),
  composio_apps: () => ["gmail", "slack", "googlecalendar", "linear"],
  catalog_status: () => catalogApps.map((a) => ({ ...a, tools: a.state === "connected" ? 9 : 0, error: a.state === "reconnect" ? "The sign-in expired" : null })),
  catalog_connect: ({ slug }) => {
    const a = catalogApps.find((x) => x.slug === slug);
    if (a?.needs_client && catalogApps[0].state !== "connected") throw new Error(`${a.name} needs an OAuth key of its own. Connect Composio first (the first card) and it signs you in, or enter your own client ID.`);
    if (a) setTimeout(() => {
      a.state = "connected";
      emit({ type: "catalog_changed", slug: String(slug), connected: true, error: null });
    }, 1200);
    return `https://accounts.example.com/oauth?app=${slug}`;
  },
  catalog_disconnect: ({ slug }) => {
    const a = catalogApps.find((x) => x.slug === slug);
    if (a) a.state = "disconnected";
  },
  reset_thread: ({ threadId }) => {
    for (let i = messages.length - 1; i >= 0; i--) if (messages[i].thread_id === threadId) messages.splice(i, 1);
    emit({ type: "threads_changed" });
  },
  set_call_mode: () => undefined,
  agent_workspace: ({ id }) => `/Users/you/Library/Application Support/opencue/agents/${id}/workspace`,
  default_vm_folder: ({ id }) => `/Users/you/Library/Application Support/opencue/agents/${id}/vm`,
  default_workspace: ({ id }) => `/Users/you/Library/Application Support/opencue/agents/${id}/workspace`,
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
  vmpal_get: () => ({ toml: vmpalToml, machines: vmpalMachines() }),
  save_vmpal: ({ toml }) => {
    if (toml.trim() && !/\[mcp_servers\./.test(toml)) throw new Error("expected one or more [mcp_servers.<name>] tables");
    vmpalToml = toml.trim();
    return { toml: vmpalToml, machines: vmpalMachines() };
  },
  add_vmpal: ({ toml }) => {
    if (!/\[mcp_servers\./.test(toml)) throw new Error("paste at least one [mcp_servers.<name>] table");
    vmpalToml = (vmpalToml ? vmpalToml + "\n\n" : "") + toml.trim();
    return { toml: vmpalToml, machines: vmpalMachines() };
  },
  test_vmpal: ({ name }) => ({ id: "vmpal:" + name, name, state: "ready", error: null, tools: ["vmpal_screenshot", "vmpal_act", "vmpal_exec"] }),
  voice_settings: () => voice,
  save_voice_settings: ({ settings, apiKey }) => Object.assign(voice, settings, { configured: voice.configured || !!apiKey }),
  list_voices: () => [{ id: "21m00Tcm4TlvDq8ikWAM", name: "Rachel" }, { id: "pNInz6obpgDQGcFmaJgB", name: "Adam" }],
  transcribe: () => "Hola, ¿qué tengo en la agenda hoy?",
  save_canvas: ({ id }) => { throw new Error("canvas editing works in the desktop app (" + id + ")"); },
  speak: () => ({ mime: "audio/wav", data: "" }),
  models_status: () => localModels,
  model_install: ({ id }) => {
    const m = localModels.find((x) => x.id === id);
    if (!m) return;
    let done = 0;
    const timer = setInterval(() => {
      done += m.size / 8;
      if (done >= m.size) {
        clearInterval(timer);
        m.installed = true;
        if (id !== "sherpa-runtime") localModels.find((x) => x.id === "sherpa-runtime")!.installed = true;
        emit({ type: "model_progress", id: String(id), phase: "ready", done: 0, total: 0, error: null });
      } else emit({ type: "model_progress", id: String(id), phase: "downloading", done, total: m.size, error: null });
    }, 350);
  },
  model_cancel: () => undefined,
  model_delete: ({ id }) => void (localModels.find((x) => x.id === id)!.installed = false),
  personaplex_status: () => persona,
  personaplex_configure: ({ lan, offload }) => void Object.assign(persona, { lan, offload }),
  personaplex_install: () => void Object.assign(persona, { installed: true }),
  personaplex_start: () => void Object.assign(persona, { running: true, ready: true, url: "https://localhost:8998" }),
  personaplex_stop: () => void Object.assign(persona, { running: false, ready: false, url: null }),
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
