import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronLeft, ChevronRight, Plus, Sparkles } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import { greetNewAgent } from "../../lib/greet";
import { isConfigured } from "../../lib/models";
import { ProviderLogo } from "../ProviderLogo";
import type { Avatar, ProviderConfig, ProviderInfo } from "../../lib/types";
import { cx } from "../../lib/format";
import { AvatarView } from "../AvatarView";
import { AvatarPickerDialog, randomAvatar } from "../panel/AvatarPicker";
import { ModelPicker } from "../ModelPicker";
import { ProviderDialog } from "../settings/ProvidersSection";
import { Badge, Button, Field, Input, Modal, Spinner, Textarea } from "../ui";
import { Logo } from "../Sidebar";

const STEPS = ["welcome", "you", "provider", "model", "agent", "done"] as const;
type Step = (typeof STEPS)[number];

/** First-run tour in a floating window: name, providers, models and a first agent. */
export function Walkthrough() {
  const { t } = useTranslation();
  const open = useStore((s) => s.dialog === "walkthrough");
  const providers = useStore((s) => s.providers);
  const settings = useStore((s) => s.settings);
  const setSetting = useStore((s) => s.setSetting);

  const [step, setStep] = useState<Step>("welcome");
  const [name, setName] = useState("");
  const [about, setAbout] = useState("");
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);
  const [setup, setSetup] = useState<{ provider: ProviderConfig | null; info: ProviderInfo } | null>(null);
  const [providerId, setProviderId] = useState<string | null>(null);
  const [model, setModel] = useState<string | null>(null);
  const [agentName, setAgentName] = useState("");
  const [avatar, setAvatar] = useState<Avatar>(randomAvatar);
  const [instructions, setInstructions] = useState("");
  const [picker, setPicker] = useState(false);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!open) return;
    setStep("welcome");
    setName(settings.user_name ?? "");
    setAbout(settings.about_me ?? "");
    setAgentName("");
    setInstructions("");
    setAvatar(randomAvatar());
    setModel(null);
    void api.providerCatalog().then(setCatalog);
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps

  const ready = providers.filter((p) => isConfigured(p, catalog.find((c) => c.kind === p.kind)));
  // Land on the provider the user just connected, or the app default.
  useEffect(() => {
    if (providerId && ready.some((p) => p.id === providerId)) return;
    const def = ready.find((p) => p.id === settings.default_provider) ?? ready[0];
    setProviderId(def?.id ?? null);
  }, [ready.map((p) => p.id).join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  // Start from the provider's own default model; the user can pick another.
  useEffect(() => {
    setModel(providers.find((p) => p.id === providerId)?.default_model ?? null);
  }, [providerId]); // eslint-disable-line react-hooks/exhaustive-deps

  const index = STEPS.indexOf(step);
  const close = () => {
    void setSetting("onboarded", "true");
    useStore.getState().setDialog(null);
  };
  const go = (d: number) => setStep(STEPS[Math.min(STEPS.length - 1, Math.max(0, index + d))]);

  const next = async () => {
    setBusy(true);
    try {
      if (step === "you") {
        await setSetting("user_name", name.trim());
        await setSetting("about_me", about.trim());
      }
      if (step === "model" && providerId) {
        await setSetting("default_provider", providerId);
        const p = providers.find((x) => x.id === providerId);
        if (p && model && p.default_model !== model) {
          await api.saveProvider({ ...p, default_model: model });
          await useStore.getState().refreshProviders();
        }
      }
      if (step === "agent" && agentName.trim()) {
        const agent = await api.createAgent({ name: agentName.trim(), avatar, instructions, provider_id: providerId, model });
        await greetNewAgent(agent, instructions.trim().length > 0);
        await useStore.getState().refresh();
        useStore.getState().select(agent.id);
      }
      go(1);
    } catch (e) {
      useStore.getState().showToast(String(e));
    } finally {
      setBusy(false);
    }
  };

  const nextLabel =
    step === "welcome"
      ? t("walkthrough.letsGo")
      : step === "provider" && ready.length === 0
        ? t("walkthrough.skipStep")
        : step === "agent" && !agentName.trim()
          ? t("walkthrough.skipStep")
          : step === "agent"
            ? t("walkthrough.createAgent")
            : t("walkthrough.next");

  return (
    <>
      <Modal open={open} onClose={close} className="w-[560px]" label={t("walkthrough.title")}>
        <div className="flex gap-1.5 px-6 pt-5">
          {STEPS.map((s, i) => (
            <span key={s} className={cx("h-1 flex-1 rounded-full transition-colors", i <= index ? "bg-accent" : "bg-line-strong/60")} />
          ))}
        </div>

        <div className="min-h-[340px] px-6 pt-5 pb-2">
          {step === "welcome" && (
            <div className="flex flex-col items-center pt-6 text-center">
              <Logo size={64} />
              <h2 className="mt-5 text-[22px] font-semibold tracking-tight">{t("walkthrough.welcomeTitle")}</h2>
              <p className="mt-2 max-w-sm text-[14px] leading-relaxed text-muted">{t("walkthrough.welcomeBody")}</p>
              <ul className="mt-5 space-y-1.5 text-left text-[13px] text-muted">
                {(["you", "provider", "model", "agent"] as const).map((s) => (
                  <li key={s} className="flex items-center gap-2">
                    <Check size={14} className="text-accent" strokeWidth={3} /> {t(`walkthrough.steps.${s}`)}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {step === "you" && (
            <div className="space-y-4">
              <Header title={t("walkthrough.youTitle")} body={t("walkthrough.youBody")} />
              <Field label={t("settings.account.name")}>
                <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder={t("walkthrough.namePlaceholder")} />
              </Field>
              <Field label={`${t("settings.account.aboutTitle")} (${t("common.optional")})`} hint={t("settings.account.aboutHint")}>
                <Textarea value={about} onChange={(e) => setAbout(e.target.value)} rows={5} placeholder={t("settings.account.aboutPlaceholder")} />
              </Field>
            </div>
          )}

          {step === "provider" && (
            <div className="space-y-3">
              <Header title={t("walkthrough.providerTitle")} body={t("walkthrough.providerBody")} />
              <div className="grid max-h-[250px] grid-cols-2 gap-2 overflow-y-auto pr-1">
                {catalog.map((info) => {
                  const mine = providers.find((p) => p.kind === info.kind);
                  const ok = mine ? isConfigured(mine, info) : false;
                  return (
                    <button
                      key={info.kind}
                      onClick={() => setSetup({ provider: mine ?? null, info })}
                      className={cx("flex items-center gap-2.5 rounded-xl border p-2.5 text-left transition-colors hover:bg-hover", ok ? "border-accent bg-accent-soft" : "border-line")}
                    >
                      <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-hover text-muted">
                        <ProviderLogo kind={info.kind} size={16} />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-[12.5px] font-medium">{info.label}</span>
                        <span className="block truncate text-[11px] text-muted">{info.note}</span>
                      </span>
                      {ok ? <Badge tone="success">✓</Badge> : <Plus size={14} className="shrink-0 text-faint" />}
                    </button>
                  );
                })}
              </div>
              <p className="text-[12px] text-muted">{ready.length > 0 ? t("walkthrough.providerReady", { n: ready.length }) : t("walkthrough.providerNone")}</p>
            </div>
          )}

          {step === "model" && (
            <div className="space-y-4">
              <Header title={t("walkthrough.modelTitle")} body={t("walkthrough.modelBody")} />
              {ready.length === 0 ? (
                <p className="rounded-xl border border-dashed border-line px-4 py-6 text-center text-[13px] text-muted">{t("walkthrough.modelNone")}</p>
              ) : (
                <ModelPicker providerId={providerId} model={model} onChange={(p, m) => (setProviderId(p), setModel(m))} defaultLabel={false} />
              )}
              <p className="text-[12px] text-muted">{t("walkthrough.modelHint")}</p>
            </div>
          )}

          {step === "agent" && (
            <div className="space-y-4">
              <Header title={t("walkthrough.agentTitle")} body={t("walkthrough.agentBody")} />
              <div className="flex items-center gap-4">
                <button onClick={() => setPicker(true)} className="rounded-full transition-transform hover:scale-105" aria-label={t("profile.pickAvatar")} data-tip={t("profile.pickAvatar")}>
                  <AvatarView avatar={avatar} size={60} />
                </button>
                <div className="flex-1">
                  <Field label={t("dialogs.name")}>
                    <Input value={agentName} onChange={(e) => setAgentName(e.target.value)} placeholder={t("dialogs.namePlaceholder")} />
                  </Field>
                </div>
              </div>
              <Field label={`${t("dialogs.instructions")} (${t("common.optional")})`}>
                <Textarea value={instructions} onChange={(e) => setInstructions(e.target.value)} rows={3} placeholder={t("dialogs.instructionsPlaceholder")} />
              </Field>
            </div>
          )}

          {step === "done" && (
            <div className="space-y-4">
              <div className="flex flex-col items-center pt-3 text-center">
                <span className="flex size-12 items-center justify-center rounded-full bg-accent-soft text-accent">
                  <Sparkles size={22} />
                </span>
                <h2 className="mt-3 text-[20px] font-semibold tracking-tight">{name.trim() ? t("walkthrough.doneTitle", { name: name.trim() }) : t("walkthrough.doneTitleAnon")}</h2>
                <p className="mt-1 text-[13.5px] text-muted">{t("walkthrough.doneBody")}</p>
              </div>
              <ul className="space-y-2">
                {(["slash", "mention", "settings", "memory"] as const).map((k) => (
                  <li key={k} className="rounded-xl border border-line px-3.5 py-2.5 text-[13px]">
                    <span className="font-medium">{t(`walkthrough.tips.${k}.title`)}</span>
                    <span className="block text-[12px] text-muted">{t(`walkthrough.tips.${k}.body`)}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>

        <div className="flex items-center gap-2 px-6 pt-3 pb-5">
          {step !== "done" && step !== "welcome" && (
            <Button variant="ghost" onClick={close}>
              {t("walkthrough.skipAll")}
            </Button>
          )}
          {step === "welcome" && (
            <Button variant="ghost" onClick={close}>
              {t("walkthrough.notNow")}
            </Button>
          )}
          <div className="ml-auto flex gap-2">
            {index > 0 && step !== "done" && (
              <Button onClick={() => go(-1)}>
                <ChevronLeft size={15} /> {t("common.back")}
              </Button>
            )}
            {step === "done" ? (
              <Button variant="primary" onClick={close}>
                {t("walkthrough.finish")}
              </Button>
            ) : (
              <Button variant="primary" onClick={() => void next()} disabled={busy}>
                {busy && <Spinner size={13} />} {nextLabel} {step !== "agent" && <ChevronRight size={15} />}
              </Button>
            )}
          </div>
        </div>
      </Modal>
      <AvatarPickerDialog open={open && picker} value={avatar} onChange={setAvatar} onClose={() => setPicker(false)} />
      {open && setup && (
        <ProviderDialog
          key={setup.provider?.id ?? setup.info.kind}
          {...setup}
          onSaved={(p) => setProviderId(p.id)}
          onClose={() => setSetup(null)}
        />
      )}
    </>
  );
}

function Header({ title, body }: { title: string; body: string }) {
  return (
    <div>
      <h2 className="text-[19px] font-semibold tracking-tight">{title}</h2>
      <p className="mt-1 text-[13.5px] leading-relaxed text-muted">{body}</p>
    </div>
  );
}
