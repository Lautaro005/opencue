import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileJson, Globe, Plug, TerminalSquare, Trash2 } from "lucide-react";
import { useStore } from "../../lib/store";
import { api, isTauri, pickJsonFile } from "../../lib/api";
import type { McpServerConfig, McpServerStatus } from "../../lib/types";
import { cx } from "../../lib/format";
import { Badge, Button, Field, Input, Textarea, Toggle } from "../ui";
import { PageTitle } from "./SettingsModal";
import { CatalogCard } from "./CatalogSection";

type Mode = null | "stdio" | "http" | "paste";

const lines = (s: string) => s.split("\n").map((l) => l.trim()).filter(Boolean);
const toPairs = (s: string, sep: string) =>
  Object.fromEntries(
    lines(s)
      .map((l) => {
        const i = l.indexOf(sep);
        return i > 0 ? [l.slice(0, i).trim(), l.slice(i + 1).trim()] : null;
      })
      .filter((x): x is [string, string] => !!x),
  );

export function AppsSection() {
  const { t } = useTranslation();
  const showToast = useStore((s) => s.showToast);
  const [servers, setServers] = useState<McpServerConfig[]>([]);
  const [status, setStatus] = useState<McpServerStatus[]>([]);
  const [mode, setMode] = useState<Mode>(null);

  const load = () => {
    void api.listMcpServers().then((l) => setServers(l.filter((s) => !s.id.startsWith("vmpal:") )));
    void api.mcpStatus().then(setStatus);
  };
  useEffect(() => {
    load();
    window.addEventListener("opencue:mcp-changed", load);
    return () => window.removeEventListener("opencue:mcp-changed", load);
  }, []);

  const importFile = async () => {
    const path = await pickJsonFile();
    if (!path) return;
    try {
      const saved = await api.importMcpFile(path);
      showToast(`Imported ${saved.length} server(s)`);
      load();
    } catch (e) {
      showToast(String(e));
    }
  };

  return (
    <>
      <PageTitle title={t("settings.apps.title")} hint={t("settings.apps.hint")} />
      <div className="mb-2 flex items-center justify-between">
        <h3 className="text-[12.5px] font-semibold tracking-wide text-muted uppercase">{t("settings.apps.mcp")}</h3>
        <div className="flex gap-1.5">
          <Button size="sm" onClick={() => setMode("stdio")}>
            <TerminalSquare size={13} /> {t("settings.apps.addLocal")}
          </Button>
          <Button size="sm" onClick={() => setMode("http")}>
            <Globe size={13} /> {t("settings.apps.addUrl")}
          </Button>
          <Button size="sm" onClick={() => (isTauri ? void importFile() : setMode("paste"))}>
            <FileJson size={13} /> {t("settings.apps.importFile")}
          </Button>
        </div>
      </div>

      {mode && <ServerForm mode={mode} onDone={() => (setMode(null), load())} />}

      {servers.length === 0 && !mode ? (
        <div className="rounded-xl border border-dashed border-line-strong px-4 py-8 text-center text-[13px] text-muted">
          {t("settings.apps.empty")}{" "}
          <button className="text-accent hover:underline" onClick={() => setMode("paste")}>
            {t("settings.apps.pasteJson")}
          </button>
        </div>
      ) : (
        <div className="space-y-2">
          {servers.map((s) => {
            const st = status.find((x) => x.id === s.id);
            const state = st?.state ?? "stopped";
            return (
              <div key={s.id} className="rounded-xl border border-line px-4 py-3">
                <div className="flex items-center gap-3">
                  <span className="flex size-8 items-center justify-center rounded-lg bg-hover text-muted">{s.transport === "http" ? <Globe size={15} /> : <Plug size={15} />}</span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-[13.5px] font-medium">{s.name}</span>
                      <Badge tone={state === "ready" ? "success" : state === "error" ? "danger" : "neutral"}>
                        <span className={cx("size-1.5 rounded-full", state === "ready" ? "bg-success" : state === "error" ? "bg-danger" : state === "connecting" ? "bg-warning" : "bg-faint")} />
                        {t(`settings.apps.state.${state}`)}
                      </Badge>
                      {state === "ready" && <span className="text-[11.5px] text-faint">{t("settings.apps.tools", { n: st!.tools.length })}</span>}
                    </div>
                    <div className="selectable truncate font-mono text-[11.5px] text-muted">{s.transport === "http" ? s.url : [s.command, ...s.args].join(" ")}</div>
                  </div>
                  <Toggle checked={s.enabled} onChange={(v) => void api.saveMcpServer({ ...s, enabled: v }).then(load)} label={s.name} />
                  <button onClick={() => void api.deleteMcpServer(s.id).then(load)} className="text-faint hover:text-danger" aria-label={t("common.delete")}>
                    <Trash2 size={15} />
                  </button>
                </div>
                {st?.error && <p className="selectable mt-2 line-clamp-3 rounded-lg bg-danger-soft px-2.5 py-1.5 text-[12px] text-danger">{st.error}</p>}
                {state === "ready" && st!.tools.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-1">
                    {st!.tools.slice(0, 14).map((tool) => (
                      <span key={tool} className="rounded-md bg-hover px-1.5 py-0.5 font-mono text-[11px] text-muted">
                        {tool}
                      </span>
                    ))}
                    {st!.tools.length > 14 && <span className="px-1 text-[11px] text-faint">+{st!.tools.length - 14}</span>}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}

      <div className="mt-8">
        <CatalogCard />
      </div>
    </>
  );
}

function ServerForm({ mode, onDone }: { mode: Exclude<Mode, null>; onDone: () => void }) {
  const { t } = useTranslation();
  const showToast = useStore((s) => s.showToast);
  const [name, setName] = useState("");
  const [command, setCommand] = useState("npx");
  const [args, setArgs] = useState("");
  const [env, setEnv] = useState("");
  const [url, setUrl] = useState("");
  const [headers, setHeaders] = useState("");
  const [json, setJson] = useState("");

  const submit = async () => {
    try {
      if (mode === "paste") {
        const saved = await api.importMcpJson(json);
        showToast(`Imported ${saved.length} server(s)`);
      } else {
        await api.saveMcpServer({
          id: "",
          name: name.trim(),
          transport: mode,
          command: mode === "stdio" ? command.trim() : null,
          args: mode === "stdio" ? lines(args) : [],
          env: mode === "stdio" ? toPairs(env, "=") : {},
          url: mode === "http" ? url.trim() : null,
          headers: mode === "http" ? toPairs(headers, ":") : {},
          enabled: true,
        });
      }
      onDone();
    } catch (e) {
      showToast(String(e));
    }
  };

  return (
    <div className="animate-pop mb-4 space-y-3 rounded-xl border border-line-strong p-4">
      {mode === "paste" ? (
        <Textarea value={json} onChange={(e) => setJson(e.target.value)} rows={8} placeholder={t("settings.apps.pastePlaceholder")} className="font-mono text-[12px]" />
      ) : (
        <>
          <Field label={t("settings.apps.name")}>
            <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder={mode === "stdio" ? "filesystem" : "my-remote-server"} />
          </Field>
          {mode === "stdio" ? (
            <>
              <Field label={t("settings.apps.command")}>
                <Input value={command} onChange={(e) => setCommand(e.target.value)} className="font-mono" />
              </Field>
              <Field label={t("settings.apps.args")}>
                <Textarea value={args} onChange={(e) => setArgs(e.target.value)} rows={3} className="font-mono text-[12px]" placeholder={"-y\n@modelcontextprotocol/server-filesystem\n/home/me/Documents"} />
              </Field>
              <Field label={t("settings.apps.env")}>
                <Textarea value={env} onChange={(e) => setEnv(e.target.value)} rows={2} className="font-mono text-[12px]" placeholder="API_TOKEN=…" />
              </Field>
            </>
          ) : (
            <>
              <Field label={t("settings.apps.url")}>
                <Input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://example.com/mcp" className="font-mono" />
              </Field>
              <Field label={t("settings.apps.headers")}>
                <Textarea value={headers} onChange={(e) => setHeaders(e.target.value)} rows={2} className="font-mono text-[12px]" placeholder="Authorization: Bearer …" />
              </Field>
            </>
          )}
        </>
      )}
      <div className="flex justify-end gap-2">
        <Button onClick={onDone}>{t("common.cancel")}</Button>
        <Button variant="primary" onClick={() => void submit()} disabled={mode === "paste" ? !json.trim() : !name.trim() || (mode === "http" ? !url.trim() : !command.trim())}>
          {mode === "paste" ? t("settings.apps.import") : t("common.add")}
        </Button>
      </div>
    </div>
  );
}
