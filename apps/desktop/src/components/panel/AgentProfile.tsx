import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { AudioLines, Box, Brain, ChevronLeft, ChevronRight, Cpu, FileText, Ghost, Plus, Shield, Sparkles, Trash2, X } from "lucide-react";
import { useStore } from "../../lib/store";
import { api } from "../../lib/api";
import type { Agent, AgentSettings, EmailAccount, McpServerConfig, McpServerStatus, MemoryEntry, ProviderInfo } from "../../lib/types";
import { cx } from "../../lib/format";
import { AvatarView } from "../AvatarView";
import { Button, IconButton, Input, Modal, DialogHeader, Textarea, Toggle } from "../ui";
import { confirmDialog } from "../../lib/confirm";
import { ModelPicker } from "../ModelPicker";
import { AvatarPickerDialog } from "./AvatarPicker";
import { Row } from "./Section";
import { AutomationsSection, ComputerPage, EmailPage, SkillsPage, VoicePage } from "./AgentPages";
import { SharedFiles } from "./SharedFiles";
import { useResizable } from "../ui/Resizer";

type Page = "main" | "model" | "apps" | "skills" | "instructions" | "soul" | "memory" | "voice" | "permissions" | "email" | "computer" | "files";

export function AgentProfile({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const setPanel = useStore((s) => s.setPanel);
  const providers = useStore((s) => s.providers);
  const [page, setPage] = useState<Page>("main");
  const [picker, setPicker] = useState(false);
  const [name, setName] = useState(agent.name);
  useEffect(() => setName(agent.name), [agent.id, agent.name]);
  const [email, setEmail] = useState<EmailAccount | null | undefined>(undefined);
  useEffect(() => void api.getEmailAccount(agent.id).then(setEmail), [agent.id, page]);

  const patchSettings = (s: Partial<AgentSettings>) => void api.updateAgent(agent.id, { settings: { ...agent.settings, ...s } });
  const provider = providers.find((p) => p.id === agent.provider_id);

  const titles: Record<Page, string> = {
    main: t("profile.title"),
    model: t("profile.model"),
    apps: t("profile.apps"),
    skills: t("profile.skills"),
    instructions: t("profile.instructions"),
    soul: t("profile.soul"),
    memory: t("profile.memory"),
    voice: t("profile.voice"),
    permissions: t("profile.permissions"),
    email: t("profile.mails"),
    computer: t("profile.computer"),
    files: t("files.title"),
  };

  const { width, handle } = useResizable("opencue:panel-width", 360, 300, 640, "left");

  return (
    <aside style={{ width }} className="animate-slide relative flex h-full shrink-0 flex-col border-l border-line bg-panel">
      {handle}
      <div className="flex h-13 shrink-0 items-center justify-between gap-2 px-4">
        <div className="flex min-w-0 items-center gap-1">
          {page !== "main" && (
            <IconButton label={t("common.back")} onClick={() => setPage("main")} className="-ml-1.5">
              <ChevronLeft size={18} />
            </IconButton>
          )}
          <h2 className="truncate text-[14px] font-semibold">{titles[page]}</h2>
        </div>
        <IconButton label={t("common.close")} onClick={() => setPanel(null)}>
          <X size={17} />
        </IconButton>
      </div>
      <div className="min-w-0 flex-1 overflow-x-hidden overflow-y-auto">
        {page === "main" && (
          <>
            <div className="flex flex-col items-center px-5 pt-2 pb-4">
              <button onClick={() => setPicker(true)} className="rounded-full transition-transform hover:scale-[1.03]" aria-label={t("profile.pickAvatar")} data-tip={t("profile.pickAvatar")}>
                <AvatarView avatar={agent.avatar} size={76} />
              </button>
              <AvatarPickerDialog open={picker} value={agent.avatar} onChange={(avatar) => void api.updateAgent(agent.id, { avatar })} onClose={() => setPicker(false)} />
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                onBlur={() => name.trim() && name !== agent.name && void api.updateAgent(agent.id, { name })}
                onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
                aria-label={t("profile.name")}
                className="mt-2 w-full rounded-lg bg-transparent px-2 py-1 text-center text-[17px] font-semibold tracking-tight outline-none hover:bg-hover focus:bg-hover"
              />
            </div>

            <Group label={t("profile.assets")}>
              <NavRow icon={<Emoji>✉️</Emoji>} label={t("profile.mails")} value={email?.address} onClick={() => setPage("email")} add={email === null} />
              <NavRow icon={<Emoji>🖥️</Emoji>} label={t("profile.computer")} value={agent.settings.computer === "none" ? undefined : t(`computer.backend.${agent.settings.computer}`)} onClick={() => setPage("computer")} add={agent.settings.computer === "none"} />
              <NavRow icon={<Emoji>📎</Emoji>} label={t("files.title")} onClick={() => setPage("files")} />
            </Group>

            <Group label={t("profile.configurations")}>
              <NavRow icon={<Cpu size={17} />} label={t("profile.model")} value={agent.model ?? provider?.default_model ?? undefined} onClick={() => setPage("model")} />
              <NavRow icon={<Box size={17} />} label={t("profile.apps")} onClick={() => setPage("apps")} />
              <NavRow icon={<Sparkles size={17} />} label={t("profile.skills")} value={agent.settings.skills ? undefined : t("common.off")} onClick={() => setPage("skills")} />
              <NavRow icon={<FileText size={17} />} label={t("profile.instructions")} onClick={() => setPage("instructions")} />
              <NavRow icon={<Ghost size={17} />} label={t("profile.soul")} onClick={() => setPage("soul")} />
              <NavRow icon={<Brain size={17} />} label={t("profile.memory")} onClick={() => setPage("memory")} />
              <NavRow icon={<AudioLines size={17} />} label={t("profile.voice")} onClick={() => setPage("voice")} />
              <NavRow icon={<Shield size={17} />} label={t("profile.permissions")} value={agent.settings.approval_mode === "ask" ? t("profile.askShort") : t("profile.autonomousShort")} onClick={() => setPage("permissions")} />
            </Group>

            <AutomationsSection agent={agent} />

            <div className="px-5 pt-2 pb-5">
              <Button
                variant="danger"
                className="w-full"
                onClick={async () => {
                  if (await confirmDialog(t("profile.deleteConfirm", { name: agent.name }), { confirmLabel: t("common.delete") })) {
                    setPanel(null);
                    void api.deleteAgent(agent.id);
                  }
                }}
              >
                <Trash2 size={15} /> {t("profile.deleteAgent")}
              </Button>
            </div>
          </>
        )}
        {page === "model" && <ModelSection agent={agent} />}
        {page === "apps" && <AppsSection agent={agent} onChange={patchSettings} />}
        {page === "skills" && <SkillsPage agent={agent} onChange={patchSettings} />}
        {page === "instructions" && <ProfileDoc agent={agent} kind="instructions" onChange={patchSettings} />}
        {page === "soul" && <ProfileDoc agent={agent} kind="soul" onChange={patchSettings} />}
        {page === "memory" && <MemorySection agent={agent} onChange={patchSettings} />}
        {page === "voice" && <VoicePage agent={agent} onChange={patchSettings} />}
        {page === "email" && <EmailPage agent={agent} />}
        {page === "computer" && <ComputerPage agent={agent} onChange={patchSettings} />}
        {page === "files" && (
          <div className="px-5 py-3">
            <SharedFiles threadId={agent.id} />
          </div>
        )}
        {page === "permissions" && (
          <Page>
            <div className="space-y-1.5">
              {(["ask", "autonomous"] as const).map((mode) => (
                <button
                  key={mode}
                  onClick={() => patchSettings({ approval_mode: mode })}
                  className={cx(
                    "flex w-full items-start gap-2.5 rounded-xl border px-3 py-2 text-left transition-colors",
                    agent.settings.approval_mode === mode ? "border-accent bg-accent-soft" : "border-line hover:bg-hover",
                  )}
                >
                  <span className={cx("mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border-2", agent.settings.approval_mode === mode ? "border-accent" : "border-line-strong")}>
                    {agent.settings.approval_mode === mode && <span className="size-1.5 rounded-full bg-accent" />}
                  </span>
                  <span>
                    <span className="block text-[13px] font-medium">{mode === "ask" ? t("profile.askFirst") : t("profile.autonomous")}</span>
                    <span className="block text-[11.5px] text-muted">{mode === "ask" ? t("profile.askFirstHint") : t("profile.autonomousHint")}</span>
                  </span>
                </button>
              ))}
            </div>
          </Page>
        )}
      </div>
    </aside>
  );
}

function Emoji({ children }: { children: string }) {
  return <span className="text-[17px] leading-none">{children}</span>;
}

function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="px-5 pb-3">
      <div className="mb-1 text-[12.5px] font-medium text-muted">{label}</div>
      <div className="divide-y divide-line">{children}</div>
    </div>
  );
}

