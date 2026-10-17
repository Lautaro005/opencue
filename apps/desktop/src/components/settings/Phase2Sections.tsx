import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Box, Download, ExternalLink, Plus, Server, Terminal, Trash2 } from "lucide-react";
import { api, openUrl } from "../../lib/api";
import { useStore } from "../../lib/store";
import type { ComposioStatus, ComputerSettings, Skill, SkillSummary, Voice, VoiceSettings } from "../../lib/types";
import { Badge, Button, DialogHeader, Field, Input, Modal, Select, Textarea, Toggle } from "../ui";
import { Card, CardRow, PageTitle } from "./SettingsModal";

// ---------- Composio (inside Apps) ----------

export function ComposioCard() {
  const { t } = useTranslation();
  const [status, setStatus] = useState<ComposioStatus | null>(null);
  const [key, setKey] = useState("");
  const [user, setUser] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void api.composioStatus().then((s) => {
      setStatus(s);
      setUser(s.user_id);
    });
  }, []);

  const save = async () => {
    setBusy(true);
    try {
      setStatus(await api.saveComposio(key || undefined, user));
      setKey("");
    } finally {
      setBusy(false);
    }
  };

  return (
    <Card
      title="Composio"
      action={
        <button onClick={() => void openUrl("https://platform.composio.dev")} className="flex items-center gap-1 text-[12px] text-muted hover:text-accent">
          {t("settings.composio.dashboard")} <ExternalLink size={12} />
        </button>
      }
    >
      <div className="space-y-3 px-4 py-3.5">
        <p className="text-[12.5px] text-muted">{t("settings.composio.hint")}</p>
        <div className="grid grid-cols-2 gap-2">
          <Field label={t("settings.composio.key")} hint={status?.configured ? t("settings.composio.keySaved") : undefined}>
            <Input type="password" value={key} onChange={(e) => setKey(e.target.value)} placeholder={status?.configured ? "••••••••" : "ak_…"} />
          </Field>
          <Field label={t("settings.composio.user")} hint={t("settings.composio.userHint")}>
            <Input value={user} onChange={(e) => setUser(e.target.value)} placeholder="default" />
          </Field>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" variant="primary" onClick={() => void save()} disabled={busy}>
            {t("common.save")}
          </Button>
          {status?.error && <span className="line-clamp-2 text-[12px] text-danger">{status.error}</span>}
        </div>
        {status?.configured && (
          <div className="flex flex-wrap gap-1.5">
            {status.accounts.length === 0 ? (
              <span className="text-[12px] text-faint">{t("settings.composio.noAccounts")}</span>
            ) : (
              status.accounts.map((a) => (
                <Badge key={a.id} tone={a.status === "ACTIVE" ? "success" : "neutral"}>
                  {a.toolkit}
                </Badge>
              ))
            )}
          </div>
        )}
      </div>
    </Card>
  );
}

// ---------- Skills ----------

export function SkillsSection() {
  const { t } = useTranslation();
  const showToast = useStore((s) => s.showToast);
  const [skills, setSkills] = useState<SkillSummary[]>([]);
  const [url, setUrl] = useState("");
  const [editing, setEditing] = useState<Skill | null>(null);
  const load = () => void api.listSkills().then(setSkills);
  useEffect(load, []);

  const importUrl = async () => {
    try {
      const k = await api.importSkill(url);
      setUrl("");
      showToast(t("settings.skills.imported", { name: k.name }));
      load();
    } catch (e) {
      showToast(String(e));
    }
  };

  return (
    <>
      <PageTitle title={t("settings.skills.title")} hint={t("settings.skills.hint")} />
      <Card title={t("settings.skills.import")}>
        <div className="flex gap-2 p-3">
          <Input value={url} onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void importUrl()} placeholder="https://github.com/…/SKILL.md" />
          <Button onClick={() => void importUrl()} disabled={!url.trim()}>
            <Download size={14} /> {t("settings.skills.importBtn")}
          </Button>
        </div>
      </Card>
      <Card
        title={t("settings.skills.installed")}
        action={
          <Button size="sm" onClick={() => setEditing({ name: "", description: "", body: "", path: "" })}>
            <Plus size={13} /> {t("settings.skills.new")}
          </Button>
        }
      >
        {skills.length === 0 ? (
          <p className="px-4 py-5 text-center text-[12.5px] text-faint">{t("settings.skills.empty")}</p>
        ) : (
          skills.map((k) => (
            <button key={k.name} onClick={() => void api.getSkill(k.name).then((s) => s && setEditing(s))} className="flex w-full items-start gap-3 px-4 py-3 text-left hover:bg-hover">
              <div className="min-w-0 flex-1">
                <div className="text-[13.5px] font-medium">{k.name}</div>
                <div className="line-clamp-2 text-[12px] text-muted">{k.description}</div>
              </div>
            </button>
          ))
        )}
      </Card>
      {editing && <SkillEditor skill={editing} onClose={() => (setEditing(null), load())} />}
    </>
  );
}

