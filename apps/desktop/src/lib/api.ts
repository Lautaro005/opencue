// Thin typed wrapper over Tauri IPC. Outside Tauri (plain browser) a mock backend is used.

import type {
  DockerSetup,
  Question,
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
  Automation,
  AutomationInput,
  CommandResult,
  CatalogStatus,
  ComputerSettings,
  EmailAccount,
  FileEntry,
  Skill,
  SkillSummary,
  Voice,
  VoiceSettings,
  NewAttachment,
  SharedFile,
  VmPalInfo,
  SearchHit,
  Artifact,
  BrowserTab,
  DashboardData,
  GrokTemplate,
  MessengerStatus,
  CliLimits,
  LocalModel,
  SpaceStatus,
  TermInfo,
  TermTool,
  GroupKind,
  RepoStatus,
  Pull,
  Changes, Lane,
  PersonaStatus,
  SpeechAudio,
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

/** What the menu-bar icon asked for: "settings" or "check-updates". */
export async function onTrayAction(cb: (action: string) => void): Promise<() => void> {
  if (!isTauri) return () => {};
  const event = await import("@tauri-apps/api/event");
  return event.listen<string>("tray-action", (e) => cb(e.payload));
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
  getSoul: (id: string) => invokeImpl<string>("get_soul", { id }),
  setSoul: (id: string, text: string) => invokeImpl<void>("set_soul", { id, text }),
  /** A new agent's first message (only posted into an empty chat). */
  greetAgent: (id: string, text: string) => invokeImpl<void>("greet_agent", { id, text }),
  notchInfo: () => invokeImpl<{ macos: boolean; notch_height: number }>("notch_info"),
  notchPlace: (width: number, height: number) => invokeImpl<void>("notch_place", { width, height }),
  notchHide: () => invokeImpl<void>("notch_hide"),
  mainInView: () => invokeImpl<boolean>("main_in_view"),
  testNotification: (title: string, body: string) => invokeImpl<void>("test_notification", { title, body }),
  listMemory: (scope: MemoryScope, agentId?: string) => invokeImpl<MemoryEntry[]>("list_memory", { scope, agentId }),
  addMemory: (scope: MemoryScope, text: string, agentId?: string) => invokeImpl<MemoryEntry>("add_memory", { scope, agentId, text }),
  deleteMemory: (scope: MemoryScope, id: number, agentId?: string) => invokeImpl<void>("delete_memory", { scope, agentId, id }),
  getMemoryRaw: (scope: MemoryScope, agentId?: string) => invokeImpl<string>("get_memory_raw", { scope, agentId }),
  setMemoryRaw: (scope: MemoryScope, text: string, agentId?: string) => invokeImpl<void>("set_memory_raw", { scope, agentId, text }),
  listGroups: () => invokeImpl<Group[]>("list_groups"),
  getGroup: (id: string) => invokeImpl<Group | null>("get_group", { id }),
  createGroup: (name: string, agentIds: string[], kind?: GroupKind, config?: Record<string, unknown>) => invokeImpl<Group>("create_group", { name, agentIds, kind, config }),
  codingSetRepo: (groupId: string, dir: string, worktrees: boolean) => invokeImpl<RepoStatus>("coding_set_repo", { groupId, dir, worktrees }),
  githubStatus: (groupId: string) => invokeImpl<RepoStatus>("github_status", { groupId }),
  githubPulls: (groupId: string, prState = "open") => invokeImpl<Pull[]>("github_pulls", { groupId, prState }),
  githubChanges: (groupId: string) => invokeImpl<Changes>("github_changes", { groupId }),
  unqueueMessage: (threadId: string, messageId: string) => invokeImpl<void>("unqueue_message", { threadId, messageId }),
  codingActivity: (groupId: string) => invokeImpl<Lane[]>("coding_activity", { groupId }),
  githubCreatePr: (groupId: string, title: string, body: string, draft: boolean, base?: string) => invokeImpl<string>("github_create_pr", { groupId, title, body, base, draft }),
  githubMergePr: (groupId: string, number: number, method: string, auto: boolean) => invokeImpl<string>("github_merge_pr", { groupId, number, method, auto }),
  githubComment: (groupId: string, number: number, body: string) => invokeImpl<void>("github_comment", { groupId, number, body }),
  codingFixCi: (groupId: string, number: number) => invokeImpl<void>("coding_fix_ci", { groupId, number }),
  updateGroup: (id: string, name?: string, maxAutoTurns?: number) => invokeImpl<void>("update_group", { id, name, maxAutoTurns }),
  setGroupKind: (id: string, kind: GroupKind) => invokeImpl<void>("set_group_kind", { id, kind }),
  deleteGroup: (id: string) => invokeImpl<void>("delete_group", { id }),
  addGroupMember: (groupId: string, agentId: string) => invokeImpl<void>("add_group_member", { groupId, agentId }),
  removeGroupMember: (groupId: string, agentId: string) => invokeImpl<void>("remove_group_member", { groupId, agentId }),
  getMessages: (threadId: string, limit?: number) => invokeImpl<Message[]>("get_messages", { threadId, limit }),
  cliLimits: () => invokeImpl<CliLimits[]>("cli_limits"),
  messengerStatus: () => invokeImpl<MessengerStatus>("messenger_status"),
  whatsappConfigure: (agentId: string | null, allow: string) => invokeImpl<void>("whatsapp_configure", { agentId, allow }),
  whatsappConnect: () => invokeImpl<void>("whatsapp_connect"),
  whatsappDisconnect: () => invokeImpl<void>("whatsapp_disconnect"),
  importGrokTemplate: (input: string) => invokeImpl<GrokTemplate>("import_grok_template", { input }),
  searchMessages: (query: string) => invokeImpl<SearchHit[]>("search_messages", { query }),
  sendMessage: (threadId: string, text: string, files?: NewAttachment[]) => invokeImpl<Message>("send_message", { threadId, text, files }),
  threadFiles: (threadId: string) => invokeImpl<SharedFile[]>("thread_files", { threadId }),
  readAttachment: (id: string) => invokeImpl<string>("read_attachment", { id }),
  openAttachment: (id: string) => invokeImpl<void>("open_attachment", { id }),
  revealAttachment: (id: string) => invokeImpl<void>("reveal_attachment", { id }),
  saveAttachment: (id: string, dest: string) => invokeImpl<void>("save_attachment", { id, dest }),
  stopThread: (threadId: string) => invokeImpl<void>("stop_thread", { threadId }),
  getArtifact: (id: string) => invokeImpl<Artifact>("get_artifact", { id }),
  listArtifacts: (agentId: string) => invokeImpl<Artifact[]>("list_artifacts", { agentId }),
  dashboardData: () => invokeImpl<DashboardData>("dashboard_data"),
  dashboardInfo: () => invokeImpl<{ html: string; is_default: boolean }>("dashboard_info"),
  resetDashboard: () => invokeImpl<void>("reset_dashboard"),
  viewBase: () => invokeImpl<string>("view_base"),
  regenerateLast: (threadId: string, text?: string) => invokeImpl<void>("regenerate_last", { threadId, text }),
  clearThread: (threadId: string) => invokeImpl<void>("clear_thread", { threadId }),
  resolveApproval: (approvalId: string, approved: boolean) => invokeImpl<boolean>("resolve_approval", { approvalId, approved }),
  answerQuestion: (questionId: string, answer: string) => invokeImpl<boolean>("answer_question", { questionId, answer }),
  pendingQuestions: () => invokeImpl<Question[]>("pending_questions"),
  dockerSetup: () => invokeImpl<DockerSetup>("docker_setup"),
  dockerPrepare: () => invokeImpl<void>("docker_prepare"),
  dockerStartShared: () => invokeImpl<void>("docker_start_shared"),
  catalogStatus: () => invokeImpl<CatalogStatus[]>("catalog_status"),
  catalogConnect: (slug: string, clientId?: string, clientSecret?: string) => invokeImpl<string>("catalog_connect", { slug, clientId, clientSecret }),
  composioApps: (refresh = false) => invokeImpl<string[]>("composio_apps", { refresh }),
  catalogDisconnect: (slug: string) => invokeImpl<void>("catalog_disconnect", { slug }),
  resetThread: (threadId: string) => invokeImpl<void>("reset_thread", { threadId }),
  setCallMode: (threadId: string, on: boolean) => invokeImpl<void>("set_call_mode", { threadId, on }),
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
  listSkills: () => invokeImpl<SkillSummary[]>("list_skills"),
  getSkill: (name: string) => invokeImpl<Skill | null>("get_skill", { name }),
  saveSkill: (name: string, description: string, body: string) => invokeImpl<Skill>("save_skill", { name, description, body }),
  deleteSkill: (name: string) => invokeImpl<void>("delete_skill", { name }),
  importSkill: (url: string) => invokeImpl<Skill>("import_skill", { url }),
  listAutomations: (agentId?: string) => invokeImpl<Automation[]>("list_automations", { agentId }),
  saveAutomation: (automation: AutomationInput) => invokeImpl<Automation>("save_automation", { automation }),
  deleteAutomation: (id: string) => invokeImpl<void>("delete_automation", { id }),
  runAutomation: (id: string) => invokeImpl<void>("run_automation", { id }),
  newWebhookToken: () => invokeImpl<string>("new_webhook_token"),
  getEmailAccount: (agentId: string) => invokeImpl<EmailAccount | null>("get_email_account", { agentId }),
  saveEmailAccount: (account: EmailAccount, password?: string) => invokeImpl<EmailAccount>("save_email_account", { account, password }),
  deleteEmailAccount: (agentId: string) => invokeImpl<void>("delete_email_account", { agentId }),
  testEmail: (agentId: string) => invokeImpl<void>("test_email", { agentId }),
  computerSettings: () => invokeImpl<ComputerSettings>("computer_settings"),
  saveComputerSettings: (settings: ComputerSettings, spaceToken?: string) => invokeImpl<ComputerSettings>("save_computer_settings", { settings, spaceToken }),
  testSpace: () => invokeImpl<number>("test_space"),
  computerExec: (agentId: string, command: string) => invokeImpl<CommandResult>("computer_exec", { agentId, command }),
  computerList: (agentId: string, path: string) => invokeImpl<FileEntry[]>("computer_list", { agentId, path }),
  computerRead: (agentId: string, path: string) => invokeImpl<string>("computer_read", { agentId, path }),
  computerLog: (agentId: string) => invokeImpl<string>("computer_log", { agentId }),
  computerDesktop: (agentId: string) => invokeImpl<string>("computer_desktop", { agentId }),
  resetComputer: (agentId: string) => invokeImpl<void>("reset_computer", { agentId }),
  dockerStartEngine: () => invokeImpl<void>("docker_start_engine"),
  stopAllComputers: () => invokeImpl<number>("stop_all_computers"),
  browserPlace: (agentId: string, x: number, y: number, w: number, h: number) => invokeImpl<void>("browser_place", { agentId, x, y, w, h }),
  browserHide: () => invokeImpl<void>("browser_hide"),
  browserControl: (agentId: string, action: string, url?: string) => invokeImpl<void>("browser_control", { agentId, action, url }),
  browserState: (agentId: string) => invokeImpl<[string, string]>("browser_state", { agentId }),
  browserClear: (agentId: string) => invokeImpl<void>("browser_clear", { agentId }),
  browserTabs: (agentId: string) => invokeImpl<BrowserTab[]>("browser_tabs", { agentId }),
  browserTabNew: (agentId: string, url?: string) => invokeImpl<number>("browser_tab_new", { agentId, url }),
  browserTabSelect: (agentId: string, tab: number) => invokeImpl<void>("browser_tab_select", { agentId, tab }),
  browserTabClose: (agentId: string, tab: number) => invokeImpl<void>("browser_tab_close", { agentId, tab }),
  stopComputer: (agentId: string) => invokeImpl<void>("stop_computer", { agentId }),
  computerRunning: (agentId: string) => invokeImpl<boolean>("computer_running", { agentId }),
  agentWorkspace: (id: string) => invokeImpl<string>("agent_workspace", { id }),
  defaultWorkspace: (id: string) => invokeImpl<string>("default_workspace", { id }),
  defaultVmFolder: (id: string) => invokeImpl<string>("default_vm_folder", { id }),
  voiceSettings: () => invokeImpl<VoiceSettings>("voice_settings"),
  saveVoiceSettings: (settings: VoiceSettings, apiKey?: string) => invokeImpl<VoiceSettings>("save_voice_settings", { settings, apiKey }),
  listVoices: () => invokeImpl<Voice[]>("list_voices"),
  transcribe: (audioBase64: string, mime: string) => invokeImpl<string>("transcribe", { audioBase64, mime }),
  speak: (text: string, agentId?: string) => invokeImpl<SpeechAudio>("speak", { text, agentId }),
  spaceStatus: () => invokeImpl<SpaceStatus>("space_status"),
  spaceInstall: () => invokeImpl<void>("space_install"),
  spaceStart: () => invokeImpl<void>("space_start"),
  spaceStop: () => invokeImpl<void>("space_stop"),
  spaceCancel: () => invokeImpl<void>("space_cancel"),
  termTools: () => invokeImpl<TermTool[]>("term_tools"),
  termList: () => invokeImpl<TermInfo[]>("term_list"),
  termCreate: (tool: string, cwd: string, cols: number, rows: number) => invokeImpl<TermInfo>("term_create", { tool, cwd, cols, rows }),
  termRestart: (id: string, cols: number, rows: number) => invokeImpl<void>("term_restart", { id, cols, rows }),
  termWrite: (id: string, data: string) => invokeImpl<void>("term_write", { id, data }),
  termResize: (id: string, cols: number, rows: number) => invokeImpl<void>("term_resize", { id, cols, rows }),
  termSnapshot: (id: string) => invokeImpl<string>("term_snapshot", { id }),
  termKill: (id: string) => invokeImpl<void>("term_kill", { id }),
  termClose: (id: string) => invokeImpl<void>("term_close", { id }),
  termRename: (id: string, label: string) => invokeImpl<void>("term_rename", { id, label }),
  homeDir: () => invokeImpl<string>("home_dir"),
  openFloat: (kind: string, id: string, title: string) => invokeImpl<void>("open_float", { kind, id, title }),
  floatPin: (on: boolean) => invokeImpl<void>("float_pin", { on }),
  closeFloat: () => invokeImpl<void>("close_float"),
  saveCanvas: (id: string, content: string) => invokeImpl<Artifact>("save_canvas", { id, content }),
  modelsStatus: () => invokeImpl<LocalModel[]>("models_status"),
  modelInstall: (id: string) => invokeImpl<void>("model_install", { id }),
  modelCancel: (id: string) => invokeImpl<void>("model_cancel", { id }),
  modelDelete: (id: string) => invokeImpl<void>("model_delete", { id }),
  personaStatus: () => invokeImpl<PersonaStatus>("personaplex_status"),
  personaConfigure: (lan: boolean, offload: boolean, token?: string, engine?: string) => invokeImpl<void>("personaplex_configure", { lan, offload, token, engine }),
  personaInstall: () => invokeImpl<void>("personaplex_install"),
  personaStart: () => invokeImpl<void>("personaplex_start"),
  personaStop: () => invokeImpl<void>("personaplex_stop"),
  vmpalGet: () => invokeImpl<VmPalInfo>("vmpal_get"),
  saveVmpal: (toml: string) => invokeImpl<VmPalInfo>("save_vmpal", { toml }),
  addVmpal: (toml: string) => invokeImpl<VmPalInfo>("add_vmpal", { toml }),
  testVmpal: (name: string) => invokeImpl<McpServerStatus>("test_vmpal", { name }),
  webSearch: (query: string) => invokeImpl<{ title: string; url: string; snippet: string; source: string }[]>("web_search", { query }),
};

export async function pickJsonFile(): Promise<string | null> {
  if (!isTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
  return typeof picked === "string" ? picked : null;
}

/** A native "choose a folder" window. Returns null when cancelled (or in the browser preview). */
export async function pickFolder(): Promise<string | null> {
  if (!isTauri) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false });
  return typeof picked === "string" ? picked : null;
}

