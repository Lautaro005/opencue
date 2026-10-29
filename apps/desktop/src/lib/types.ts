// Mirrors opencue-core's serde types.

export type ApprovalMode = "read_only" | "ask" | "critical" | "autonomous";

export interface AgentSettings {
  approval_mode: ApprovalMode;
  web_access: boolean;
  mcp_all: boolean;
  mcp_servers: string[];
  use_global_memory: boolean;
  auto_memory: boolean;
  auto_profile: boolean;
  temperature: number | null;
  computer: ComputerBackend;
  skills: boolean;
  /** Lets the agent drive the built-in browser. */
  browser: boolean;
  /** Catalog apps (by slug) this agent can use. */
  catalog: string[];
  voice_id: string | null;
  cli_tools: boolean;
  vmpal: string[];
  /** Folder the agent works in; empty/null means its own folder in OpenCue's data. */
  working_dir: string | null;
  vm: VmOptions;
}

export interface VmOptions {
  /** Live desktop; null follows Settings → Computers. */
  desktop: boolean | null;
  memory_mb: number | null;
  cpus: number | null;
  /** Folder that keeps the VM's home; null uses a folder inside OpenCue's data. */
  data_dir: string | null;
  /** "docker" | "podman"; null follows Settings → Computers. */
  runtime: "docker" | "podman" | null;
}

export type ComputerBackend = "none" | "host" | "docker" | "wsl" | "space" | "vmpal";

export interface Avatar {
  face: string;
  color: string;
  shape: string; // see SHAPE_DEFS in components/AvatarView
}

export interface Agent {
  id: string;
  name: string;
  avatar: Avatar;
  provider_id: string | null;
  model: string | null;
  settings: AgentSettings;
  created_at: string;
  updated_at: string;
}

export interface NewAgent {
  name: string;
  avatar?: Avatar;
  provider_id?: string | null;
  model?: string | null;
  instructions?: string;
  settings?: AgentSettings;
}

export interface AgentPatch {
  name?: string;
  avatar?: Avatar;
  provider_id?: string | null;
  model?: string | null;
  settings?: AgentSettings;
}

export interface GroupMember {
  agent_id: string;
  is_owner: boolean;
}

export type GroupKind = "chat" | "council" | "coding";

export interface Group {
  id: string;
  name: string;
  max_auto_turns: number;
  members: GroupMember[];
  created_at: string;
  kind: GroupKind;
  /** A coding group's `repo_dir` and `isolate`. */
  config: { repo_dir?: string; isolate?: boolean; [key: string]: unknown };
}

export interface RepoStatus {
  dir: string;
  is_repo: boolean;
  git_installed: boolean;
  gh_installed: boolean;
  gh_user: string | null;
  slug: string | null;
  branch: string | null;
  default_branch: string | null;
  ahead: number;
  behind: number;
  changed_files: number;
  additions: number;
  deletions: number;
  worktrees: { path: string; branch: string }[];
  error: string | null;
}

export interface Checks {
  total: number;
  passed: number;
  failed: number;
  pending: number;
  failing: string[];
}

export interface Pull {
  number: number;
  title: string;
  branch: string;
  base: string;
  author: string;
  draft: boolean;
  review: string;
  checks: Checks;
  url: string;
  updated_at: string;
  additions: number;
  deletions: number;
  files: number;
  mergeable: string;
  auto_merge: boolean;
}

export interface FileChange {
  path: string;
  status: string;
  additions: number;
  deletions: number;
}

/** One working folder of a coding group and what changed in it. */
export interface Lane {
  agent_ids: string[];
  dir: string;
  branch: string | null;
  files: FileChange[];
}

export interface FamilyApp {
  id: string;
  name: string;
  installed: boolean;
  running: boolean | null;
  version: string | null;
  home: string | null;
  url: string | null;
  /** off | on | via_workstudio */
  mcp: string;
  install_command: string;
  repo_url: string;
  can_start: boolean;
}

export interface CliVersion {
  kind: string;
  installed: string | null;
  latest: string | null;
  outdated: boolean;
}

export interface CliSkill {
  name: string;
  description: string;
  path: string;
  /** claude | codex | agents | grok | vibe | gemini */
  source: string;
}

