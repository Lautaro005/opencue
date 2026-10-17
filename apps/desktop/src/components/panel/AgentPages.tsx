import { useCallback, useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Blocks, Clock, Copy, Mail, Pencil, Play, Plus, Trash2, Webhook, Workflow } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import type { Agent, AgentSettings, Automation, ComputerBackend, ComputerSettings, EmailAccount, SkillSummary, Trigger, Voice } from "../../lib/types";
import { cx, describeCron, formatTime } from "../../lib/format";
import { Button, DialogHeader, Field, IconButton, Input, Modal, Select, Textarea, Toggle } from "../ui";
import { BACKENDS } from "./ComputerPanel";
import { Row } from "./Section";

function Page({ children }: { children: ReactNode }) {
  return <div className="space-y-3 px-5 pt-1 pb-5">{children}</div>;
}

function Hint({ children }: { children: ReactNode }) {
  return <p className="text-[12px] leading-relaxed text-muted">{children}</p>;
}

// ---------- email ----------

const EMPTY_EMAIL = (agentId: string): EmailAccount => ({
  agent_id: agentId,
  kind: "imap",
  address: "",
  display_name: null,
  imap_host: null,
  imap_port: 993,
  smtp_host: null,
  smtp_port: 465,
  username: null,
  last_seen_uid: 0,
  has_password: false,
});

export function EmailPage({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const [acc, setAcc] = useState<EmailAccount | null>(null);
  const [exists, setExists] = useState(false);
  const [password, setPassword] = useState("");
  const [advanced, setAdvanced] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api.getEmailAccount(agent.id).then((a) => {
      setExists(!!a);
      setAcc(a ?? { ...EMPTY_EMAIL(agent.id), display_name: agent.name });
    });
  }, [agent.id, agent.name]);

  if (!acc) return null;
  const set = (p: Partial<EmailAccount>) => setAcc({ ...acc, ...p });

  const save = async () => {
    setBusy(true);
    setStatus(null);
    try {
      const saved = await api.saveEmailAccount(acc, password || undefined);
      setAcc(saved);
      setExists(true);
      setPassword("");
      await api.testEmail(agent.id);
      setStatus({ ok: true, text: t("email.connected") });
    } catch (e) {
      setStatus({ ok: false, text: String(e) });
    } finally {
      setBusy(false);
    }
  };

  return (
    <Page>
      <Hint>{t("email.hint", { name: agent.name })}</Hint>
      <Field label={t("email.provider")}>
        <Select
          value={acc.kind}
          onChange={(v) => set({ kind: v as EmailAccount["kind"] })}
          options={[
            { value: "imap", label: t("email.imap") },
            { value: "composio_gmail", label: t("email.composioGmail") },
          ]}
        />
      </Field>
      <Field label={t("email.address")}>
        <Input value={acc.address} onChange={(e) => set({ address: e.target.value })} placeholder="agent@example.com" />
      </Field>
      {acc.kind === "imap" ? (
        <>
          <Field label={t("email.password")} hint={acc.has_password ? t("email.passwordSaved") : t("email.passwordHint")}>
            <Input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder={acc.has_password ? "••••••••" : ""} />
          </Field>
          <button onClick={() => setAdvanced((v) => !v)} className="text-[12px] text-muted hover:text-accent">
            {advanced ? t("email.hideServers") : t("email.showServers")}
          </button>
          {advanced && (
            <div className="space-y-2 rounded-xl border border-line p-3">
              <Field label={t("email.displayName")}>
                <Input value={acc.display_name ?? ""} onChange={(e) => set({ display_name: e.target.value || null })} />
              </Field>
              <div className="grid grid-cols-[1fr_76px] gap-2">
                <Field label="IMAP">
                  <Input value={acc.imap_host ?? ""} onChange={(e) => set({ imap_host: e.target.value || null })} placeholder={t("email.auto")} />
                </Field>
                <Field label={t("email.port")}>
                  <Input value={acc.imap_port} onChange={(e) => set({ imap_port: Number(e.target.value) || 993 })} />
                </Field>
                <Field label="SMTP">
                  <Input value={acc.smtp_host ?? ""} onChange={(e) => set({ smtp_host: e.target.value || null })} placeholder={t("email.auto")} />
                </Field>
                <Field label={t("email.port")}>
                  <Input value={acc.smtp_port} onChange={(e) => set({ smtp_port: Number(e.target.value) || 465 })} />
                </Field>
              </div>
              <Field label={t("email.username")}>
                <Input value={acc.username ?? ""} onChange={(e) => set({ username: e.target.value || null })} placeholder={acc.address} />
              </Field>
            </div>
          )}
        </>
      ) : (
        <Hint>{t("email.composioHint")}</Hint>
      )}
      {status && <p className={cx("text-[12px]", status.ok ? "text-success" : "text-danger")}>{status.text}</p>}
      <div className="flex gap-2">
        <Button variant="primary" onClick={() => void save()} disabled={busy || !acc.address.includes("@")}>
          {exists ? t("email.saveTest") : t("email.connect")}
        </Button>
        {exists && (
          <Button
            variant="danger"
            onClick={async () => {
              await api.deleteEmailAccount(agent.id);
              setExists(false);
              setAcc(EMPTY_EMAIL(agent.id));
              setStatus(null);
            }}
          >
            {t("common.remove")}
          </Button>
        )}
      </div>
      <Hint>{t("email.approvalNote")}</Hint>
    </Page>
  );
}

