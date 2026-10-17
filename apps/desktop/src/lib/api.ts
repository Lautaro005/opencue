// Thin typed wrapper over Tauri IPC. Outside Tauri (plain browser) a mock backend is used.

import type {
  Agent,
  AgentPatch,
  AppInfo,
  CoreEvent,
  Group,
  McpServerConfig,
  McpServerStatus,
  MemoryEntry,
  Message,
  NewAgent,
  ProviderConfig,
  ProviderInfo,
  ThreadSummary,
  UsageRow,
} from "./types";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = (cb: (ev: CoreEvent) => void) => Promise<() => void>;

let invokeImpl: Invoke;
let listenImpl: Listen;

if (isTauri) {
  const core = await import("@tauri-apps/api/core");
  const event = await import("@tauri-apps/api/event");
  invokeImpl = core.invoke;
  listenImpl = (cb) => event.listen<CoreEvent>("core-event", (e) => cb(e.payload));
} else {
  const mock = await import("./mock");
  invokeImpl = mock.invoke;
  listenImpl = mock.listen;
}

export const onCoreEvent = (cb: (ev: CoreEvent) => void) => listenImpl(cb);

export type MemoryScope = "agent" | "global";

export const api = {
  appInfo: () => invokeImpl<AppInfo>("app_info"),
  listThreads: () => invokeImpl<ThreadSummary[]>("list_threads"),
  listAgents: () => invokeImpl<Agent[]>("list_agents"),
  getAgent: (id: string) => invokeImpl<Agent>("get_agent", { id }),
  createAgent: (agent: NewAgent) => invokeImpl<Agent>("create_agent", { agent }),
  updateAgent: (id: string, patch: AgentPatch) => invokeImpl<Agent>("update_agent", { id, patch }),
  deleteAgent: (id: string) => invokeImpl<void>("delete_agent", { id }),
  getInstructions: (id: string) => invokeImpl<string>("get_instructions", { id }),
  setInstructions: (id: string, text: string) => invokeImpl<void>("set_instructions", { id, text }),
  listMemory: (scope: MemoryScope, agentId?: string) => invokeImpl<MemoryEntry[]>("list_memory", { scope, agentId }),
  addMemory: (scope: MemoryScope, text: string, agentId?: string) => invokeImpl<MemoryEntry>("add_memory", { scope, agentId, text }),
  deleteMemory: (scope: MemoryScope, id: number, agentId?: string) => invokeImpl<void>("delete_memory", { scope, agentId, id }),
  getMemoryRaw: (scope: MemoryScope, agentId?: string) => invokeImpl<string>("get_memory_raw", { scope, agentId }),
  setMemoryRaw: (scope: MemoryScope, text: string, agentId?: string) => invokeImpl<void>("set_memory_raw", { scope, agentId, text }),
  listGroups: () => invokeImpl<Group[]>("list_groups"),
  getGroup: (id: string) => invokeImpl<Group | null>("get_group", { id }),
  createGroup: (name: string, agentIds: string[]) => invokeImpl<Group>("create_group", { name, agentIds }),
  updateGroup: (id: string, name?: string, maxAutoTurns?: number) => invokeImpl<void>("update_group", { id, name, maxAutoTurns }),
  deleteGroup: (id: string) => invokeImpl<void>("delete_group", { id }),
  addGroupMember: (groupId: string, agentId: string) => invokeImpl<void>("add_group_member", { groupId, agentId }),
  removeGroupMember: (groupId: string, agentId: string) => invokeImpl<void>("remove_group_member", { groupId, agentId }),
  getMessages: (threadId: string) => invokeImpl<Message[]>("get_messages", { threadId }),
  sendMessage: (threadId: string, text: string) => invokeImpl<Message>("send_message", { threadId, text }),
  stopThread: (threadId: string) => invokeImpl<void>("stop_thread", { threadId }),
  clearThread: (threadId: string) => invokeImpl<void>("clear_thread", { threadId }),
  resolveApproval: (approvalId: string, approved: boolean) => invokeImpl<boolean>("resolve_approval", { approvalId, approved }),
  providerCatalog: () => invokeImpl<ProviderInfo[]>("provider_catalog"),
  listProviders: () => invokeImpl<ProviderConfig[]>("list_providers"),
  saveProvider: (provider: ProviderConfig, apiKey?: string) => invokeImpl<ProviderConfig>("save_provider", { provider, apiKey }),
  deleteProvider: (id: string) => invokeImpl<void>("delete_provider", { id }),
  listModels: (providerId: string) => invokeImpl<string[]>("list_models", { providerId }),
  listMcpServers: () => invokeImpl<McpServerConfig[]>("list_mcp_servers"),
  mcpStatus: () => invokeImpl<McpServerStatus[]>("mcp_status"),
  saveMcpServer: (server: McpServerConfig) => invokeImpl<McpServerConfig>("save_mcp_server", { server }),
  deleteMcpServer: (id: string) => invokeImpl<void>("delete_mcp_server", { id }),
  importMcpJson: (json: string) => invokeImpl<McpServerConfig[]>("import_mcp_json", { json }),
  importMcpFile: (path: string) => invokeImpl<McpServerConfig[]>("import_mcp_file", { path }),
  getSettings: () => invokeImpl<Record<string, string>>("get_settings"),
  setSetting: (key: string, value: string) => invokeImpl<void>("set_setting", { key, value }),
  usage: () => invokeImpl<UsageRow[]>("usage"),
  webSearch: (query: string) => invokeImpl<{ title: string; url: string; snippet: string; source: string }[]>("web_search", { query }),
};

export async function pickJsonFile(): Promise<string | null> {
  if (!isTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
  return typeof picked === "string" ? picked : null;
}

export async function openUrl(url: string) {
  if (isTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener");
  }
}
