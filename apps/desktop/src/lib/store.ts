import { create } from "zustand";
import { api, onCoreEvent, filesToAttachments, type UpdateInfo } from "./api";
import { onMessageForVoice } from "./voice";
import type { Agent, ApprovalRequest, CoreEvent, Group, Message, ProviderConfig, Question, ThreadSummary } from "./types";

export type Panel = "profile" | "group" | "computer" | null;
export type SettingsSection = "general" | "account" | "usage" | "providers" | "models" | "apps" | "skills" | "computers" | "notifications" | "voice" | "internet" | "about";
export type Dialog = "create-agent" | "create-group" | "walkthrough" | null;

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
  toast: string | null;
  /** Models each provider reports, by provider id. */
  models: Record<string, ModelList>;
  /** A newer release found by the updater, if any. */
  update: UpdateInfo | null;
  /** Bumped by the menu-bar "Check for Updates"; the About page runs a check when it changes. */
  updateCheckNonce: number;

  init: () => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: string | null) => void;
  openDm: (a: string, b: string) => void;
  closeDm: () => void;
  setPanel: (p: Panel) => void;
  setDialog: (d: Dialog) => void;
  openSettings: (s?: SettingsSection) => void;
  closeSettings: () => void;
  loadMessages: (threadId: string) => Promise<void>;
  send: (threadId: string, text: string, files?: File[]) => Promise<void>;
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
  dialog: null,
  settingsSection: null,
  messages: {},
  working: {},
  approvals: {},
  questions: {},
  dm: null,
  toast: null,
  models: {},
  update: null,
  updateCheckNonce: 0,

  init: async () => {
    if (!listening) {
      listening = true;
      await onCoreEvent((ev) => handleEvent(ev, set, get));
    }
    const [settings] = await Promise.all([api.getSettings(), get().refresh(), get().refreshProviders()]);
    let selected: string | null = null;
    try {
      selected = localStorage.getItem("opencue.selected");
    } catch {}
    const threads = get().threads;
    if (!selected || !threads.some((t) => t.id === selected)) selected = threads[0]?.id ?? null;
    const pending = await api.pendingQuestions().catch(() => [] as Question[]);
    set({ settings, ready: true, questions: Object.fromEntries(pending.map((q) => [q.id, q])) });
    get().select(selected);
  },

  refresh: async () => {
    const [threads, agents, groups] = await Promise.all([api.listThreads(), api.listAgents(), api.listGroups()]);
    set({
      threads,
      agents: Object.fromEntries(agents.map((a) => [a.id, a])),
      groups: Object.fromEntries(groups.map((g) => [g.id, g])),
    });
    const sel = get().selected;
    if (sel && !threads.some((t) => t.id === sel)) get().select(threads[0]?.id ?? null);
  },

  refreshProviders: async () => set({ providers: await api.listProviders() }),

  select: (id) => {
    const { panel, groups } = get();
    const isGroup = !!(id && groups[id]);
    // Keep the right panel open but switch it to the matching kind.
    const nextPanel: Panel = panel ? (isGroup ? "group" : panel === "computer" ? "computer" : "profile") : null;
    set({ selected: id, panel: nextPanel, dm: null });
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
  setPanel: (panel) => set({ panel }),
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
    case "automations_changed":
      window.dispatchEvent(new CustomEvent("opencue:automations-changed"));
      break;
    case "computer_output":
      window.dispatchEvent(new CustomEvent("opencue:computer-output", { detail: ev }));
      break;
  }
}
