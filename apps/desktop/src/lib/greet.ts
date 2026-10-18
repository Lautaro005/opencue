import i18n from "../i18n";
import { api } from "./api";
import type { Agent } from "./types";

/** A new agent's first message: it asks what its job will be, then writes its own profile. */
export async function greetNewAgent(agent: Agent, hadInstructions: boolean) {
  const text = i18n.t(hadInstructions ? "greet.ready" : "greet.ask", { name: agent.name });
  try {
    await api.greetAgent(agent.id, text);
  } catch {
    /* the chat just starts empty */
  }
}
