import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, FolderOpen } from "lucide-react";
import { useStore } from "../../lib/store";
import { api, pickFolder } from "../../lib/api";
import { greetNewAgent } from "../../lib/greet";
import type { Avatar, GroupKind, TermTool } from "../../lib/types";
import { CLI_AGENT_TOOLS, createCliAgent } from "../../lib/coding";
import { createGroupFromTemplate } from "../../lib/templateActions";
import { TermIcon } from "../terminal/TermIcon";
import { cx } from "../../lib/format";
import { AvatarView } from "../AvatarView";
import { AvatarPickerDialog, randomAvatar } from "../panel/AvatarPicker";
import { ModelPicker } from "../ModelPicker";
import { isConfigured } from "../../lib/models";
import { Button, DialogHeader, Field, Input, Modal, Spinner, Textarea } from "../ui";

export function CreateAgentDialog() {
  const { t } = useTranslation();
  const open = useStore((s) => s.dialog === "create-agent");
  const close = () => useStore.getState().setDialog(null);
  const providers = useStore((s) => s.providers);
  const settings = useStore((s) => s.settings);
  const [name, setName] = useState("");
  const [instructions, setInstructions] = useState("");
  const [avatar, setAvatar] = useState<Avatar>(randomAvatar);
  const [providerId, setProviderId] = useState<string | null>(null);
  const [model, setModel] = useState<string | null>(null);
  const [showPicker, setShowPicker] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setName("");
    setInstructions("");
    setAvatar(randomAvatar());
    setShowPicker(false);
    setModel(null);
    void api.providerCatalog().then((c) => {
      const usable = providers.filter((p) => isConfigured(p, c.find((x) => x.kind === p.kind)));
      setProviderId(settings.default_provider && usable.some((p) => p.id === settings.default_provider) ? settings.default_provider : usable[0]?.id ?? null);
    });
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  const submit = async () => {
    if (!name.trim()) return;
    setBusy(true);
    try {
      const agent = await api.createAgent({ name: name.trim(), avatar, instructions, provider_id: providerId, model });
      await greetNewAgent(agent, instructions.trim().length > 0);
      await useStore.getState().refresh();
      useStore.getState().select(agent.id);
      close();
    } catch (e) {
      useStore.getState().showToast(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <Modal open={open} onClose={close} className="w-[480px]" label={t("dialogs.createAgent")}>
        <DialogHeader title={t("dialogs.createAgent")} subtitle={t("dialogs.createAgentHint")} onClose={close} />
        <div className="max-h-[calc(100vh-150px)] space-y-4 overflow-y-auto px-6 pt-3 pb-5">
          <div className="flex items-center gap-4">
            <button onClick={() => setShowPicker(true)} className="rounded-full transition-transform hover:scale-105" aria-label={t("profile.pickAvatar")} data-tip={t("profile.pickAvatar")}>
              <AvatarView avatar={avatar} name={name} size={60} />
            </button>
            <div className="flex-1">
              <Field label={t("dialogs.name")}>
                <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void submit()} placeholder={t("dialogs.namePlaceholder")} />
              </Field>
            </div>
          </div>
          <Field label={t("dialogs.instructions")}>
            <Textarea value={instructions} onChange={(e) => setInstructions(e.target.value)} placeholder={t("dialogs.instructionsPlaceholder")} rows={4} />
          </Field>
          <div>
            <div className="mb-1.5 text-[12.5px] font-medium text-muted">{t("dialogs.provider")}</div>
            <ModelPicker providerId={providerId} model={model} onChange={(p, m) => (setProviderId(p), setModel(m))} />
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <Button onClick={close}>{t("common.cancel")}</Button>
            <Button variant="primary" onClick={() => void submit()} disabled={!name.trim() || busy}>
              {t("common.create")}
            </Button>
          </div>
        </div>
      </Modal>
      <AvatarPickerDialog open={open && showPicker} value={avatar} onChange={setAvatar} onClose={() => setShowPicker(false)} />
    </>
  );
}

export function CreateGroupDialog() {
  const { t } = useTranslation();
  const open = useStore((s) => s.dialog === "create-group");
  const close = () => useStore.getState().setDialog(null);
  const agents = useStore((s) => s.agents);
  const [name, setName] = useState("");
  const [picked, setPicked] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [kind, setKind] = useState<GroupKind>("chat");
  const [repo, setRepo] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setName("");
      setPicked([]);
      setKind("chat");
      setRepo(null);
    }
  }, [open]);

  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));

  const submit = async () => {
    if (!name.trim() || picked.length === 0) return;
    setBusy(true);
    try {
      const g = await api.createGroup(name.trim(), picked, kind);
      if (kind === "coding" && repo) await api.codingSetRepo(g.id, repo, false).catch((e) => useStore.getState().showToast(String(e)));
      await useStore.getState().refresh();
      useStore.getState().select(g.id);
      close();
    } catch (e) {
      useStore.getState().showToast(String(e));
    } finally {
      setBusy(false);
    }
  };

  const list = Object.values(agents);
  return (
    <Modal open={open} onClose={close} className="w-[460px]" label={t("dialogs.createGroup")}>
      <DialogHeader title={t("dialogs.createGroup")} subtitle={t("dialogs.createGroupHint")} onClose={close} />
      <div className="space-y-4 px-6 pt-3 pb-5">
        <Field label={t("dialogs.groupName")}>
          <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder={t("dialogs.groupNamePlaceholder")} />
        </Field>
        <div>
          <div className="mb-1.5 text-[12.5px] font-medium text-muted">{t("dialogs.groupKind")}</div>
          <div className="grid grid-cols-3 gap-1.5" role="radiogroup">
            {(["chat", "council", "coding"] as const).map((k) => (
              <button key={k} role="radio" aria-checked={kind === k} onClick={() => setKind(k)} className={cx("rounded-xl border px-2.5 py-2 text-left", kind === k ? "border-accent bg-accent-soft" : "border-line hover:bg-hover")}>
                <span className="block text-[13px] font-medium">{t(`dialogs.kind.${k}`)}</span>
                <span className="block text-[11.5px] leading-snug text-muted">{t(`dialogs.kind.${k}Hint`)}</span>
              </button>
            ))}
          </div>
          {kind === "coding" && (
            <div className="mt-2 flex items-center gap-2">
              <span className="selectable min-w-0 flex-1 truncate rounded-lg bg-hover px-3 py-1.5 font-mono text-[12px]">{repo ?? t("coding.noRepo")}</span>
              <Button onClick={() => void pickFolder().then((d) => d && setRepo(d))}>{t("coding.choose")}</Button>
            </div>
          )}
        </div>
        <div>
          <div className="mb-1.5 text-[12.5px] font-medium text-muted">{t("dialogs.pickMembers")}</div>
          {list.length === 0 ? (
            <p className="text-[12.5px] text-faint">{t("sidebar.emptyHint")}</p>
          ) : (
            <div className="max-h-64 space-y-0.5 overflow-y-auto rounded-xl border border-line p-1">
              {list.map((a) => {
                const on = picked.includes(a.id);
                return (
                  <button key={a.id} onClick={() => toggle(a.id)} className={cx("flex w-full items-center gap-2.5 rounded-lg px-2 py-1.5 text-left", on ? "bg-accent-soft" : "hover:bg-hover")}>
                    <AvatarView avatar={a.avatar} name={a.name} size={28} />
                    <span className="flex-1 text-[13.5px] font-medium">{a.name}</span>
                    {picked[0] === a.id && <span className="text-[11px] text-accent">{t("group.owner")}</span>}
                    <span className={cx("flex size-4.5 items-center justify-center rounded-md border", on ? "border-accent bg-accent text-white" : "border-line-strong")}>
                      {on && <Check size={12} strokeWidth={3} />}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
        <div className="flex justify-end gap-2 pt-1">
          <Button onClick={close}>{t("common.cancel")}</Button>
          <Button variant="primary" onClick={() => void submit()} disabled={!name.trim() || picked.length === 0 || busy}>
            {t("common.create")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/** A coding team from a template: the name of the app and its project folder, in a floating window. */
export function CreateCodingDialog() {
  const { t } = useTranslation();
  const open = useStore((s) => s.dialog === "create-coding");
  const template = useStore((s) => s.codingTemplate);
  const close = () => useStore.getState().setDialog(null);
  const [name, setName] = useState("");
  const [dir, setDir] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (open) {
      setName("");
      setDir("");
    }
  }, [open]);

  const browse = async () => {
    const d = await pickFolder();
    if (!d) return;
    setDir(d);
    // The folder's own name is a fair name for the app.
    if (!name.trim()) setName(d.split(/[\\/]/).filter(Boolean).slice(-1)[0] ?? "");
  };

  const submit = async () => {
    if (!template || !name.trim() || !dir.trim()) return;
    setBusy(true);
    try {
      const id = await createGroupFromTemplate(template, { name, dir: dir.trim() });
      await useStore.getState().refresh();
      useStore.getState().select(id);
      close();
    } catch (e) {
      useStore.getState().showToast(String(e));
      await useStore.getState().refresh();
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal open={open && !!template} onClose={close} className="w-[480px]" label={t("coding.newTitle")}>
      <DialogHeader title={t("coding.newTitle")} subtitle={t("coding.newHint")} onClose={close} />
      <div className="space-y-4 px-6 pt-3 pb-5">
        <Field label={t("coding.appName")}>
          <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder={t("coding.appNamePlaceholder")} onKeyDown={(e) => e.key === "Enter" && void submit()} />
        </Field>
        <Field label={t("coding.projectFolder")}>
          <div className="flex items-center gap-2">
            <Input value={dir} onChange={(e) => setDir(e.target.value)} placeholder="/Users/you/projects/my-app" className="font-mono text-[12.5px]" />
            <Button onClick={() => void browse()}>
              <FolderOpen size={14} /> {t("coding.choose")}
            </Button>
          </div>
          <p className="mt-1.5 text-[12px] leading-snug text-muted">{t("coding.gitAuto")}</p>
        </Field>
        <div className="flex justify-end gap-2 pt-1">
          <Button onClick={close}>{t("common.cancel")}</Button>
          <Button variant="primary" onClick={() => void submit()} disabled={!name.trim() || !dir.trim() || busy}>
            {busy && <Spinner size={13} />} {t("common.create")}
          </Button>
        </div>
      </div>
    </Modal>
  );
}

/** Agents that run on a coding CLI (Claude Code, Codex…), grouped in one floating window. */
export function CliAgentsDialog() {
  const { t } = useTranslation();
  const open = useStore((s) => s.dialog === "cli-agents");
  const close = () => useStore.getState().setDialog(null);
  const [tools, setTools] = useState<TermTool[] | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setTools(null);
    void api.termTools().then(setTools).catch(() => setTools([]));
  }, [open]);

  const create = async (c: (typeof CLI_AGENT_TOOLS)[number]) => {
    setBusy(c.tool);
    try {
      const agent = await createCliAgent(c.kind, c.label);
      await useStore.getState().refresh();
      useStore.getState().select(agent.id);
      close();
    } catch (e) {
      useStore.getState().showToast(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Modal open={open} onClose={close} className="w-[460px]" label={t("cli.title")}>
      <DialogHeader title={t("cli.title")} subtitle={t("cli.hint")} onClose={close} />
      <div className="space-y-1.5 px-6 pt-3 pb-5">
        {tools === null ? (
          <div className="flex justify-center py-8">
            <Spinner />
          </div>
        ) : (
          CLI_AGENT_TOOLS.map((c) => {
            const info = tools.find((x) => x.key === c.tool);
            const installed = !!info?.installed;
            return (
              <button
                key={c.tool}
                onClick={() => (installed ? void create(c) : useStore.getState().showToast(t("terminal.notInstalled", { name: c.label, how: info?.install || "—" })))}
                disabled={busy !== null}
                className={cx("flex w-full items-center gap-3 rounded-xl border border-line px-3.5 py-2.5 text-left transition-colors", installed ? "hover:border-accent hover:bg-hover" : "opacity-55")}
              >
                <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-white text-[#17171a] shadow-sm ring-1 ring-black/5 dark:bg-[#17171a] dark:text-white dark:ring-white/10">{busy === c.tool ? <Spinner size={15} /> : <TermIcon tool={c.tool} size={18} />}</span>
                <span className="min-w-0 flex-1">
                  <span className="block text-[14px] font-medium">{c.label}</span>
                  <span className="block truncate text-[12px] text-muted">{installed ? t("cli.ready") : t("cli.notInstalled", { how: info?.install || "—" })}</span>
                </span>
              </button>
            );
          })
        )}
      </div>
    </Modal>
  );
}
