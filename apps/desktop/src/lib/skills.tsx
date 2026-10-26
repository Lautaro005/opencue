import { Bot, ClipboardList, Code2, LayoutDashboard, LayoutPanelLeft, Scale, Slash, Sparkles, Workflow } from "lucide-react";

export interface SystemSkill {
  name: string;
  /** i18n key of the description. */
  desc: string;
  icon: string;
}

/** The commands OpenCue itself understands, in the order they are listed. */
export function systemSkills(opts: { dashboard: boolean; group: boolean }): SystemSkill[] {
  // /dashboard is always listed; only the agent chosen in Settings → General can run it.
  return [
    { name: "plan", desc: "slash.plan", icon: "plan" },
    { name: "dashboard", desc: opts.dashboard ? "slash.dashboard" : "slash.dashboardSet", icon: "dashboard" },
    { name: "create-skill", desc: "slash.createSkill", icon: "skill" },
    { name: "create-automation", desc: "slash.createAutomation", icon: "automation" },
    { name: "artifact", desc: "slash.artifact", icon: "artifact" },
    { name: "canvas", desc: "slash.canvas", icon: "canvas" },
    ...(opts.group ? [{ name: "ai-council", desc: "slash.aiCouncil", icon: "council" }] : []),
  ];
}

export function SkillIcon({ name, size = 14 }: { name: string; size?: number }) {
  switch (name) {
    case "plan":
      return <ClipboardList size={size} />;
    case "dashboard":
      return <LayoutDashboard size={size} />;
    case "skill":
      return <Sparkles size={size} />;
    case "automation":
      return <Workflow size={size} />;
    case "artifact":
      return <Code2 size={size} />;
    case "canvas":
      return <LayoutPanelLeft size={size} />;
    case "council":
      return <Scale size={size} />;
    case "agent":
      return <Bot size={size} />;
    default:
      return <Slash size={size} />;
  }
}