export interface Commit {
  sha: string;
  subject: string;
  author: string;
  when: string;
  files: FileChange[];
  /** Already on the upstream branch (on GitHub). */
  pushed: boolean;
}

export interface Changes {
  files: FileChange[];
  patch: string;
  truncated: boolean;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
}

export type Role = "user" | "assistant" | "tool" | "event";

export interface Message {
  id: string;
  thread_id: string;
  role: Role;
  agent_id: string | null;
  content: string;
  tool_calls: ToolCall[] | null;
  tool_call_id: string | null;
  tool_name: string | null;
  status: string; // done | streaming | running | awaiting_approval | denied | error | stopped
  input_tokens: number;
  output_tokens: number;
  created_at: string;
  attachments?: Attachment[];
}

export interface Attachment {
  id: string;
  name: string;
  mime: string;
  size: number;
  /** Set when an agent made this file with `create_artifact`. */
  artifact?: ArtifactRef;
}

export type ArtifactKind = "html" | "svg" | "markdown" | "mermaid" | "code" | "text" | "json" | "csv" | "xlsx" | "docx" | "pptx" | "pdf";

export interface ArtifactRef {
  id: string;
  version: number;
  kind: ArtifactKind;
  title: string;
  canvas?: boolean;
}

export interface ArtifactVersion {
  version: number;
  attachment: Attachment;
  created_at: string;
  /** The text the agent sent for this version. */
  source: string;
  /** "user" when this version was edited by hand in the canvas. */
  by?: string;
}

export interface Artifact {
  id: string;
  thread_id: string;
  agent_id: string;
  title: string;
  kind: ArtifactKind;
  language: string | null;
  canvas?: boolean;
  versions: ArtifactVersion[];
}

/** What the dashboard page draws (see the contract in the built-in page). */
export interface DashboardData {
  now: string;
  theme?: "light" | "dark";
  user_name: string;
  agents: { total: number; running: number; list: { id: string; name: string; model: string | null; busy: boolean; avatar: Avatar }[] };
  running: { thread_id: string; name: string; kind: "agent" | "group" }[];
  automations: { total: number; enabled: number; upcoming: unknown[]; list: unknown[] };
  tasks: { agent_id: string; agent_name: string; done: number; total: number; items: { text: string; status: "todo" | "doing" | "done" }[] }[];
  usage: { messages: number; input_tokens: number; output_tokens: number; by_agent: { agent_id: string; name: string; messages: number; tokens: number }[] };
  recent: { thread_id: string; name: string; kind: "agent" | "group"; last_message: string | null; last_at: string | null }[];
}

/** A file the user is about to send (base64 bytes). */
export interface NewAttachment {
  name: string;
  data: string;
}

export interface SharedFile {
  attachment: Attachment;
  message_id: string;
  agent_id: string | null;
  created_at: string;
}

export interface ThreadSummary {
  kind: "agent" | "group";
  id: string;
  name: string;
  avatars: Avatar[];
  last_message: string | null;
  last_at: string | null;
  busy: boolean;
}

export type ProviderKind =
  | "openai"
  | "anthropic"
  | "xai"
  | "openrouter"
  | "cheaperinference"
  | "ollama"
  | "ollama_cloud"
  | "lmstudio"
  | "openai_compatible"
  | "codex_cli"
  | "claude_cli"
  | "grok_cli"
  | "muse_cli"
  | "gemini"
  | "muse"
  | "mistral_api"
  | "groq"
  | "vibe_cli"
  | "antigravity_cli";

export interface ProviderInfo {
  kind: ProviderKind;
  label: string;
  api: "openai" | "anthropic" | "cli";
  default_base_url: string | null;
  default_model: string | null;
  requires_key: boolean;
  supports_tools: boolean;
  note: string;
}

export interface ProviderConfig {
  id: string;
  kind: ProviderKind;
  name: string;
  base_url: string | null;
  default_model: string | null;
  enabled: boolean;
  has_key: boolean;
}

