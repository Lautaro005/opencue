import { create } from "zustand";
import { api, onCoreEvent, filesToAttachments, type UpdateInfo } from "./api";
import { onMessageForVoice } from "./voice";
import type { Agent, ApprovalRequest, CoreEvent, Group, Message, ProviderConfig, ThreadSummary } from "./types";

export type Panel = "profile" | "group" | "computer" | null;
export type SettingsSection = "general" | "account" | "usage" | "models" | "apps" | "skills" | "computers" | "voice" | "internet" | "about";
export type Dialog = "create-agent" | "create-group" | null;

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
  toast: string | null;
  /** A newer release found by the updater, if any. */
  update: UpdateInfo | null;

  init: () => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: string | null) => void;
  setPanel: (p: Panel) => void;
  setDialog: (d: Dialog) => void;
  openSettings: (s?: SettingsSection) => void;
  closeSettings: () => void;
  loadMessages: (threadId: string) => Promise<void>;
  send: (threadId: string, text: string, files?: File[]) => Promise<void>;
  setSetting: (key: string, value: string) => Promise<void>;
  refreshProviders: () => Promise<void>;
  showToast: (text: string) => void;
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
  toast: null,
  update: null,

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
    set({ settings, ready: true });
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
    set({ selected: id, panel: nextPanel });
    try {
      if (id) localStorage.setItem("opencue.selected", id);
    } catch {}
    if (id) void get().loadMessages(id);
  },

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

  showToast: (text) => {
    set({ toast: text });
    setTimeout(() => {
      if (get().toast === text) set({ toast: null });
    }, 5000);
  },
}));

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
      break;
    case "approval_requested":
      set((s) => ({ approvals: { ...s.approvals, [ev.approval.message_id]: ev.approval } }));
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
