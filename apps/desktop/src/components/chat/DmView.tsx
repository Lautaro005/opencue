import { useTranslation } from "react-i18next";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { dmThreadId, useStore } from "../../lib/store";
import { AvatarView } from "../AvatarView";
import { Typing } from "../ui";
import { MessageList } from "./MessageList";

const NO_WORKERS: string[] = [];

/** The conversation two agents had with each other, opened from a "Chat with …" pill. */
export function DmView() {
  const { t } = useTranslation();
  const dm = useStore((s) => s.dm);
  const agents = useStore((s) => s.agents);
  const closeDm = useStore((s) => s.closeDm);
  const threadId = dm ? dmThreadId(dm.a, dm.b) : "";
  const count = useStore((s) => s.messages[threadId]?.length ?? 0);
  const working = useStore((s) => s.working[threadId] ?? NO_WORKERS);
  if (!dm) return null;
  const a = agents[dm.a];
  const b = agents[dm.b];
  if (!a || !b) return null;
  return (
    <main className="relative flex min-w-0 flex-1 flex-col bg-bg">
      <header className="flex h-13 shrink-0 items-center gap-2.5 border-b border-line px-4" data-tauri-drag-region>
        <AvatarView avatar={a.avatar} name={a.name} size={28} />
        <ChevronRight size={13} className="text-faint" />
        <AvatarView avatar={b.avatar} name={b.name} size={28} />
        <span className="ml-1 min-w-0 truncate text-[14px] font-semibold">
          {a.name} &amp; {b.name}
        </span>
        {working.length > 0 && <Typing />}
      </header>
      {count === 0 ? (
        <div className="flex flex-1 items-center justify-center text-[13px] text-muted">{t("chat.dmEmpty")}</div>
      ) : (
        <MessageList threadId={threadId} isGroup />
      )}
      <div className="pointer-events-none absolute inset-x-0 bottom-5 flex justify-center">
        <button
          onClick={closeDm}
          className="pointer-events-auto inline-flex items-center gap-1.5 rounded-full border border-line bg-elevated px-4 py-2 text-[14px] font-medium shadow-pop hover:bg-hover"
        >
          <ChevronLeft size={16} /> {t("chat.backTo", { name: a.name })}
        </button>
      </div>
    </main>
  );
}