// ---------- computer ----------

export function ComputerPage({ agent, onChange }: { agent: Agent; onChange: (s: Partial<AgentSettings>) => void }) {
  const { t } = useTranslation();
  const openSettings = useStore((s) => s.openSettings);
  const setPanel = useStore((s) => s.setPanel);
  const [env, setEnv] = useState<ComputerSettings | null>(null);
  useEffect(() => void api.computerSettings().then(setEnv), []);

  const available = (b: ComputerBackend) => {
    if (!env) return true;
    if (b === "wsl") return env.wsl_available;
    if (b === "docker") return env.docker_available;
    return true;
  };

  return (
    <Page>
      <Hint>{t("computer.pageHint")}</Hint>
      <div className="space-y-1.5">
        {BACKENDS.filter((b) => b !== "wsl" || env?.os === "windows").map((b) => {
          const on = agent.settings.computer === b;
          return (
            <button
              key={b}
              disabled={!available(b)}
              onClick={() => onChange({ computer: b })}
              className={cx("flex w-full items-start gap-2.5 rounded-xl border px-3 py-2 text-left transition-colors disabled:opacity-45", on ? "border-accent bg-accent-soft" : "border-line hover:bg-hover")}
            >
              <span className={cx("mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border-2", on ? "border-accent" : "border-line-strong")}>
                {on && <span className="size-1.5 rounded-full bg-accent" />}
              </span>
              <span>
                <span className="block text-[13px] font-medium">{t(`computer.backend.${b}`)}</span>
                <span className="block text-[11.5px] text-muted">{available(b) ? t(`computer.backendHint.${b}`) : t("computer.unavailable")}</span>
              </span>
            </button>
          );
        })}
      </div>
      {agent.settings.computer === "space" && !env?.space_url && (
        <button onClick={() => openSettings("computers")} className="text-[12px] text-warning hover:underline">
          {t("computer.spaceMissing")}
        </button>
      )}
      {agent.settings.computer !== "none" && (
        <Button onClick={() => setPanel("computer")} className="w-full">
          {t("computer.open")}
        </Button>
      )}
    </Page>
  );
}

// ---------- skills ----------

