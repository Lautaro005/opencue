import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import type { Avatar } from "../../lib/types";
import { cx } from "../../lib/format";
import { AvatarView } from "../AvatarView";
import { AvatarPickerDialog, randomAvatar } from "../panel/AvatarPicker";
import { ModelPicker } from "../ModelPicker";
import { isConfigured } from "../../lib/models";
import { Button, DialogHeader, Field, Input, Modal, Textarea } from "../ui";

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

  useEffect(() => {
    if (open) {
      setName("");
      setPicked([]);
    }
  }, [open]);

  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));

  const submit = async () => {
    if (!name.trim() || picked.length === 0) return;
    setBusy(true);
    try {
      const g = await api.createGroup(name.trim(), picked);
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