function NavRow({ icon, label, value, onClick, add }: { icon: ReactNode; label: string; value?: string; onClick: () => void; add?: boolean }) {
  const { t } = useTranslation();
  return (
    <button onClick={onClick} className="group flex w-full items-center gap-3 py-2.5 text-left">
      <span className="flex w-5 justify-center text-ink">{icon}</span>
      <span className="flex-1 text-[13.5px]">{label}</span>
      {add ? (
        <span className="flex items-center gap-1 rounded-full border border-line-strong px-2.5 py-0.5 text-[12.5px] group-hover:bg-hover">
          <Plus size={13} /> {t("common.add")}
        </span>
      ) : (
        <>
          {value && <span className="max-w-[150px] truncate text-[12.5px] text-muted">{value}</span>}
          <ChevronRight size={16} className="text-faint group-hover:text-muted" />
        </>
      )}
    </button>
  );
}

function Page({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <div className="px-5 pt-1 pb-5">
      {action && <div className="mb-2 flex justify-end">{action}</div>}
      {children}
    </div>
  );
}

function ModelSection({ agent }: { agent: Agent }) {
  const { t } = useTranslation();
  const providers = useStore((s) => s.providers);
  const [catalog, setCatalog] = useState<ProviderInfo[]>([]);
  const provider = providers.find((p) => p.id === agent.provider_id);
  const info = catalog.find((c) => c.kind === provider?.kind);
  useEffect(() => void api.providerCatalog().then(setCatalog), []);

  return (
    <Page>
      <ModelPicker
        providerId={agent.provider_id}
        model={agent.model}
        onChange={(provider_id, model) => void api.updateAgent(agent.id, { provider_id, model })}
      />
      {info && info.api === "cli" && (
        <div className="mt-3 flex items-start justify-between gap-3 rounded-xl border border-line px-3 py-2.5">
          <span>
            <span className="block text-[13px] font-medium">{t("profile.cliTools")}</span>
            <span className="block text-[11.5px] text-muted">{t("profile.cliToolsHint")}</span>
          </span>
          <Toggle checked={agent.settings.cli_tools !== false} onChange={(v) => void api.updateAgent(agent.id, { settings: { ...agent.settings, cli_tools: v } })} />
        </div>
      )}
    </Page>
  );
}

