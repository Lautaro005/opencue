// Mirrors opencue-core's serde types.

export type ApprovalMode = "ask" | "autonomous";

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
  composio: boolean;
  voice_id: string | null;
  cli_tools: boolean;
  vmpal: string[];
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

export interface Group {
  id: string;
  name: string;
  max_auto_turns: number;
  members: GroupMember[];
  created_at: string;
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
  | "grok_cli";

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

export interface ApprovalRequest {
  id: string;
  thread_id: string;
  agent_id: string;
  message_id: string;
  tool_call: ToolCall;
  reason: string;
}

export type CoreEvent =
  | { type: "message"; message: Message }
  | { type: "delta"; thread_id: string; message_id: string; agent_id: string; text: string }
  | { type: "busy"; thread_id: string; agent_id: string; busy: boolean }
  | { type: "approval_requested"; approval: ApprovalRequest }
  | { type: "approval_resolved"; approval_id: string; approved: boolean }
  | { type: "threads_changed" }
  | { type: "mcp_changed" }
  | { type: "profile_changed"; agent_id: string }
  | { type: "automations_changed" }
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

export type EmailKind = "imap" | "composio_gmail";

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

export interface ComposioStatus {
  configured: boolean;
  user_id: string;
  accounts: { id: string; toolkit: string; status: string }[];
  error: string | null;
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
  configured: boolean;
  voice_id: string;
  auto_speak: boolean;
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
