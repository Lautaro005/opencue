import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Box, Check, Copy, Download, ExternalLink, Github, MonitorCog, Play, Plus, RefreshCw, Server, Square, Terminal, Trash2 } from "lucide-react";
import { api, openUrl } from "../../lib/api";
import { useStore } from "../../lib/store";
import type { ComputerSettings, DockerSetup, Skill, SkillSummary, Voice, VoiceSettings, VmPalInfo } from "../../lib/types";
import { Badge, Button, DialogHeader, Field, Input, Modal, Select, Spinner, Textarea, Toggle } from "../ui";
import { Card, CardRow, PageTitle } from "./SettingsModal";
import { SpaceSetup } from "./SpaceSetup";

export const SPACE_REPO = "https://github.com/perplexityai/SPACE";

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
  const showToast = useStore((st) => st.showToast);
  const [s, setS] = useState<ComputerSettings | null>(null);
  const [token, setToken] = useState("");
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  const [tab, setTab] = useState<"sandbox" | "space" | "vmpal" | "more">("sandbox");
  useEffect(() => void api.computerSettings().then(setS), []);
  if (!s) return null;

  const save = async (next: ComputerSettings, tok?: string) => {
    setS(await api.saveComputerSettings(next, tok));
  };

  return (
    <>
      <PageTitle title={t("settings.computers.title")} hint={t("settings.computers.hint")} />
      <div className="mb-4 flex flex-wrap gap-1 rounded-xl border border-line p-1" role="tablist" aria-label={t("settings.computers.title")}>
        {(["sandbox", "space", "vmpal", "more"] as const).map((k) => (
          <button
            key={k}
            role="tab"
            aria-selected={tab === k}
            onClick={() => setTab(k)}
            className={`flex-1 rounded-lg px-3 py-1.5 text-[13px] font-medium whitespace-nowrap ${tab === k ? "bg-accent text-white" : "text-muted hover:bg-hover"}`}
          >
            {t(`settings.computers.tab.${k}`)}
          </button>
        ))}
      </div>
      {tab === "sandbox" && (
        <>
      <Card title={t("settings.computers.docker")}>
        <CardRow label={t("settings.computers.runtime")} hint={t("settings.computers.runtimeHint")}>
          <span className="inline-flex rounded-lg border border-line p-0.5" role="group" aria-label={t("settings.computers.runtime")}>
            {(["docker", "podman"] as const).map((r) => (
              <button
                key={r}
                aria-pressed={s.runtime === r}
                onClick={() => void save({ ...s, runtime: r })}
                className={`rounded-md px-3 py-1 text-[12.5px] font-medium ${s.runtime === r ? "bg-accent text-white" : "text-muted hover:bg-hover"}`}
              >
                {r === "docker" ? "Docker" : "Podman"}
              </button>
            ))}
          </span>
        </CardRow>
        <CardRow label={t("settings.computers.status")} hint={s.runtime === "podman" ? t("settings.computers.podmanHint") : t("settings.computers.dockerHint")}>
          {(() => {
            const found = s.runtime === "podman" ? s.podman_available : s.docker_available;
            return <Badge tone={found ? "success" : "neutral"}>{found ? t("settings.computers.found") : t("settings.computers.notFound")}</Badge>;
          })()}
        </CardRow>
        {s.runtime !== "podman" && (
          <CardRow label={t("settings.computers.image")}>
            <Input className="w-56" value={s.docker_image} placeholder="ubuntu:24.04" onChange={(e) => setS({ ...s, docker_image: e.target.value })} onBlur={() => void save(s)} />
          </CardRow>
        )}
        <CardRow label={t("settings.computers.stopVms")} hint={t("settings.computers.stopVmsHint")}>
          <Button
            size="sm"
            onClick={() =>
              void api.stopAllComputers().then((n) => showToast(n > 0 ? t("settings.computers.stopped", { count: n }) : t("settings.computers.noneRunning")))
            }
          >
            <Square size={12} fill="currentColor" /> {t("settings.computers.stopAll")}
          </Button>
        </CardRow>
        <CardRow label={t("settings.computers.desktop")} hint={t("settings.computers.desktopHint")}>
          <Toggle checked={s.docker_desktop} onChange={(v) => void save({ ...s, docker_desktop: v })} />
        </CardRow>
        {s.runtime === "podman" && <p className="px-4 pb-3.5 text-[11.5px] text-muted">{t("settings.computers.podmanDesktop")}</p>}
        {s.docker_desktop && s.runtime !== "podman" && (
          <div className="grid grid-cols-[1fr_96px] gap-2 px-4 pb-3.5">
            <Field label={t("settings.computers.desktopImage")}>
              <Input value={s.desktop_image} onChange={(e) => setS({ ...s, desktop_image: e.target.value })} onBlur={() => void save(s)} />
            </Field>
            <Field label={t("settings.computers.desktopPort")}>
              <Input value={s.desktop_port} onChange={(e) => setS({ ...s, desktop_port: Number(e.target.value) || 0 })} onBlur={() => void save(s)} />
            </Field>
            <p className="col-span-2 text-[11.5px] text-muted">{t("settings.computers.desktopNote")}</p>
          </div>
        )}
      </Card>
      <DockerSetupCard s={s} save={save} />
        </>
      )}
      {tab === "space" && (
        <>
      <Card
        title="Perplexity SPACE"
        action={
          <span className="flex items-center gap-3">
            <button onClick={() => void openUrl(SPACE_REPO)} className="flex items-center gap-1 text-[12px] text-muted hover:text-accent" aria-label={t("computer.spaceRepo")} data-tip={t("computer.spaceRepo")}>
              <Github size={13} /> GitHub
            </button>
            <button onClick={() => void openUrl("https://space.docs.perplexity.ai/")} className="flex items-center gap-1 text-[12px] text-muted hover:text-accent">
              {t("settings.computers.spaceDocs")} <ExternalLink size={12} />
            </button>
          </span>
        }
      >
        <div className="flex items-start gap-3 px-4 pt-3.5 pb-3 text-[12.5px] text-muted">
          <Server size={16} className="mt-0.5 shrink-0" />
          <span>{s.os === "linux" ? t("settings.computers.spaceHint") : t("settings.computers.spaceRemote")}</span>
        </div>
        <SpaceSetup onChanged={() => void api.computerSettings().then(setS)} />
        <p className="px-4 pt-3 text-[10.5px] font-semibold tracking-wide text-faint uppercase">{t("settings.computers.connection")}</p>
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
        </>
      )}
      {tab === "vmpal" && <VmPalCard />}
      {tab === "more" && (
        <>
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
        </>
      )}
      <p className="mt-2 flex items-center gap-1.5 text-[12px] text-faint">
        <Box size={13} /> {t("settings.computers.perAgent")}
      </p>
    </>
  );
}