export function SkillsPage({ agent, onChange }: { agent: Agent; onChange: (s: Partial<AgentSettings>) => void }) {
  const { t } = useTranslation();
  const openSettings = useStore((s) => s.openSettings);
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  useEffect(() => void api.listSkills().then(setSkills), []);
  return (
    <Page>
      <Row label={t("skills.enable")} hint={t("skills.enableHint")}>
        <Toggle checked={agent.settings.skills} onChange={(v) => onChange({ skills: v })} label={t("skills.enable")} />
      </Row>
      {skills.length === 0 ? (
        <Hint>{t("skills.none")}</Hint>
      ) : (
        <ul className="space-y-1.5">
          {skills.map((k) => (
            <li key={k.name} className="rounded-xl border border-line px-3 py-2">
              <div className="text-[13px] font-medium">{k.name}</div>
              <div className="line-clamp-2 text-[11.5px] text-muted">{k.description}</div>
            </li>
          ))}
        </ul>
      )}
      <Button onClick={() => openSettings("skills")} className="w-full">
        {t("skills.manage")}
      </Button>
    </Page>
  );
}

// ---------- voice ----------

export function VoicePage({ agent, onChange }: { agent: Agent; onChange: (s: Partial<AgentSettings>) => void }) {
  const { t } = useTranslation();
  const openSettings = useStore((s) => s.openSettings);
  const [voices, setVoices] = useState<Voice[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.listVoices().then(setVoices).catch((e) => {
      setVoices([]);
      setError(String(e));
    });
  }, []);
  return (
    <Page>
      <Hint>{t("voice.agentHint", { name: agent.name })}</Hint>
      {error ? (
        <button onClick={() => openSettings("voice")} className="text-left text-[12px] text-warning hover:underline">
          {t("voice.needKey")}
        </button>
      ) : (
        <Field label={t("voice.voice")}>
          <Select
            value={agent.settings.voice_id ?? ""}
            onChange={(v) => onChange({ voice_id: v || null })}
            options={[{ value: "", label: t("voice.appDefault") }, ...(voices ?? []).map((v) => ({ value: v.id, label: v.name }))]}
            disabled={voices === null}
          />
        </Field>
      )}
    </Page>
  );
}

// ---------- automations ----------

function describeTrigger(tr: Trigger, t: (k: string, o?: Record<string, unknown>) => string, groups: Record<string, { name: string }>) {
  switch (tr.kind) {
    case "schedule":
      return describeCron(tr.cron);
    case "email":
      return tr.filter ? t("automations.summary.emailFilter", { filter: tr.filter }) : t("automations.summary.email");
    case "webhook":
      return t("automations.summary.webhook");
    case "group_message":
      return t("automations.summary.group", { group: groups[tr.group_id]?.name ?? "?" });
  }
}

const TRIGGER_ICON: Record<Trigger["kind"], ReactNode> = {
  schedule: <Clock size={15} />,
  email: <Mail size={15} />,
  webhook: <Webhook size={15} />,
  group_message: <Blocks size={15} />,
};