function SkillEditor({ skill, onClose }: { skill: Skill; onClose: () => void }) {
  const { t } = useTranslation();
  const [name, setName] = useState(skill.name);
  const [description, setDescription] = useState(skill.description);
  const [body, setBody] = useState(skill.body);
  const [error, setError] = useState<string | null>(null);
  const isNew = !skill.path;
  return (
    <Modal open onClose={onClose} className="w-[640px]" label={t("settings.skills.title")}>
      <DialogHeader title={isNew ? t("settings.skills.new") : skill.name} subtitle={skill.path || "SKILL.md"} onClose={onClose} />
      <div className="space-y-3 px-6 pb-5">
        <Field label={t("settings.skills.name")}>
          <Input value={name} onChange={(e) => setName(e.target.value)} disabled={!isNew} placeholder="weekly-report" />
        </Field>
        <Field label={t("settings.skills.description")} hint={t("settings.skills.descriptionHint")}>
          <Input value={description} onChange={(e) => setDescription(e.target.value)} />
        </Field>
        <Field label={t("settings.skills.body")}>
          <Textarea value={body} onChange={(e) => setBody(e.target.value)} rows={12} className="font-mono text-[12.5px]" />
        </Field>
        {error && <p className="text-[12px] text-danger">{error}</p>}
        <div className="flex gap-2">
          {!isNew && (
            <Button
              variant="danger"
              onClick={async () => {
                await api.deleteSkill(skill.name);
                onClose();
              }}
            >
              <Trash2 size={14} /> {t("common.delete")}
            </Button>
          )}
          <div className="ml-auto flex gap-2">
            <Button onClick={onClose}>{t("common.cancel")}</Button>
            <Button
              variant="primary"
              disabled={!name.trim() || !description.trim()}
              onClick={async () => {
                try {
                  await api.saveSkill(name, description, body);
                  onClose();
                } catch (e) {
                  setError(String(e));
                }
              }}
            >
              {t("common.save")}
            </Button>
          </div>
        </div>
      </div>
    </Modal>
  );
}

// ---------- Computers ----------

export function ComputersSection() {
  const { t } = useTranslation();
  const [s, setS] = useState<ComputerSettings | null>(null);
  const [token, setToken] = useState("");
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  useEffect(() => void api.computerSettings().then(setS), []);
  if (!s) return null;

  const save = async (next: ComputerSettings, tok?: string) => {
    setS(await api.saveComputerSettings(next, tok));
  };

  return (
    <>
      <PageTitle title={t("settings.computers.title")} hint={t("settings.computers.hint")} />
      <Card title={t("settings.computers.docker")}>
        <CardRow label={t("settings.computers.status")} hint={t("settings.computers.dockerHint")}>
          <Badge tone={s.docker_available ? "success" : "neutral"}>{s.docker_available ? t("settings.computers.found") : t("settings.computers.notFound")}</Badge>
        </CardRow>
        <CardRow label={t("settings.computers.image")}>
          <Input className="w-56" value={s.docker_image} placeholder="ubuntu:24.04" onChange={(e) => setS({ ...s, docker_image: e.target.value })} onBlur={() => void save(s)} />
        </CardRow>
      </Card>
      <Card
        title="SPACE"
        action={
          <button onClick={() => void openUrl("https://space.docs.perplexity.ai/")} className="flex items-center gap-1 text-[12px] text-muted hover:text-accent">
            {t("settings.computers.spaceDocs")} <ExternalLink size={12} />
          </button>
        }
      >
        <div className="flex items-start gap-3 px-4 pt-3.5 text-[12.5px] text-muted">
          <Server size={16} className="mt-0.5 shrink-0" />
          <span>{s.os === "linux" ? t("settings.computers.spaceHint") : t("settings.computers.spaceRemote")}</span>
        </div>
        <div className="grid grid-cols-2 gap-2 px-4 py-3">
          <Field label={t("settings.computers.spaceUrl")}>
            <Input value={s.space_url} placeholder="http://localhost:8080/v1" onChange={(e) => setS({ ...s, space_url: e.target.value })} onBlur={() => void save(s)} />
          </Field>
          <Field label={t("settings.computers.template")}>
            <Input value={s.space_template} placeholder="seed:latest" onChange={(e) => setS({ ...s, space_template: e.target.value })} onBlur={() => void save(s)} />
          </Field>
          <Field label={t("settings.computers.token")} hint={s.has_space_token ? t("settings.computers.tokenSaved") : undefined}>
            <Input type="password" value={token} placeholder={s.has_space_token ? "••••••••" : ""} onChange={(e) => setToken(e.target.value)} onBlur={() => token && void save(s, token).then(() => setToken(""))} />
          </Field>
          <div className="flex items-end gap-2">
            <Button
              onClick={async () => {
                setTest(null);
                try {
                  const n = await api.testSpace();
                  setTest({ ok: true, text: t("settings.computers.spaceOk", { n }) });
                } catch (e) {
                  setTest({ ok: false, text: String(e) });
                }
              }}
              disabled={!s.space_url}
            >
              {t("settings.computers.test")}
            </Button>
          </div>
        </div>
        {test && <p className={`px-4 pb-3 text-[12px] ${test.ok ? "text-success" : "text-danger"}`}>{test.text}</p>}
      </Card>
      <Card title={t("settings.computers.host")}>
        <div className="flex items-start gap-3 px-4 py-3.5 text-[12.5px] text-muted">
          <Terminal size={16} className="mt-0.5 shrink-0" />
          <span>{t("settings.computers.hostHint")}</span>
        </div>
      </Card>
      <Card title={t("settings.computers.webhooks")}>
        <CardRow label={t("settings.computers.port")} hint={t("settings.computers.portHint")}>
          <Input className="w-24" value={s.webhook_port} onChange={(e) => setS({ ...s, webhook_port: Number(e.target.value) || 7777 })} onBlur={() => void save(s)} />
        </CardRow>
      </Card>
      <p className="flex items-center gap-1.5 text-[12px] text-faint">
        <Box size={13} /> {t("settings.computers.perAgent")}
      </p>
    </>
  );
}