// ---------- Docker setup (isolation, idle stop, checklist) ----------

function DockerSetupCard({ s, save }: { s: ComputerSettings; save: (next: ComputerSettings) => Promise<void> }) {
  const { t } = useTranslation();
  const showToast = useStore((st) => st.showToast);
  const [setup, setSetup] = useState<DockerSetup | null>(null);
  const [busy, setBusy] = useState<"" | "pull" | "vm">("");
  const [log, setLog] = useState("");
  const [copied, setCopied] = useState(false);
  const load = () => void api.dockerSetup().then(setSetup);
  useEffect(load, [s.shared_vm, s.runtime]);
  useEffect(() => {
    const onOut = (e: Event) => {
      const ev = (e as CustomEvent<{ agent_id: string; text: string }>).detail;
      if (ev.agent_id === "docker-setup") setLog((l) => (l + ev.text).slice(-6000));
    };
    window.addEventListener("opencue:computer-output", onOut);
    return () => window.removeEventListener("opencue:computer-output", onOut);
  }, []);

  const podman = s.runtime === "podman";
  const startCmd = podman
    ? s.os === "linux"
      ? "podman info"
      : "podman machine init; podman machine start"
    : s.os === "macos"
      ? "open -a Docker"
      : s.os === "windows"
        ? "Start Docker Desktop"
        : "sudo systemctl start docker";
  // Docker on Linux needs a password, so only the others can run from here.
  const canRunStart = podman || s.os === "macos";
  const [starting, setStarting] = useState(false);
  const runStart = async () => {
    setStarting(true);
    try {
      await api.dockerStartEngine();
    } catch (e) {
      showToast(String(e));
    } finally {
      setStarting(false);
      load();
    }
  };
  const run = async (kind: "pull" | "vm") => {
    setBusy(kind);
    try {
      if (kind === "pull") await api.dockerPrepare();
      else await api.dockerStartShared();
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy("");
      load();
    }
  };

  const steps: { done: boolean; title: string; body?: React.ReactNode }[] = [
    {
      done: !!setup?.installed,
      title: podman ? t("settings.computers.stepInstallPodman") : t("settings.computers.stepInstall"),
      body: !setup?.installed && (
        <button onClick={() => void openUrl(podman ? "https://podman.io/docs/installation" : "https://docs.docker.com/get-docker/")} className="text-[12.5px] text-accent hover:underline">
          {podman ? "podman.io/docs/installation" : "docs.docker.com/get-docker"}
        </button>
      ),
    },
    {
      done: !!setup?.running,
      title: podman ? t(s.os === "linux" ? "settings.computers.stepStartPodmanLinux" : "settings.computers.stepStartPodman") : t("settings.computers.stepStart"),
      body: !setup?.running && (
        <div className="space-y-2">
        <div className="flex w-fit items-center gap-3 rounded-lg bg-hover px-3 py-2 font-mono text-[12.5px]">
          {startCmd}
          {canRunStart && (
            <button aria-label={t("settings.computers.runCommand")} data-tip={t("settings.computers.runCommand")} disabled={starting} onClick={() => void runStart()} className="text-success hover:brightness-110 disabled:opacity-50">
              {starting ? <Spinner size={13} /> : <Play size={14} fill="currentColor" />}
            </button>
          )}
          <button
            aria-label={t("common.copy")}
            onClick={() => {
              void navigator.clipboard?.writeText(startCmd);
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            }}
            className="text-muted hover:text-ink"
          >
            {copied ? <Check size={14} /> : <Copy size={14} />}
          </button>
        </div>
        {log && starting && <pre className="selectable max-h-32 overflow-auto whitespace-pre-wrap rounded-lg bg-hover px-2.5 py-2 font-mono text-[11.5px] text-muted">{log}</pre>}
        </div>
      ),
    },
    {
      done: !!setup?.image_ready,
      title: t("settings.computers.stepImage"),
      body: !setup?.image_ready && (
        <div className="space-y-2">
          <Button size="sm" onClick={() => void run("pull")} disabled={busy !== "" || !setup?.running}>
            {busy === "pull" ? t("settings.computers.downloading") : t("settings.computers.download")}
          </Button>
          {log && (
            <details className="text-[12px] text-muted">
              <summary className="cursor-pointer">{t("settings.computers.showProgress")}</summary>
              <pre className="selectable mt-1 max-h-40 overflow-auto whitespace-pre-wrap rounded-lg bg-hover px-2.5 py-2 font-mono text-[11.5px]">{log}</pre>
            </details>
          )}
        </div>
      ),
    },
    s.shared_vm
      ? {
          done: setup?.vm === "running",
          title: t("settings.computers.stepVm"),
          body: setup?.vm !== "running" && (
            <Button size="sm" variant="primary" onClick={() => void run("vm")} disabled={busy !== "" || !setup?.image_ready}>
              {busy === "vm" ? t("settings.computers.starting") : t("settings.computers.startVm")}
            </Button>
          ),
        }
      : { done: false, title: t("settings.computers.stepVmPerAgent") },
  ];

  return (
    <>
      <Card title={t("settings.computers.isolation")}>
        <div className="px-4 pt-3.5 text-[12.5px] text-muted">{t("settings.computers.isolationHint")}</div>
        <div className="grid grid-cols-2 gap-1 px-4 py-3">
          {([false, true] as const).map((shared) => (
            <button
              key={String(shared)}
              onClick={() => void save({ ...s, shared_vm: shared })}
              className={`rounded-lg border px-3 py-2 text-[13px] font-medium ${s.shared_vm === shared ? "border-accent bg-hover text-ink" : "border-line text-muted hover:bg-hover"}`}
            >
              {shared ? t("settings.computers.shared") : t("settings.computers.perAgentOption")}
            </button>
          ))}
        </div>
        <p className="px-4 pb-3 text-[11.5px] text-muted">{s.shared_vm ? t("settings.computers.sharedNote") : t("settings.computers.perAgentNote")}</p>
        <CardRow label={t("settings.computers.idle")} hint={t("settings.computers.idleHint")}>
          <span className="flex items-center gap-2">
            <Input className="w-20 text-right" value={s.idle_minutes} onChange={(e) => void save({ ...s, idle_minutes: Math.max(0, Number(e.target.value.replace(/\D/g, "")) || 0) })} />
            <span className="text-[12.5px] text-muted">{t("settings.computers.minutes")}</span>
          </span>
        </CardRow>
      </Card>
      <Card
        title={t("settings.computers.setup")}
        action={
          <button onClick={load} aria-label={t("settings.composio.refresh")} className="text-muted hover:text-ink">
            <RefreshCw size={13} />
          </button>
        }
      >
        <p className="px-4 pt-3.5 text-[12.5px] text-muted">{t("settings.computers.setupHint")}</p>
        <ol className="space-y-3.5 px-4 py-3.5">
          {steps.map((st, i) => (
            <li key={i} className="flex items-start gap-3">
              <span className={`mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-full text-[12px] ${st.done ? "bg-success text-white" : "border border-line text-muted"}`}>
                {st.done ? <Check size={13} /> : i + 1}
              </span>
              <div className="min-w-0 space-y-2">
                <span className={`block text-[13.5px] ${st.done ? "text-muted line-through" : "font-medium"}`}>{st.title}</span>
                {st.body}
              </div>
            </li>
          ))}
        </ol>
      </Card>
    </>
  );
}