export interface McpServerConfig {
  id: string;
  name: string;
  transport: "stdio" | "http";
  command: string | null;
  args: string[];
  env: Record<string, string>;
  url: string | null;
  headers: Record<string, string>;
  enabled: boolean;
}

export interface McpServerStatus {
  id: string;
  name: string;
  state: "stopped" | "connecting" | "ready" | "error";
  error: string | null;
  tools: string[];
}

export interface MemoryEntry {
  id: number;
  text: string;
}

export interface UsageRow {
  agent_id: string | null;
  agent_name: string | null;
  messages: number;
  input_tokens: number;
  output_tokens: number;
}

export interface AppInfo {
  version: string;
  platform: string;
  data_dir: string;
  space_supported: boolean;
}

export interface StorageItem {
  id: "chats" | "agents" | "files" | "artifacts" | "models" | "memory" | "messenger" | "space" | "other";
  bytes: number;
  files: number;
}

export interface StorageReport {
  root: string;
  total: number;
  items: StorageItem[];
  disk_free: number | null;
  disk_total: number | null;
}

export interface ApprovalRequest {
  id: string;
  thread_id: string;
  agent_id: string;
  message_id: string;
  tool_call: ToolCall;
  reason: string;
}

/** An agent asks the user something: free text, choices or a yes/no permission. */
export interface Question {
  id: string;
  thread_id: string;
  agent_id: string;
  kind: "question" | "permission";
  title: string;
  body: string;
  options: { title: string; description: string }[];
}

export interface DockerSetup {
  installed: boolean;
  running: boolean;
  image_ready: boolean;
  vm: "none" | "stopped" | "running";
}

export interface SearchHit {
  message_id: string;
  thread_id: string;
  role: string;
  agent_id: string | null;
  content: string;
  created_at: string;
}

export interface CatalogStatus {
  slug: string;
  name: string;
  description: string;
  /** red: disconnected, yellow: reconnect, green: connected */
  state: "disconnected" | "reconnect" | "connected";
  error: string | null;
  tools: number;
  /** The provider needs an OAuth key of your own; Composio can sign you in instead. */
  needs_client: boolean;
  /** Connected through Composio rather than directly. */
  via_composio: boolean;
}

export interface TermInfo {
  id: string;
  tool: string;
  label: string;
  cwd: string;
  created_at: string;
  alive: boolean;
}

export interface TermTool {
  key: string;
  label: string;
  installed: boolean;
  install: string;
}

export type CoreEvent =
  | { type: "message"; message: Message }
  | { type: "term_output"; id: string; data: string }
  | { type: "term_exit"; id: string; code: number }
  | { type: "tool_progress"; thread_id: string; message_id: string; agent_id: string; tool: string; title: string | null; chars: number }
  | { type: "model_progress"; id: string; phase: string; done: number; total: number; error: string | null }
  | { type: "delta"; thread_id: string; message_id: string; agent_id: string; text: string }
  | { type: "messages_removed"; thread_id: string; ids: string[] }
  | { type: "busy"; thread_id: string; agent_id: string; busy: boolean }
  | { type: "approval_requested"; approval: ApprovalRequest }
  | { type: "approval_resolved"; approval_id: string; approved: boolean }
  | { type: "browser_activity"; agent_id: string }
  | { type: "messenger_changed" }
  | { type: "catalog_changed"; slug: string; connected: boolean; error: string | null }
  | { type: "question_requested"; question: Question }
  | { type: "question_resolved"; question_id: string; answer: string }
  | { type: "threads_changed" }
  | { type: "mcp_changed" }
  | { type: "profile_changed"; agent_id: string }
  | { type: "automations_changed" }
  | { type: "dashboard_changed" }
  | { type: "computer_output"; agent_id: string; text: string; stderr: boolean }
  | { type: "error"; thread_id: string | null; message: string };

export interface SkillSummary {
  name: string;
  description: string;
  path: string;
}

export interface Skill extends SkillSummary {
  body: string;
}

export type Trigger =
  | { kind: "schedule"; cron: string }
  | { kind: "email"; filter: string; poll_minutes: number }
  | { kind: "webhook"; token: string }
  | { kind: "group_message"; group_id: string; keyword: string };