// ---------- Voice ----------

export function VoiceSection() {
  const { t } = useTranslation();
  const [s, setS] = useState<VoiceSettings | null>(null);
  const [key, setKey] = useState("");
  const [voices, setVoices] = useState<Voice[]>([]);
  const [error, setError] = useState<string | null>(null);

  const loadVoices = () =>
    api
      .listVoices()
      .then((v) => (setVoices(v), setError(null)))
      .catch((e) => setError(String(e)));

  useEffect(() => {
    void api.voiceSettings().then((v) => {
      setS(v);
      if (v.configured) void loadVoices();
    });
  }, []);
  if (!s) return null;

  const save = async (next: VoiceSettings, apiKey?: string) => {
    const saved = await api.saveVoiceSettings(next, apiKey);
    setS(saved);
    useStore.setState((st) => ({ settings: { ...st.settings, voice_auto_speak: String(saved.auto_speak) } }));
    if (apiKey) void loadVoices();
  };

  return (
    <>
      <PageTitle title={t("settings.voice.title")} hint={t("settings.voice.hint")} />
      <Card
        title="ElevenLabs"
        action={
          <button onClick={() => void openUrl("https://elevenlabs.io/app/settings/api-keys")} className="flex items-center gap-1 text-[12px] text-muted hover:text-accent">
            {t("settings.voice.getKey")} <ExternalLink size={12} />
          </button>
        }
      >
        <CardRow label={t("settings.voice.key")} hint={s.configured ? t("settings.voice.keySaved") : t("settings.voice.keyHint")}>
          <div className="flex gap-1.5">
            <Input type="password" className="w-56" value={key} placeholder={s.configured ? "••••••••" : "sk_…"} onChange={(e) => setKey(e.target.value)} />
            <Button disabled={!key} onClick={() => void save(s, key).then(() => setKey(""))}>
              {t("common.save")}
            </Button>
          </div>
        </CardRow>
        <CardRow label={t("settings.voice.defaultVoice")}>
          <Select
            className="w-56"
            value={s.voice_id}
            onChange={(v) => void save({ ...s, voice_id: v })}
            options={[{ value: "", label: "Rachel (default)" }, ...voices.map((v) => ({ value: v.id, label: v.name }))]}
            disabled={!s.configured}
          />
        </CardRow>
        <CardRow label={t("settings.voice.autoSpeak")} hint={t("settings.voice.autoSpeakHint")}>
          <Toggle checked={s.auto_speak} onChange={(v) => void save({ ...s, auto_speak: v })} label={t("settings.voice.autoSpeak")} />
        </CardRow>
      </Card>
      {error && <p className="text-[12px] text-danger">{error}</p>}
      <p className="text-[12px] text-faint">{t("settings.voice.privacy")}</p>
    </>
  );
}