// ---------- VMPal ----------

export const VMPAL_EXAMPLE = `[mcp_servers.vmpal-work]
command = "/Applications/VMPal.app/Contents/Helpers/VMPalMachine.app/Contents/MacOS/VMPalMachine"
args = ["--mcp", "E7D-your-machine-id"]`;

function VmPalCard() {
  const { t } = useTranslation();
  const [info, setInfo] = useState<VmPalInfo | null>(null);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [tests, setTests] = useState<Record<string, { busy?: boolean; ok?: boolean; text?: string }>>({});

  useEffect(() => {
    void api.vmpalGet().then((v) => {
      setInfo(v);
      setText(v.toml);
    });
  }, []);

  const save = async () => {
    setError(null);
    setSaved(false);
    try {
      const v = await api.saveVmpal(text);
      setInfo(v);
      setText(v.toml);
      setSaved(true);
    } catch (e) {
      setError(String(e));
    }
  };

  const test = async (name: string) => {
    setTests((s) => ({ ...s, [name]: { busy: true } }));
    try {
      const st = await api.testVmpal(name);
      setTests((s) => ({ ...s, [name]: st.state === "ready" ? { ok: true, text: t("settings.vmpal.ready", { n: st.tools.length }) } : { ok: false, text: st.error ?? st.state } }));
    } catch (e) {
      setTests((s) => ({ ...s, [name]: { ok: false, text: String(e) } }));
    }
  };

  return (
    <Card title="VMPal">
      <div className="space-y-3 px-4 py-3.5">
        <div className="flex items-start gap-3 text-[12.5px] text-muted">
          <MonitorCog size={16} className="mt-0.5 shrink-0" />
          <span>{t("settings.vmpal.hint")}</span>
        </div>
        <Textarea
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setSaved(false);
          }}
          rows={8}
          spellCheck={false}
          placeholder={VMPAL_EXAMPLE}
          className="font-mono text-[12px] leading-relaxed"
        />
        <div className="flex items-center gap-2">
          <Button variant="primary" size="sm" onClick={() => void save()} disabled={text.trim() === (info?.toml ?? "")}>
            {t("common.save")}
          </Button>
          {!text.trim() && (
            <Button size="sm" onClick={() => setText(VMPAL_EXAMPLE)}>
              {t("settings.vmpal.example")}
            </Button>
          )}
          {saved && <span className="text-[12px] text-success">{t("common.saved")}</span>}
        </div>
        {error && <p className="selectable text-[12px] text-danger">{error}</p>}
        {info && info.machines.length > 0 && (
          <ul className="divide-y divide-line rounded-xl border border-line">
            {info.machines.map((m) => {
              const st = tests[m.name];
              return (
                <li key={m.name} className="flex items-center gap-3 px-3 py-2">
                  <MonitorCog size={15} className="shrink-0 text-muted" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-[13px] font-medium">{m.name}</span>
                    <span className={`block truncate text-[11.5px] ${st?.ok === false ? "text-danger" : st?.ok ? "text-success" : "text-faint"}`}>{st?.text ?? m.args.join(" ")}</span>
                  </span>
                  <Button size="sm" onClick={() => void test(m.name)} disabled={st?.busy}>
                    {st?.busy ? t("settings.vmpal.starting") : t("settings.computers.test")}
                  </Button>
                </li>
              );
            })}
          </ul>
        )}
        <p className="text-[11.5px] text-faint">{t("settings.vmpal.perAgent")}</p>
      </div>
    </Card>
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
      if (v.has_key) void loadVoices();
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
        title={t("settings.voice.engines")}
        action={
          <button onClick={() => useStore.getState().openSettings("models", "local")} className="flex items-center gap-1 text-[12px] text-muted hover:text-accent">
            {t("settings.voice.localModels")}
          </button>
        }
      >
        {(["stt_engine", "tts_engine"] as const).map((k) => (
          <CardRow key={k} label={t(`settings.voice.${k}`)} hint={t(`settings.voice.${k}Hint`)}>
            <Select
              className="w-56"
              value={s[k]}
              onChange={(v) => void save({ ...s, [k]: v })}
              options={[
                { value: "auto", label: t("settings.voice.engineAuto") },
                { value: "elevenlabs", label: "ElevenLabs" },
                { value: "local", label: t("settings.voice.engineLocal") },
              ]}
            />
          </CardRow>
        ))}
        <div className="px-4 py-2.5 text-[12px] text-muted">{s.configured ? t("settings.voice.ready") : t("settings.voice.notReady")}</div>
      </Card>
      <Card
        title="ElevenLabs"
        action={
          <button onClick={() => void openUrl("https://elevenlabs.io/app/settings/api-keys")} className="flex items-center gap-1 text-[12px] text-muted hover:text-accent">
            {t("settings.voice.getKey")} <ExternalLink size={12} />
          </button>
        }
      >
        <CardRow label={t("settings.voice.key")} hint={s.has_key ? t("settings.voice.keySaved") : t("settings.voice.keyHint")}>
          <div className="flex gap-1.5">
            <Input type="password" className="w-56" value={key} placeholder={s.has_key ? "••••••••" : "sk_…"} onChange={(e) => setKey(e.target.value)} />
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
            disabled={!s.has_key}
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