export interface Automation {
  id: string;
  agent_id: string;
  name: string;
  trigger: Trigger;
  prompt: string;
  enabled: boolean;
  last_run_at: string | null;
  last_status: string | null;
  created_at: string;
  next_run_at: string | null;
}

export interface AutomationInput {
  id?: string | null;
  agent_id: string;
  name: string;
  trigger: Trigger;
  prompt: string;
  enabled: boolean;
}

export type EmailKind = "imap";

export interface EmailAccount {
  agent_id: string;
  kind: EmailKind;
  address: string;
  display_name: string | null;
  imap_host: string | null;
  imap_port: number;
  smtp_host: string | null;
  smtp_port: number;
  username: string | null;
  last_seen_uid: number;
  has_password: boolean;
}

export interface ComputerSettings {
  docker_image: string;
  docker_desktop: boolean;
  desktop_image: string;
  desktop_port: number;
  space_url: string;
  space_template: string;
  has_space_token: boolean;
  os: string;
  docker_available: boolean;
  podman_available: boolean;
  /** The container engine behind Local VMs. */
  runtime: "docker" | "podman";
  /** One Docker computer for every agent instead of one each. */
  shared_vm: boolean;
  /** Minutes of inactivity before a Docker computer stops; 0 never. */
  idle_minutes: number;
  wsl_available: boolean;
  webhook_port: number;
}

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
}

export interface CommandResult {
  exit_code: number;
  output: string;
  timed_out: boolean;
}

export interface VoiceSettings {
  /** Speech in and speech out both work (an ElevenLabs key, or local models). */
  configured: boolean;
  has_key: boolean;
  voice_id: string;
  auto_speak: boolean;
  /** "auto" | "elevenlabs" | "local" */
  stt_engine: string;
  tts_engine: string;
  local_stt: string;
  local_tts: string;
  local_voice: string;
}

export interface SpeechAudio {
  mime: string;
  data: string;
}

export interface LocalModel {
  id: string;
  kind: "stt" | "tts" | "runtime";
  label: string;
  size: number;
  license: string;
  source: string;
  languages: string[];
  note: string;
  ram_gb: number;
  installed: boolean;
  supported: boolean;
}

export interface PersonaStatus {
  /** "personaplex" (NVIDIA, CUDA) or "moshi_mlx" (light, Apple silicon). */
  engine: string;
  mlx_supported: boolean;
  installed: boolean;
  installing: boolean;
  running: boolean;
  ready: boolean;
  has_token: boolean;
  lan: boolean;
  offload: boolean;
  gpu: string | null;
  python: string | null;
  url: string | null;
  lan_url: string | null;
  qr_svg: string | null;
  log: string;
}

export interface Voice {
  id: string;
  name: string;
}

export interface VmPalMachine {
  name: string;
  command: string;
  args: string[];
  env: Record<string, string>;
}

export interface VmPalInfo {
  toml: string;
  machines: VmPalMachine[];
}

export interface LimitWindow {
  label: string;
  used_percent: number | null;
  resets_at: number | null;
}

export interface CliLimits {
  cli: string;
  name: string;
  plan: string | null;
  windows: LimitWindow[];
  updated_at: number | null;
  note: string | null;
}

export interface MessengerStatus {
  state: "off" | "starting" | "qr" | "connected" | "error";
  qr_svg: string | null;
  phone: string | null;
  agent_id: string | null;
  allow: string;
  error: string | null;
}

export interface GrokTemplate {
  id: string;
  name: string;
  creator: string;
  description: string;
  instructions: string;
  color: string;
  shape: string;
  memories: string[];
  skills: { name: string; description: string; content: string }[];
  routines: { name: string; summary: string }[];
  integrations: string[];
  /** "marketplace" has everything; "shared" only a name and a description. */
  source: "marketplace" | "shared";
}

export interface BrowserTab {
  id: number;
  url: string;
  title: string;
  active: boolean;
}

export interface SpaceStatus {
  supported: boolean;
  /** idle | checking | installing | ready | error */
  state: string;
  step: string;
  checks: { id: string; ok: boolean; detail: string }[];
  log: string;
  dir: string;
  managed: boolean;
  api_up: boolean;
}