export function AutomationsSection({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const groups = useStore((s) => s.groups);
  const [list, setList] = useState<Automation[]>([]);
  const [editing, setEditing] = useState<Automation | "new" | null>(null);

  const load = useCallback(() => void api.listAutomations(agent.id).then(setList), [agent.id]);
  useEffect(() => {
    load();
    window.addEventListener("opencue:automations-changed", load);
    return () => window.removeEventListener("opencue:automations-changed", load);
  }, [load]);

  return (
    <div className="px-5 pb-3">
      <div className="mb-1 flex items-center justify-between">
        <span className="text-[12.5px] font-medium text-muted">{t("profile.automations")}</span>
        <IconButton label={t("automations.new")} onClick={() => setEditing("new")} className="size-7">
          <Plus size={15} />
        </IconButton>
      </div>
      {list.length === 0 ? (
        <button onClick={() => setEditing("new")} className="flex w-full flex-col items-center gap-2 rounded-xl py-6 text-center hover:bg-hover">
          <Workflow size={26} className="text-faint" strokeWidth={1.6} />
          <span className="text-[13px] text-muted">{t("profile.automationsEmpty")}</span>
        </button>
      ) : (
        <ul className="space-y-1.5">
          {list.map((a) => (
            <li key={a.id} className="group rounded-xl border border-line px-3 py-2.5">
              <div className="flex items-center gap-2">
                <span className="text-muted">{TRIGGER_ICON[a.trigger.kind]}</span>
                <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{a.name}</span>
                <Toggle checked={a.enabled} onChange={(v) => void api.saveAutomation({ ...a, enabled: v })} label={a.name} />
              </div>
              <div className="mt-1 pl-6 text-[11.5px] text-muted">{describeTrigger(a.trigger, t, groups)}</div>
              <div className="flex items-center gap-1 pt-1 pl-6">
                <span className="flex-1 text-[11px] text-faint">
                  {a.next_run_at
                    ? t("automations.next", { when: formatTime(a.next_run_at) })
                    : a.last_run_at
                      ? t("automations.last", { when: formatTime(a.last_run_at) })
                      : ""}
                </span>
                <IconButton label={t("automations.runNow")} onClick={() => void api.runAutomation(a.id)} className="size-6 opacity-0 group-hover:opacity-100">
                  <Play size={12} />
                </IconButton>
                <IconButton label={t("common.edit")} onClick={() => setEditing(a)} className="size-6 opacity-0 group-hover:opacity-100">
                  <Pencil size={12} />
                </IconButton>
              </div>
            </li>
          ))}
        </ul>
      )}
      {editing && <AutomationEditor agent={agent} initial={editing === "new" ? null : editing} onClose={() => setEditing(null)} />}
    </div>
  );
}

const PRESETS: { label: string; cron: string }[] = [
  { label: "automations.presets.morning", cron: "0 9 * * *" },
  { label: "automations.presets.weekdays", cron: "0 9 * * 1-5" },
  { label: "automations.presets.monday", cron: "0 9 * * 1" },
  { label: "automations.presets.hourly", cron: "0 * * * *" },
];

function AutomationEditor({ agent, initial, onClose }: { agent: Agent; initial: Automation | null; onClose: () => void }) {
  const { t } = useTranslation();
  const groups = useStore((s) => s.groups);
  const [name, setName] = useState(initial?.name ?? "");
  const [prompt, setPrompt] = useState(initial?.prompt ?? "");
  const [trigger, setTrigger] = useState<Trigger>(initial?.trigger ?? { kind: "schedule", cron: "0 9 * * *" });
  const [port, setPort] = useState(7777);
  const [error, setError] = useState<string | null>(null);
  const groupList = Object.values(groups).filter((g) => g.members.some((m) => m.agent_id === agent.id));

  useEffect(() => void api.computerSettings().then((s) => setPort(s.webhook_port)), []);

  const setKind = async (kind: Trigger["kind"]) => {
    if (kind === trigger.kind) return;
    if (kind === "schedule") setTrigger({ kind, cron: "0 9 * * *" });
    if (kind === "email") setTrigger({ kind, filter: "", poll_minutes: 5 });
    if (kind === "webhook") setTrigger({ kind, token: await api.newWebhookToken() });
    if (kind === "group_message") setTrigger({ kind, group_id: groupList[0]?.id ?? "", keyword: "" });
  };

  const save = async () => {
    setError(null);
    try {
      await api.saveAutomation({ id: initial?.id ?? null, agent_id: agent.id, name, prompt, trigger, enabled: initial?.enabled ?? true });
      onClose();
    } catch (e) {
      setError(String(e));
    }
  };

  const hookUrl = trigger.kind === "webhook" ? `http://127.0.0.1:${port}/hooks/${trigger.token}` : "";

  return (
    <Modal open onClose={onClose} className="w-[520px]" label={t("automations.new")}>
      <DialogHeader title={initial ? t("automations.edit") : t("automations.new")} subtitle={agent.name} onClose={onClose} />
      <div className="space-y-3 px-6 pb-5">
        <Field label={t("automations.name")}>
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder={t("automations.namePlaceholder")} autoFocus />
        </Field>
        <Field label={t("automations.when")}>
          <div className="grid grid-cols-4 gap-1.5">
            {(["schedule", "email", "webhook", "group_message"] as const).map((k) => (
              <button
                key={k}
                onClick={() => void setKind(k)}
                className={cx("flex flex-col items-center gap-1 rounded-xl border px-2 py-2 text-[11.5px]", trigger.kind === k ? "border-accent bg-accent-soft text-ink" : "border-line text-muted hover:bg-hover")}
              >
                {TRIGGER_ICON[k]}
                {t(`automations.kinds.${k}`)}
              </button>
            ))}
          </div>
        </Field>
        {trigger.kind === "schedule" && (
          <Field label={t("automations.cron")} hint={t("automations.cronHint")}>
            <Input value={trigger.cron} onChange={(e) => setTrigger({ ...trigger, cron: e.target.value })} className="font-mono" />
            {describeCron(trigger.cron) !== trigger.cron && <p className="mt-1 text-[12px] text-accent">{describeCron(trigger.cron)}</p>}
            <div className="mt-1.5 flex flex-wrap gap-1">
              {PRESETS.map((p) => (
                <button key={p.cron} onClick={() => setTrigger({ ...trigger, cron: p.cron })} className="rounded-full border border-line px-2 py-0.5 text-[11.5px] text-muted hover:bg-hover">
                  {t(p.label)}
                </button>
              ))}
            </div>
          </Field>
        )}
        {trigger.kind === "email" && (
          <>
            <Field label={t("automations.filter")} hint={t("automations.filterHint")}>
              <Input value={trigger.filter} onChange={(e) => setTrigger({ ...trigger, filter: e.target.value })} placeholder={t("automations.filterPlaceholder")} />
            </Field>
            <Field label={t("automations.poll")}>
              <Select
                value={String(trigger.poll_minutes)}
                onChange={(v) => setTrigger({ ...trigger, poll_minutes: Number(v) })}
                options={[1, 5, 15, 60].map((m) => ({ value: String(m), label: t("automations.everyMinutes", { n: m }) }))}
              />
            </Field>
          </>
        )}
        {trigger.kind === "webhook" && (
          <Field label={t("automations.hookUrl")} hint={t("automations.hookHint")}>
            <div className="flex gap-1.5">
              <Input value={hookUrl} readOnly className="font-mono text-[11.5px]" />
              <IconButton label={t("common.copy")} onClick={() => void navigator.clipboard.writeText(hookUrl)} className="border border-line">
                <Copy size={14} />
              </IconButton>
            </div>
          </Field>
        )}
        {trigger.kind === "group_message" && (
          <>
            <Field label={t("automations.group")}>
              {groupList.length === 0 ? (
                <p className="text-[12px] text-muted">{t("automations.noGroups", { name: agent.name })}</p>
              ) : (
                <Select value={trigger.group_id} onChange={(v) => setTrigger({ ...trigger, group_id: v })} options={groupList.map((g) => ({ value: g.id, label: g.name }))} />
              )}
            </Field>
            <Field label={t("automations.keyword")}>
              <Input value={trigger.keyword} onChange={(e) => setTrigger({ ...trigger, keyword: e.target.value })} placeholder={t("automations.keywordPlaceholder")} />
            </Field>
          </>
        )}
        <Field label={t("automations.prompt")}>
          <Textarea value={prompt} onChange={(e) => setPrompt(e.target.value)} rows={4} placeholder={t("automations.promptPlaceholder")} />
        </Field>
        {error && <p className="text-[12px] text-danger">{error}</p>}
        <div className="flex items-center gap-2 pt-1">
          {initial && (
            <Button
              variant="danger"
              onClick={async () => {
                await api.deleteAutomation(initial.id);
                onClose();
              }}
            >
              <Trash2 size={14} /> {t("common.delete")}
            </Button>
          )}
          <div className="ml-auto flex gap-2">
            <Button onClick={onClose}>{t("common.cancel")}</Button>
            <Button variant="primary" onClick={() => void save()} disabled={!name.trim() || !prompt.trim()}>
              {t("common.save")}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}
