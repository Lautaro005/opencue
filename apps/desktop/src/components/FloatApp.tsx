import { useEffect } from "react";
import { useStore } from "../lib/store";
import { Spinner } from "./ui";
import { ConfirmHost, TooltipHost } from "./ui/Overlays";
import { ArtifactPanel } from "./panel/ArtifactPanel";
import { ComputerPanel } from "./panel/ComputerPanel";

/** What a floating window shows: one panel, taken out of the main window. */
export function FloatApp() {
  const ready = useStore((s) => s.ready);
  const init = useStore((s) => s.init);
  const toast = useStore((s) => s.toast);
  const agent = useStore((s) => (window.__FLOAT__ ? s.agents[window.__FLOAT__.id] : undefined));
  const f = window.__FLOAT__!;

  useEffect(() => {
    void init().then(() => {
      if (f.kind !== "computer") useStore.setState({ artifactView: { id: f.id }, panel: f.kind });
      else useStore.setState({ panel: "computer" });
    });
  }, [init, f.kind, f.id]);

  if (!ready) {
    return (
      <div className="flex h-full items-center justify-center text-muted">
        <Spinner size={20} />
      </div>
    );
  }
  return (
    <div className="h-full">
      {f.kind === "computer" ? agent && <ComputerPanel agent={agent} /> : <ArtifactPanel />}
      <ConfirmHost />
      <TooltipHost />
      {toast && <div role="status" className="fixed bottom-4 left-1/2 z-[60] max-w-[90%] -translate-x-1/2 rounded-xl border border-line bg-elevated px-4 py-2 text-[13px] shadow-pop">{toast}</div>}
    </div>
  );
}