function AppsSection({ agent, onChange }: { agent: Agent; onChange: (s: Partial<AgentSettings>) => void }) {
  const { t } = useTranslation();
  const openSettings = useStore((s) => s.openSettings);
  const [servers, setServers] = useState<McpServerConfig[]>([]);
  const [status, setStatus] = useState<McpServerStatus[]>([]);

  useEffect(() => {
    const load = () => {
      void api.listMcpServers().then(setServers);
      void api.mcpStatus().then(setStatus);
    };
    load();
    window.addEventListener("opencue:mcp-changed", load);
    return () => window.removeEventListener("opencue:mcp-changed", load);
  }, []);

  const s = agent.settings;
  const enabledServers = servers.filter((x) => x.enabled && !x.id.startsWith("vmpal:"));
  const toggleServer = (id: string, on: boolean) => {
    const base = s.mcp_all ? enabledServers.map((x) => x.id) : s.mcp_servers;
    const next = on ? [...new Set([...base, id])] : base.filter((x) => x !== id);
    onChange({ mcp_all: false, mcp_servers: next });
  };

  return (
    <Page>
      <Row label={t("profile.webAccess")}>
        <Toggle checked={s.web_access} onChange={(v) => onChange({ web_access: v })} label={t("profile.webAccess")} />
      </Row>
      <Row label={t("profile.composio")} hint={t("profile.composioHint")}>
        <Toggle checked={s.composio} onChange={(v) => onChange({ composio: v })} label={t("profile.composio")} />
      </Row>
      <div className="mt-2 mb-1 text-[12px] font-medium text-muted">MCP</div>
      {enabledServers.length === 0 ? (
        <button onClick={() => openSettings("apps")} className="mt-1 text-left text-[12px] text-muted hover:text-accent">
          {t("profile.noMcp")}
        </button>
      ) : (
        <>
          <Row label={t("profile.mcpAll")}>
            <Toggle checked={s.mcp_all} onChange={(v) => onChange({ mcp_all: v })} label={t("profile.mcpAll")} />
          </Row>
          <div className="mt-1 space-y-0.5">
            {enabledServers.map((srv) => {
              const st = status.find((x) => x.id === srv.id);
              const on = s.mcp_all || s.mcp_servers.includes(srv.id);
              return (
                <div key={srv.id} className="flex items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-hover">
                  <span className={cx("size-1.5 shrink-0 rounded-full", st?.state === "ready" ? "bg-success" : st?.state === "error" ? "bg-danger" : "bg-faint")} />
                  <span className="min-w-0 flex-1 truncate text-[13px]">{srv.name}</span>
                  {st?.state === "ready" && <span className="text-[11px] text-faint">{t("settings.apps.tools", { n: st.tools.length })}</span>}
                  <Toggle checked={on} onChange={(v) => toggleServer(srv.id, v)} label={srv.name} />
                </div>
              );
            })}
          </div>
        </>
      )}
    </Page>
  );
}

