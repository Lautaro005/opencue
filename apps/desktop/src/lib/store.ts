import { create } from "zustand";
import { api, onCoreEvent, filesToAttachments, type UpdateInfo } from "./api";
import { onMessageForVoice } from "./voice";
import i18n from "../i18n";
import type { Agent, ApprovalRequest, CoreEvent, Group, Message, ProviderConfig, Question, TermInfo, ThreadSummary } from "./types";

export type Panel = "profile" | "group" | "computer" | "artifact" | "canvas" | null;
export type SettingsSection = "general" | "account" | "usage" | "providers" | "models" | "apps" | "messenger" | "skills" | "computers" | "notifications" | "voice" | "internet" | "about";
export type Dialog = "create-agent" | "create-group" | "templates" | "walkthrough" | null;

export interface ModelList {
  list: string[];
  error: string | null;
  loading: boolean;
}

interface State {
  ready: boolean;
  threads: ThreadSummary[];
  agents: Record<string, Agent>;
  groups: Record<string, Group>;
  providers: ProviderConfig[];
  settings: Record<string, string>;
  selected: string | null;
  panel: Panel;
  /** The artifact shown in the side panel. */
  artifactView: { id: string; version?: number } | null;
  dialog: Dialog;
  settingsSection: SettingsSection | null;
  messages: Record<string, Message[]>;
  /** thread id → agent ids currently working */
  working: Record<string, string[]>;
  /** tool message id → pending approval */
  approvals: Record<string, ApprovalRequest>;
  /** question id → question an agent is waiting on */
  questions: Record<string, Question>;
  /** The conversation between two agents being looked at, if any (the chat it came from is `a`). */
  dm: { a: string; b: string } | null;
  searchOpen: boolean;
  /** A message to scroll to once its chat is open (from the search). */
  focusMessage: string | null;
  toast: string | null;
  /** The voice call in progress, if any. */
  call: { threadId: string; agentId: string } | null;
  /** Models each provider reports, by provider id. */
  models: Record<string, ModelList>;
  /** A newer release found by the updater, if any. */
  update: UpdateInfo | null;
  /** Bumped by the menu-bar "Check for Updates"; the About page runs a check when it changes. */
  updateCheckNonce: number;
  /** Tool calls (artifacts) a model is still writing, by assistant message id. */
  toolProgress: Record<string, { tool: string; title: string | null; chars: number }>;
  /** Terminal sessions (Claude Code, Codex…) and the one on screen. */
  terminals: TermInfo[];
  selectedTerm: string | null;
  refreshTerminals: () => Promise<void>;
  selectTerm: (id: string | null) => void;
  /** Local model downloads, by model id. */
  modelProgress: Record<string, { phase: string; done: number; total: number; error: string | null }>;

  init: () => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: string | null) => void;
  startCall: (agentId: string) => void;
  endCall: () => void;
  openDm: (a: string, b: string) => void;
  setSearchOpen: (open: boolean) => void;
  openMessage: (threadId: string, messageId: string) => Promise<void>;
  closeDm: () => void;
  setPanel: (p: Panel) => void;
  openArtifact: (id: string, version?: number, canvas?: boolean) => void;
  /** Text to put in the composer when the next chat opens (e.g. "/dashboard "). */
  prefill: string | null;
  setPrefill: (text: string | null) => void;
  setDialog: (d: Dialog) => void;
  openSettings: (s?: SettingsSection) => void;
  closeSettings: () => void;
  loadMessages: (threadId: string) => Promise<void>;
  send: (threadId: string, text: string, files?: File[]) => Promise<void>;
  /** Answer the last user message again. */
  regenerate: (threadId: string) => Promise<void>;
  /** Edit the last user message and answer it again. */
  editLast: (threadId: string, text: string) => Promise<void>;
  setSetting: (key: string, value: string) => Promise<void>;
  refreshProviders: () => Promise<void>;
  loadModels: (providerId: string, force?: boolean) => Promise<void>;
  setModelHidden: (providerId: string, model: string, hidden: boolean) => Promise<void>;
  showToast: (text: string) => void;
}

/** `hidden_models` setting: provider id → models the user turned off. */
export function parseHidden(raw: string | undefined): Record<string, string[]> {
  try {
    const v = raw ? JSON.parse(raw) : {};
    return v && typeof v === "object" ? v : {};
  } catch {
    return {};
  }
}

