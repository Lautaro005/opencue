import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { MessageCircle } from "lucide-react";
import { api } from "../../lib/api";
import { useStore } from "../../lib/store";
import type { MessengerStatus } from "../../lib/types";
import { AvatarView } from "../AvatarView";
import { Badge, Button, Field, Input, Select, Spinner } from "../ui";
import { Card, PageTitle } from "./SettingsModal";

/** Settings → Messenger: link a WhatsApp to an agent, scanning a QR from the phone. */
export function MessengerSection() {
  const { t } = useTranslation();
  const agents = useStore((s) => s.agents);
  const showToast = useStore((s) => s.showToast);
  const [st, setSt] = useState<MessengerStatus | null>(null);
  const [agentId, setAgentId] = useState("");
  const [allow, setAllow] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(() => {
    void api.messengerStatus().then((s) => {
      setSt(s);
    });
  }, []);
  useEffect(() => {
    load();
    window.addEventListener("opencue:messenger-changed", load);
    return () => window.removeEventListener("opencue:messenger-changed", load);
  }, [load]);
  // Fill the form once from what is saved.
  useEffect(() => {
    if (st) {
      setAgentId((a) => a || st.agent_id || "");
      setAllow((a) => a || st.allow);
    }
  }, [st?.agent_id]); // eslint-disable-line react-hooks/exhaustive-deps

  const list = Object.values(agents);
  const linked = st?.state === "connected";
  const running = st && st.state !== "off" && st.state !== "error";

  const save = async (next: { agent?: string; allow?: string }) => {
    const a = next.agent ?? agentId;
    const al = next.allow ?? allow;
    await api.whatsappConfigure(a || null, al);
  };

  const connect = async () => {
    setBusy(true);
    try {
      await save({});
      await api.whatsappConnect();
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
      load();
    }
  };

  const disconnect = async () => {
    setBusy(true);
    try {
      await api.whatsappDisconnect();
    } catch (e) {
      showToast(String(e));
    } finally {
      setBusy(false);
      load();
    }
  };

  const badge = !st ? null : linked ? (
    <Badge tone="success">{t("settings.messenger.connected", { phone: st.phone ? `+${st.phone}` : "" })}</Badge>
  ) : st.state === "qr" ? (
    <Badge tone="warning">{t("settings.messenger.scan")}</Badge>
  ) : st.state === "starting" ? (
    <Badge>{t("settings.messenger.starting")}</Badge>
  ) : st.state === "error" ? (
    <Badge tone="danger">{t("settings.messenger.error")}</Badge>
  ) : (
    <Badge>{t("settings.messenger.off")}</Badge>
  );

  return (
    <>
      <PageTitle title={t("settings.messenger.title")} hint={t("settings.messenger.hint")} />
      <Card title="WhatsApp">
        <div className="space-y-4 px-4 py-4">
          <div className="flex items-center gap-2.5">
            <span className="flex size-9 items-center justify-center rounded-xl bg-success/15 text-success">
              <MessageCircle size={19} />
            </span>
            <div className="min-w-0 flex-1">
              <div className="text-[14px] font-semibold">WhatsApp</div>
              <div className="text-[12px] text-muted">{t("settings.messenger.whatsappHint")}</div>
            </div>
            {badge}
          </div>

          <Field label={t("settings.messenger.agent")} hint={t("settings.messenger.agentHint")}>
            <Select
              value={agentId}
              onChange={(v) => {
                setAgentId(v);
                void save({ agent: v });
              }}
              placeholder={t("settings.messenger.chooseAgent")}
              options={list.map((a) => ({ value: a.id, label: a.name }))}
              className="w-64"
            />
          </Field>
          <Field label={t("settings.messenger.allow")} hint={t("settings.messenger.allowHint")}>
            <Input value={allow} onChange={(e) => setAllow(e.target.value)} onBlur={() => void save({ allow })} placeholder="+54 9 11 …" spellCheck={false} />
          </Field>

          {st?.state === "qr" && st.qr_svg && (
            <div className="flex items-start gap-4 rounded-xl border border-line p-4">
              <div className="size-[200px] shrink-0 overflow-hidden rounded-lg bg-white p-1" dangerouslySetInnerHTML={{ __html: st.qr_svg.replace(/width="\d+"/, 'width="100%"').replace(/height="\d+"/, 'height="100%"') }} />
              <ol className="list-decimal space-y-1.5 pl-4 text-[12.5px] text-muted">
                <li>{t("settings.messenger.step1")}</li>
                <li>{t("settings.messenger.step2")}</li>
                <li>{t("settings.messenger.step3")}</li>
              </ol>
            </div>
          )}
          {st?.error && <p className="text-[12.5px] text-danger">{st.error}</p>}

          <div className="flex items-center gap-2">
            {running || linked ? (
              <Button onClick={() => void disconnect()} disabled={busy}>
                {t(linked ? "settings.messenger.disconnect" : "settings.messenger.cancel")}
              </Button>
            ) : (
              <Button variant="primary" onClick={() => void connect()} disabled={busy || !agentId}>
                {busy ? <Spinner size={13} /> : t("settings.messenger.link")}
              </Button>
            )}
            {agentId && agents[agentId] && (
              <span className="flex items-center gap-1.5 text-[12px] text-muted">
                <AvatarView avatar={agents[agentId].avatar} name={agents[agentId].name} size={18} /> {t("settings.messenger.answers", { name: agents[agentId].name })}
              </span>
            )}
          </div>
          <p className="text-[11.5px] text-faint">{t("settings.messenger.privacy")}</p>
        </div>
      </Card>
    </>
  );
}