/** INSTRUCTIONS.md or SOUL.md: edited here, and rewritten by the agent as the user talks to it. */
function ProfileDoc({ agent, kind, onChange: patch }: { agent: Agent; kind: "instructions" | "soul"; onChange: (s: Partial<AgentSettings>) => void }) {
  const { t } = useTranslation();
  const [text, setText] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const messageCount = useStore((s) => s.messages[agent.id]?.length ?? 0);
  const get = kind === "soul" ? api.getSoul : api.getInstructions;
  const set = kind === "soul" ? api.setSoul : api.setInstructions;

  const load = useCallback(() => void get(agent.id).then((v) => !timer.current && setText(v)), [agent.id, get]);
  useEffect(() => {
    setText(null);
    load();
  }, [agent.id, kind]); // eslint-disable-line react-hooks/exhaustive-deps
  // The agent may have rewritten it: reload after each reply, unless the user is mid-edit.
  useEffect(load, [messageCount, load]);
  useEffect(() => {
    const on = (e: Event) => (e as CustomEvent).detail === agent.id && load();
    window.addEventListener("opencue:profile-changed", on);
    return () => window.removeEventListener("opencue:profile-changed", on);
  }, [agent.id, load]);

  const onChange = (v: string) => {
    setText(v);
    setSaved(false);
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      await set(agent.id, v);
      timer.current = null;
      setSaved(true);
    }, 600);
  };

  return (
    <Page action={saved ? <span className="text-[11.5px] text-faint">{t("common.saved")}</span> : null}>
      <p className="mb-2 text-[12px] text-muted">{kind === "soul" ? t("profile.soulHint") : t("profile.instructionsHint")}</p>
      <Textarea
        value={text ?? ""}
        disabled={text === null}
        onChange={(e) => onChange(e.target.value)}
        placeholder={kind === "soul" ? t("profile.soulPlaceholder", { name: agent.name }) : t("profile.instructionsPlaceholder", { name: agent.name })}
        rows={16}
        className="min-h-64 resize-y"
      />
      <div className="mt-3">
        <Row label={t("profile.autoProfile")} hint={t("profile.autoProfileHint")}>
          <Toggle checked={agent.settings.auto_profile !== false} onChange={(v) => patch({ auto_profile: v })} label={t("profile.autoProfile")} />
        </Row>
      </div>
    </Page>
  );
}