// Set synchronously so StrictMode's double init can't register two listeners.
let listening = false;

export const useStore = create<State>((set, get) => ({
  ready: false,
  threads: [],
  agents: {},
  groups: {},
  providers: [],
  settings: {},
  selected: null,
  panel: null,
  artifactView: null,
  prefill: null,
  dialog: null,
  settingsSection: null,
  messages: {},
  working: {},
  approvals: {},
  questions: {},
  dm: null,
  searchOpen: false,
  focusMessage: null,
  toast: null,
  call: null,
  models: {},
  update: null,
  updateCheckNonce: 0,
  toolProgress: {},
  modelProgress: {},
  terminals: [],
  selectedTerm: null,

  init: async () => {
    if (!listening) {
      listening = true;
      await onCoreEvent((ev) => handleEvent(ev, set, get));
    }
    const [settings] = await Promise.all([api.getSettings(), get().refresh(), get().refreshProviders(), get().refreshTerminals()]);
    const pending = await api.pendingQuestions().catch(() => [] as Question[]);
    set({ settings, ready: true, questions: Object.fromEntries(pending.map((q) => [q.id, q])) });
    // The dashboard is home: the app opens on it, and any chat is one click away.
    get().select(null);
  },

  refresh: async () => {
    const [threads, agents, groups] = await Promise.all([api.listThreads(), api.listAgents(), api.listGroups()]);
    set({
      threads,
      agents: Object.fromEntries(agents.map((a) => [a.id, a])),
      groups: Object.fromEntries(groups.map((g) => [g.id, g])),
    });
    const sel = get().selected;
    if (sel && !threads.some((t) => t.id === sel)) get().select(null);
  },

  refreshTerminals: async () => {
    const list = await api.termList().catch(() => [] as TermInfo[]);
    set((s) => ({ terminals: list, selectedTerm: s.selectedTerm && list.some((t) => t.id === s.selectedTerm) ? s.selectedTerm : null }));
  },
  selectTerm: (id) => set(id ? { selectedTerm: id, selected: null, panel: null, dm: null } : { selectedTerm: null }),

  refreshProviders: async () => set({ providers: await api.listProviders() }),

  select: (id) => {
    const { panel, groups } = get();
    const isGroup = !!(id && groups[id]);
    // Keep the right panel open but switch it to the matching kind.
    const nextPanel: Panel = panel && panel !== "artifact" && panel !== "canvas" ? (isGroup ? "group" : panel === "computer" ? "computer" : "profile") : null;
    // A coding group's panel (sessions, changes, pull requests) is part of working in it.
    const coding = isGroup && groups[id!]?.kind === "coding";
    set({ selected: id, panel: coding && nextPanel === null ? "group" : nextPanel, dm: null, selectedTerm: id ? null : get().selectedTerm });
    try {
      if (id) localStorage.setItem("opencue.selected", id);
    } catch {}
    if (id) void get().loadMessages(id);
  },

  openDm: (a, b) => {
    set({ dm: { a, b } });
    void get().loadMessages(dmThreadId(a, b));
  },
  closeDm: () => set({ dm: null }),
  setSearchOpen: (searchOpen) => set({ searchOpen }),
  openMessage: async (threadId, messageId) => {
    get().select(threadId);
    // The hit may be older than what a chat loads by default.
    const list = await api.getMessages(threadId, 5000);
    set((s) => ({ messages: { ...s.messages, [threadId]: list }, focusMessage: messageId }));
  },
  startCall: (agentId) => set({ call: { threadId: agentId, agentId } }),
  endCall: () => set({ call: null }),
  setPanel: (panel) => set({ panel }),
  setPrefill: (prefill) => set({ prefill }),
  openArtifact: (id, version) => set({ panel: "artifact", artifactView: { id, version } }),
  setDialog: (dialog) => set({ dialog }),
  openSettings: (s = "general") => set({ settingsSection: s }),
  closeSettings: () => set({ settingsSection: null }),

  loadMessages: async (threadId) => {
    const list = await api.getMessages(threadId);
    set((s) => ({ messages: { ...s.messages, [threadId]: list } }));
  },

  send: async (threadId, text, files = []) => {
    try {
      const attachments = files.length ? await filesToAttachments(files) : undefined;
      await api.sendMessage(threadId, text, attachments);
    } catch (e) {
      get().showToast(String(e));
    }
  },

  regenerate: async (threadId) => {
    try {
      await api.regenerateLast(threadId);
      await get().loadMessages(threadId);
    } catch (e) {
      get().showToast(String(e));
    }
  },

  editLast: async (threadId, text) => {
    try {
      await api.regenerateLast(threadId, text);
      await get().loadMessages(threadId);
    } catch (e) {
      get().showToast(String(e));
    }
  },

  setSetting: async (key, value) => {
    await api.setSetting(key, value);
    set((s) => ({ settings: { ...s.settings, [key]: value } }));
  },

  loadModels: async (providerId, force = false) => {
    const cur = get().models[providerId];
    if (!force && (cur?.loading || (cur && !cur.error && cur.list.length))) return;
    set((s) => ({ models: { ...s.models, [providerId]: { list: cur?.list ?? [], error: null, loading: true } } }));
    try {
      const list = await api.listModels(providerId);
      set((s) => ({ models: { ...s.models, [providerId]: { list, error: null, loading: false } } }));
    } catch (e) {
      set((s) => ({ models: { ...s.models, [providerId]: { list: [], error: String(e), loading: false } } }));
    }
  },

  setModelHidden: async (providerId, model, hidden) => {
    const map = parseHidden(get().settings.hidden_models);
    const cur = new Set(map[providerId] ?? []);
    if (hidden) cur.add(model);
    else cur.delete(model);
    map[providerId] = [...cur];
    await get().setSetting("hidden_models", JSON.stringify(map));
  },

  showToast: (text) => {
    set({ toast: text });
    setTimeout(() => {
      if (get().toast === text) set({ toast: null });
    }, 5000);
  },
}));

