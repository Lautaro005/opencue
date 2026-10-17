// Mirrors opencue-core's serde types.

export type ApprovalMode = "ask" | "autonomous";

export interface AgentSettings {
  approval_mode: ApprovalMode;
  web_access: boolean;
  mcp_all: boolean;
  mcp_servers: string[];
  use_global_memory: boolean;
  auto_memory: boolean;
  temperature: number | null;
}

export interface Avatar {
  emoji: string;
  color: string;
  shape: string; // circle | squircle | blob
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
  | "cheapestinference"
  | "ollama"
  | "ollama_cloud"
  | "lmstudio"
  | "openai_compatible"
  | "codex_cli"
  | "claude_cli";

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
  | { type: "error"; thread_id: string | null; message: string };