function MemorySection({ agent, onChange }: { agent: Agent; onChange: (s: Partial<AgentSettings>) => void }) {
  const { t } = useTranslation();
  const [entries, setEntries] = useState<MemoryEntry[]>([]);
  const [draft, setDraft] = useState("");
  const [raw, setRaw] = useState<string | null>(null);
  const messages = useStore((s) => s.messages[agent.id]);

  const load = useCallback(() => void api.listMemory("agent", agent.id).then(setEntries), [agent.id]);
  // Reload when the conversation changes: the agent may have remembered something.
  useEffect(load, [load, messages?.length]);

  const add = async () => {
    if (!draft.trim()) return;
    await api.addMemory("agent", draft.trim(), agent.id);
    setDraft("");
    load();
  };

  return (
    <Page
      action={
        <button onClick={() => void api.getMemoryRaw("agent", agent.id).then(setRaw)} className="text-[11.5px] text-muted hover:text-accent">
          {t("profile.memoryRaw")}
        </button>
      }
    >
      {entries.length === 0 ? (
        <p className="text-[12.5px] text-faint">{t("profile.memoryEmpty")}</p>
      ) : (
        <ul className="space-y-1">
          {entries.map((e) => (
            <li key={`${e.id}-${e.text}`} className="group flex items-start gap-2 rounded-lg bg-hover/60 px-2.5 py-1.5 text-[12.5px]">
              <span className="selectable min-w-0 flex-1">{e.text}</span>
              <button
                onClick={() => void api.deleteMemory("agent", e.id, agent.id).then(load)}
                className="text-faint opacity-0 transition-opacity group-hover:opacity-100 hover:text-danger"
                aria-label={t("common.delete")}
              >
                <X size={13} />
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="mt-2 flex gap-1.5">
        <Input value={draft} onChange={(e) => setDraft(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void add()} placeholder={t("profile.memoryAdd")} className="h-8 text-[12.5px]" />
        <IconButton label={t("common.add")} onClick={() => void add()} disabled={!draft.trim()} className="border border-line">
          <Plus size={15} />
        </IconButton>
      </div>
      <div className="mt-2">
        <Row label={t("profile.useGlobalMemory")}>
          <Toggle checked={agent.settings.use_global_memory} onChange={(v) => onChange({ use_global_memory: v })} label={t("profile.useGlobalMemory")} />
        </Row>
        <Row label={t("profile.autoMemory")}>
          <Toggle checked={agent.settings.auto_memory} onChange={(v) => onChange({ auto_memory: v })} label={t("profile.autoMemory")} />
        </Row>
      </div>

      <Modal open={raw !== null} onClose={() => setRaw(null)} className="w-[560px]" label={t("profile.memoryRaw")}>
        <DialogHeader title={`${t("profile.memory")} · ${agent.name}`} subtitle="MEMORY.md" onClose={() => setRaw(null)} />
        <div className="px-6 pb-5">
          <Textarea value={raw ?? ""} onChange={(e) => setRaw(e.target.value)} rows={14} className="font-mono text-[12.5px]" />
          <div className="mt-3 flex justify-end gap-2">
            <Button onClick={() => setRaw(null)}>{t("common.cancel")}</Button>
            <Button
              variant="primary"
              onClick={async () => {
                await api.setMemoryRaw("agent", raw ?? "", agent.id);
                setRaw(null);
                load();
              }}
            >
              {t("common.save")}
            </Button>
          </div>
        </div>
      </Modal>
    </Page>
  );
}
