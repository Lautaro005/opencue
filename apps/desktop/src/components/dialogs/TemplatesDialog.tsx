import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Download, LayoutTemplate } from "lucide-react";
import { api } from "../../lib/api";
import { cx } from "../../lib/format";
import { useStore } from "../../lib/store";
import { AGENT_TEMPLATES, CATEGORIES, GROUP_TEMPLATES, type TemplateCategory } from "../../lib/templates";
import { avatarFor, createAgentFromGrok, createAgentFromTemplate, createGroupFromTemplate } from "../../lib/templateActions";
import type { GrokTemplate } from "../../lib/types";
import { AvatarView, GroupAvatar } from "../AvatarView";
import { Button, DialogHeader, Input, Modal, Spinner } from "../ui";

/** Templates in a floating window: ready-made agents and groups, and an import from x.ai. */
export function TemplatesDialog() {
  const { t } = useTranslation();
  const open = useStore((s) => s.dialog === "templates");
  const close = () => useStore.getState().setDialog(null);
  const showToast = useStore((s) => s.showToast);
  const [tab, setTab] = useState<"agents" | "groups">("agents");
  const [category, setCategory] = useState<TemplateCategory | "all">("all");
  const [busy, setBusy] = useState<string | null>(null);
  const [link, setLink] = useState("");
  const [found, setFound] = useState<GrokTemplate | null>(null);
  const [importing, setImporting] = useState(false);

  const done = async (id: string) => {
    await useStore.getState().refresh();
    useStore.getState().select(id);
    close();
  };

  const useAgent = async (key: string) => {
    const tpl = AGENT_TEMPLATES.find((x) => x.key === key);
    if (!tpl) return;
    setBusy(key);
    try {
      await done((await createAgentFromTemplate(tpl)).id);
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const useGroup = async (key: string) => {
    const g = GROUP_TEMPLATES.find((x) => x.key === key);
    if (!g) return;
    setBusy(key);
    try {
      await done(await createGroupFromTemplate(g));
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const doImport = async () => {
    if (!link.trim()) return;
    setImporting(true);
    setFound(null);
    try {
      setFound(await api.importGrokTemplate(link.trim()));
    } catch (e) {
      showToast(String(e));
    } finally {
      setImporting(false);
    }
  };

  const addImported = async () => {
    if (!found) return;
    setBusy("import");
    try {
      const r = await createAgentFromGrok(found);
      showToast(t("templates.imported", { skills: r.skills, memories: r.memories }));
      await done(r.agent.id);
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  const agents = AGENT_TEMPLATES.filter((x) => category === "all" || x.category === category);
  const groups = GROUP_TEMPLATES.filter((x) => category === "all" || x.category === category);

  return (
    <Modal open={open} onClose={close} className="w-[720px]" label={t("templates.title")}>
      <DialogHeader title={t("templates.title")} subtitle={t("templates.hint")} onClose={close} />
      <div className="flex max-h-[calc(100vh-150px)] flex-col gap-4 overflow-y-auto px-6 pt-2 pb-5">
        <div className="rounded-xl border border-line p-3">
          <div className="mb-1.5 flex items-center gap-1.5 text-[12.5px] font-medium text-muted">
            <Download size={13} /> {t("templates.importTitle")}
          </div>
          <div className="flex gap-2">
            <Input value={link} onChange={(e) => setLink(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void doImport()} placeholder="https://x.ai/bot/…" spellCheck={false} />
            <Button onClick={() => void doImport()} disabled={!link.trim() || importing}>
              {importing ? <Spinner size={13} /> : t("templates.import")}
            </Button>
          </div>
          <p className="mt-1.5 text-[11.5px] text-muted">{t("templates.importHint")}</p>
          {found && (
            <div className="mt-3 rounded-xl bg-hover p-3 text-[13px]">
              <div className="font-semibold">
                {found.name} {found.creator && <span className="font-normal text-muted">· {found.creator}</span>}
              </div>
              <p className="mt-0.5 line-clamp-3 text-[12.5px] text-muted">{found.description}</p>
              <div className="mt-2 flex flex-wrap gap-1.5 text-[11.5px]">
                {found.source === "marketplace" ? (
                  <>
                    <Chip>{t("templates.skills", { count: found.skills.length })}</Chip>
                    <Chip>{t("templates.memories", { count: found.memories.length })}</Chip>
                    <Chip>{t("templates.routines", { count: found.routines.length })}</Chip>
                    <Chip>{t("templates.integrations", { count: found.integrations.length })}</Chip>
                  </>
                ) : (
                  <Chip>{t("templates.sharedOnly")}</Chip>
                )}
              </div>
              <div className="mt-3 flex justify-end">
                <Button variant="primary" size="sm" onClick={() => void addImported()} disabled={busy !== null}>
                  {busy === "import" ? <Spinner size={13} /> : t("templates.addAgent")}
                </Button>
              </div>
            </div>
          )}
        </div>

        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-1.5">
            {(["agents", "groups"] as const).map((k) => (
              <button key={k} onClick={() => setTab(k)} className={cx("rounded-lg px-3 py-1 text-[13px] font-medium", tab === k ? "bg-active text-ink" : "text-muted hover:bg-hover")}>
                {t(`templates.${k}`)}
              </button>
            ))}
          </div>
          <div className="flex flex-wrap items-center gap-1.5">
            {(["all", ...CATEGORIES] as const).map((c) => (
              <button key={c} onClick={() => setCategory(c)} className={cx("rounded-full border px-2.5 py-0.5 text-[12px]", category === c ? "border-accent bg-accent-soft text-ink" : "border-line text-muted hover:bg-hover")}>
                {t(`templates.cat.${c}`)}
              </button>
            ))}
          </div>
        </div>

        {tab === "agents" ? (
          <div className="grid grid-cols-2 gap-2.5">
            {agents.map((a) => (
              <button key={a.key} onClick={() => void useAgent(a.key)} disabled={busy !== null} className="flex items-start gap-3 rounded-xl border border-line p-3 text-left transition-colors hover:bg-hover disabled:opacity-60">
                <AvatarView avatar={avatarFor(a)} name={a.name} size={38} />
                <span className="min-w-0 flex-1">
                  <span className="flex items-center gap-1.5 text-[13.5px] font-semibold">
                    {a.name} {busy === a.key && <Spinner size={12} />}
                  </span>
                  <span className="line-clamp-2 block text-[12px] leading-snug text-muted">{a.summary}</span>
                </span>
              </button>
            ))}
          </div>
        ) : (
          <div className="grid grid-cols-2 gap-2.5">
            {groups.map((g) => {
              const members = g.members.map((k) => AGENT_TEMPLATES.find((x) => x.key === k)!).filter(Boolean);
              return (
                <button key={g.key} onClick={() => void useGroup(g.key)} disabled={busy !== null} className="flex items-start gap-3 rounded-xl border border-line p-3 text-left transition-colors hover:bg-hover disabled:opacity-60">
                  <GroupAvatar avatars={members.map(avatarFor)} size={38} />
                  <span className="min-w-0 flex-1">
                    <span className="flex items-center gap-1.5 text-[13.5px] font-semibold">
                      {g.name} {busy === g.key && <Spinner size={12} />}
                    </span>
                    <span className="line-clamp-1 block text-[12px] text-muted">{g.summary}</span>
                    <span className="mt-0.5 line-clamp-2 block text-[11.5px] text-faint">{members.map((m) => m.name).join(" · ")}</span>
                  </span>
                </button>
              );
            })}
          </div>
        )}
        <p className="flex items-center gap-1.5 text-[11.5px] text-faint">
          <LayoutTemplate size={12} /> {t("templates.credit")}
        </p>
      </div>
    </Modal>
  );
}

function Chip({ children }: { children: React.ReactNode }) {
  return <span className="rounded-full border border-line px-2 py-0.5 text-muted">{children}</span>;
}