export async function openUrl(url: string) {
  if (isTauri) {
    await invokeImpl<void>("open_external", { url });
  } else {
    window.open(url, "_blank", "noopener");
  }
}

/** Ask where to save a shared file, then copy it there. */
export async function saveAttachmentAs(id: string, name: string): Promise<boolean> {
  if (!isTauri) return false;
  const { save } = await import("@tauri-apps/plugin-dialog");
  const dest = await save({ defaultPath: name });
  if (!dest) return false;
  await api.saveAttachment(id, dest);
  return true;
}

/** Read browser File objects as base64 for `sendMessage`. */
export async function filesToAttachments(files: File[]): Promise<NewAttachment[]> {
  return Promise.all(
    files.map(
      (f) =>
        new Promise<NewAttachment>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve({ name: f.name, data: String(reader.result).split(",")[1] ?? "" });
          reader.onerror = () => reject(reader.error);
          reader.readAsDataURL(f);
        }),
    ),
  );
}

export interface UpdateInfo {
  version: string;
  notes?: string;
  install: (onProgress?: (pct: number) => void) => Promise<void>;
}

/** Check GitHub Releases for a newer signed build. Returns null when up to date or unsupported. */
export async function checkForUpdate(): Promise<UpdateInfo | null> {
  if (!isTauri) return null;
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check();
  if (!update) return null;
  return {
    version: update.version,
    notes: update.body,
    install: async (onProgress) => {
      let total = 0;
      let done = 0;
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        if (ev.event === "Progress") {
          done += ev.data.chunkLength;
          if (total) onProgress?.(Math.round((done / total) * 100));
        }
      });
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    },
  };
}
