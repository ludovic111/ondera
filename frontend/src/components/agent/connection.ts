import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export const providers = {
  codex: {
    name: "Codex",
    description:
      "Use your OpenAI account through the Codex companion. No API key to copy.",
    help: "https://developers.openai.com/codex/cli/",
    destination:
      "Requests and session context are sent to OpenAI through Codex.",
  },
  claude: {
    name: "Claude Code",
    description:
      "Use your Claude account through the Claude Code companion. No API key to copy.",
    help: "https://code.claude.com/docs/en/quickstart",
    destination:
      "Requests and session context are sent to Anthropic through Claude Code.",
  },
  anthropic: {
    name: "Anthropic API",
    description:
      "Connect with an Anthropic API key. API usage is billed separately from a chat subscription.",
    help: "https://console.anthropic.com/settings/keys",
    destination: "Requests and session context are sent directly to Anthropic.",
  },
  openai: {
    name: "OpenAI API",
    description:
      "Connect with an OpenAI API key. API usage is billed separately from a chat subscription.",
    help: "https://platform.openai.com/api-keys",
    destination: "Requests and session context are sent directly to OpenAI.",
  },
  compatible: {
    name: "Local or compatible model",
    description:
      "Connect a running server such as LM Studio or Ollama. Your model must support tool calls.",
    help: "",
    destination:
      "Requests and session context go to the server address you choose.",
  },
} as const;
export type Provider = keyof typeof providers;
export const isProvider = (value: unknown): value is Provider =>
  typeof value === "string" && Object.hasOwn(providers, value);
export interface Connection {
  provider: Provider;
  state:
    | "signedIn"
    | "configured"
    | "missingCli"
    | "updateRequired"
    | "unavailable"
    | "signInRequired"
    | "missingKey"
    | "missingEndpoint"
    | "missingModel"
    | "bridgeDisabled";
  message: string;
}
export const canChat = (connection: Connection | null) =>
  connection?.state === "signedIn" || connection?.state === "configured";

export function useAgentConnection(refreshKey: unknown) {
  const [connection, setConnection] = useState<Connection | null>(null);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState("");
  const sequence = useRef(0);
  const check = useCallback(async () => {
    const request = ++sequence.current;
    setChecking(true);
    setError("");
    setConnection(null);
    try {
      const result = await invoke<Connection>("daw_agent_connection");
      if (request === sequence.current) setConnection(result);
    } catch (reason) {
      if (request === sequence.current) setError(String(reason));
    } finally {
      if (request === sequence.current) setChecking(false);
    }
  }, []);
  useEffect(() => {
    void check();
    return () => {
      sequence.current++;
    };
  }, [check, refreshKey]);
  return { connection, checking, error, check };
}

export function agentErrorMessage(error: string, unsent: boolean): string {
  const text = error.toLowerCase();
  if (
    /429|rate.?limit|usage (limit|credits)|quota|insufficient_quota/.test(text)
  )
    return "Your AI service has reached a usage limit. Check your account allowance or choose another service.";
  if (
    /401|403|unauthori[sz]ed|invalid.*(key|token)|not logged in|authentication/.test(
      text,
    )
  )
    return "Your AI service could not verify your account. Sign in again or check your API key in Agent settings.";
  if (
    /connection refused|could not connect|failed to connect|error sending request/.test(
      text,
    )
  )
    return "Ondera could not reach your AI service. Check your connection and, for a local model, make sure its server is running.";
  return unsent
    ? "Your message was not sent. It is still in the box below; check the details and try again."
    : "The agent could not finish this request. Completed edits are kept in your project and in Changes.";
}
