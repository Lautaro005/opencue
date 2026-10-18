import { useEffect } from "react";
import { X } from "lucide-react";
import { useStore } from "./lib/store";
import { Sidebar } from "./components/Sidebar";
import { ChatView } from "./components/chat/ChatView";
import { AgentProfile } from "./components/panel/AgentProfile";
import { GroupDetails } from "./components/panel/GroupDetails";
import { ComputerPanel } from "./components/panel/ComputerPanel";
import { CreateAgentDialog, CreateGroupDialog } from "./components/dialogs/CreateDialogs";
import { SettingsModal } from "./components/settings/SettingsModal";
import { Spinner } from "./components/ui";
import { ConfirmHost, ContextMenuHost, TooltipHost } from "./components/ui/Overlays";
import { Walkthrough } from "./components/dialogs/Walkthrough";
import { checkForUpdate, onTrayAction } from "./lib/api";

export default function App() {
  const ready = useStore((s) => s.ready);
  const init = useStore((s) => s.init);
  const panel = useStore((s) => s.panel);
  const selected = useStore((s) => s.selected);
  const agent = useStore((s) => (s.selected ? s.agents[s.selected] : undefined));
  const group = useStore((s) => (s.selected ? s.groups[s.selected] : undefined));
  const toast = useStore((s) => s.toast);

  useEffect(() => {
    init()
      .then(() => {
        // First run: a new user (no chats, no name yet) gets the walkthrough once.
        const st = useStore.getState();
        if (st.settings.onboarded !== "true" && st.threads.length === 0 && !st.settings.user_name) st.setDialog("walkthrough");
        // Look for a signed update on GitHub once per launch, unless turned off.
        if (useStore.getState().settings.auto_update === "false") return;
        setTimeout(() => void checkForUpdate().then((update) => useStore.setState({ update })).catch(() => {}), 4000);
      })
      .catch((e) => useStore.getState().showToast(String(e)));
  }, [init]);

  // The menu-bar icon: Settings opens the settings window; Check for Updates runs a check there.
  useEffect(() => {
    let off = () => {};
    void onTrayAction((action) => {
      const st = useStore.getState();
      if (action === "settings") st.openSettings();
      if (action === "check-updates") {
        st.openSettings("about");
        useStore.setState((s) => ({ updateCheckNonce: s.updateCheckNonce + 1 }));
      }
    }).then((fn) => (off = fn));
    return () => off();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.key === ",") {
        e.preventDefault();
        useStore.getState().openSettings();
      }
      if (mod && e.key.toLowerCase() === "n") {
        e.preventDefault();
        useStore.getState().setDialog("create-agent");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  if (!ready) {
    return (
      <div className="flex h-full items-center justify-center text-muted">
        <Spinner size={20} />
      </div>
    );
  }

  return (
    <div className="flex h-full">
      <Sidebar />
      <ChatView />
      {panel === "profile" && agent && <AgentProfile key={selected} agent={agent} />}
      {panel === "group" && group && <GroupDetails key={selected} group={group} />}
      {panel === "computer" && agent && <ComputerPanel key={selected} agent={agent} />}
      <CreateAgentDialog />
      <CreateGroupDialog />
      <SettingsModal />
      <Walkthrough />
      <ConfirmHost />
      <TooltipHost />
      <ContextMenuHost />
      {toast && (
        <div role="status" className="animate-pop fixed bottom-24 left-1/2 z-[60] flex max-w-[560px] -translate-x-1/2 items-start gap-3 rounded-xl border border-line bg-elevated px-4 py-2.5 text-[13px] shadow-pop">
          <span className="selectable line-clamp-4 flex-1">{toast}</span>
          <button onClick={() => useStore.setState({ toast: null })} className="text-faint hover:text-ink" aria-label="Dismiss">
            <X size={14} />
          </button>
        </div>
      )}
    </div>
  );
}