/** Id of the conversation two agents share (the same as the core's). */
export function dmThreadId(a: string, b: string): string {
  return a <= b ? `dm_${a}_${b}` : `dm_${b}_${a}`;
}

function upsertMessage(list: Message[] | undefined, m: Message): Message[] {
  const cur = list ?? [];
  const i = cur.findIndex((x) => x.id === m.id);
  if (i === -1) return [...cur, m];
  const next = cur.slice();
  next[i] = m;
  return next;
}

let refreshTimer: ReturnType<typeof setTimeout> | null = null;
function scheduleRefresh(get: () => State) {
  if (refreshTimer) clearTimeout(refreshTimer);
  refreshTimer = setTimeout(() => void get().refresh(), 60);
}

function handleEvent(ev: CoreEvent, set: (fn: (s: State) => Partial<State>) => void, get: () => State) {
  switch (ev.type) {
    case "message": {
      const m = ev.message;
      // An agent just made something: show it next to the chat.
      const made = m.attachments?.find((a) => a.artifact);
      if (made?.artifact && get().selected === m.thread_id && !get().messages[m.thread_id]?.some((x) => x.id === m.id)) {
        // The card in the chat opens it with a click; "open automatically" in Settings skips the click.
        if (get().settings.artifact_auto_open === "true") get().openArtifact(made.artifact.id, made.artifact.version, made.artifact.canvas);
        window.dispatchEvent(new CustomEvent("opencue:artifact-changed", { detail: { id: made.artifact.id } }));
      }
      set((s) => (s.messages[m.thread_id] ? { messages: { ...s.messages, [m.thread_id]: upsertMessage(s.messages[m.thread_id], m) } } : {}));
      if (m.status !== "awaiting_approval") {
        set((s) => {
          if (!s.approvals[m.id]) return {};
          const { [m.id]: _, ...rest } = s.approvals;
          return { approvals: rest };
        });
      }
      if (m.role !== "tool" && m.status !== "streaming") scheduleRefresh(get);
      onMessageForVoice(m, get().settings.voice_auto_speak !== "false");
      break;
    }
    case "term_output":
      window.dispatchEvent(new CustomEvent("opencue:term-output", { detail: ev }));
      break;
    case "term_exit":
      window.dispatchEvent(new CustomEvent("opencue:term-exit", { detail: ev }));
      void get().refreshTerminals();
      break;
    case "tool_progress":
      set((s) => ({ toolProgress: { ...s.toolProgress, [ev.message_id]: { tool: ev.tool, title: ev.title, chars: ev.chars } } }));
      break;
    case "model_progress":
      set((s) => ({ modelProgress: { ...s.modelProgress, [ev.id]: { phase: ev.phase, done: ev.done, total: ev.total, error: ev.error } } }));
      window.dispatchEvent(new CustomEvent("opencue:models-changed"));
      break;
    case "delta":
      set((s) => {
        const list = s.messages[ev.thread_id];
        if (!list) return {};
        const i = list.findIndex((x) => x.id === ev.message_id);
        if (i === -1) return {};
        const next = list.slice();
        next[i] = { ...next[i], content: next[i].content + ev.text };
        return { messages: { ...s.messages, [ev.thread_id]: next } };
      });
      break;
    case "busy":
      set((s) => {
        const cur = new Set(s.working[ev.thread_id] ?? []);
        if (ev.busy) cur.add(ev.agent_id);
        else cur.delete(ev.agent_id);
        return { working: { ...s.working, [ev.thread_id]: [...cur] } };
      });
      if (!ev.busy) {
        // An agent that stopped working has nothing left to wait for.
        set((s) => ({ questions: Object.fromEntries(Object.entries(s.questions).filter(([, q]) => q.agent_id !== ev.agent_id)) }));
      }
      break;
    case "approval_requested":
      set((s) => ({ approvals: { ...s.approvals, [ev.approval.message_id]: ev.approval } }));
      break;
    case "question_requested":
      set((s) => ({ questions: { ...s.questions, [ev.question.id]: ev.question } }));
      break;
    case "question_resolved":
      set((s) => {
        const { [ev.question_id]: _, ...rest } = s.questions;
        return { questions: rest };
      });
      break;
    case "approval_resolved":
      set((s) => ({
        approvals: Object.fromEntries(Object.entries(s.approvals).filter(([, a]) => a.id !== ev.approval_id)),
      }));
      break;
    case "threads_changed":
      scheduleRefresh(get);
      void get().refreshTerminals();
      break;
    case "error":
      get().showToast(ev.message);
      break;
    case "profile_changed":
      window.dispatchEvent(new CustomEvent("opencue:profile-changed", { detail: ev.agent_id }));
      break;
    case "mcp_changed":
      window.dispatchEvent(new CustomEvent("opencue:mcp-changed"));
      break;
    case "dashboard_changed":
      window.dispatchEvent(new CustomEvent("opencue:dashboard-changed"));
      break;
    case "automations_changed":
      window.dispatchEvent(new CustomEvent("opencue:automations-changed"));
      break;
    case "computer_output": {
      // A command started on the open agent's computer: show the terminal / VM next to the chat.
      const s = get();
      if (ev.text.startsWith("$ ") && s.selected === ev.agent_id && s.panel !== "computer" && s.settings.auto_open_computer !== "false") {
        set(() => ({ panel: "computer" as Panel }));
        window.dispatchEvent(new CustomEvent("opencue:computer-focus"));
      }
      window.dispatchEvent(new CustomEvent("opencue:computer-output", { detail: ev }));
      break;
    }
    case "messenger_changed":
      window.dispatchEvent(new CustomEvent("opencue:messenger-changed"));
      break;
    case "browser_activity": {
      // An agent is using the browser: open the Computer panel on it.
      const s = get();
      if (s.selected === ev.agent_id && s.settings.auto_open_computer !== "false") {
        if (s.panel !== "computer") set(() => ({ panel: "computer" as Panel }));
        window.dispatchEvent(new CustomEvent("opencue:browser-focus"));
      }
      break;
    }
    case "catalog_changed": {
      window.dispatchEvent(new CustomEvent("opencue:catalog-changed"));
      get().showToast(ev.connected ? i18n.t("settings.catalog.connected", { name: ev.slug }) : i18n.t("settings.catalog.failed", { error: ev.error ?? "" }));
      // The sign-in finished in the browser: bring the app back to the front.
      void import("@tauri-apps/api/window").then((w) => w.getCurrentWindow().setFocus()).catch(() => {});
      break;
    }
  }
}
