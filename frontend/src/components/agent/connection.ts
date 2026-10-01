import { useCallback, useEffect, useRef, useState } from "react";
import { native } from "../../state/native";

/**
 * Every agent service, as Settings > Agent shows it. `kind` decides the form: an account
 * signed in through a companion app, an API key, a server on this computer, or any address.
 * Mirrors `Provider` in engine/src/settings.rs (keys, default models, key fields).
 */
type ProviderInfo = {
  name: string;
  description: string;
  help: string;
  destination: string;
  kind: "account" | "key" | "local" | "endpoint";
  /** The settings field holding its key. */
  keyPath?: string;
  /** The settings field holding its address, and the address when it is blank. */
  urlPath?: string;
  defaultUrl?: string;
  /** The model used when none is chosen; empty means one must be chosen. */
  defaultModel: string;
};
const hosted = (
  name: string,
  description: string,
  help: string,
  keyPath: string,
  defaultModel = "",
): ProviderInfo => ({
  name,
  description,
  help,
  destination: `Requests and session context are sent directly to ${name}.`,
  kind: "key",
  keyPath,
  defaultModel,
});
export const providers = {
  codex: {
    name: "Codex",
    description:
      "Use your OpenAI account through the Codex companion. No API key to copy.",
    help: "https://developers.openai.com/codex/cli/",
    destination:
      "Requests and session context are sent to OpenAI through Codex.",
    kind: "account",
    defaultModel: "",
  },
  claude: {
    name: "Claude Code",
    description:
      "Use your Claude account through the Claude Code companion. No API key to copy.",
    help: "https://code.claude.com/docs/en/quickstart",
    destination:
      "Requests and session context are sent to Anthropic through Claude Code.",
    kind: "account",
    defaultModel: "",
  },
  anthropic: {
    ...hosted(
      "Anthropic API",
      "Connect with an Anthropic API key. API usage is billed separately from a chat subscription.",
      "https://console.anthropic.com/settings/keys",
      "anthropicApiKey",
      "claude-sonnet-5",
    ),
    destination: "Requests and session context are sent directly to Anthropic.",
  },
  openai: {
    ...hosted(
      "OpenAI API",
      "Connect with an OpenAI API key. API usage is billed separately from a chat subscription.",
      "https://platform.openai.com/api-keys",
      "openaiApiKey",
      "gpt-5",
    ),
    destination: "Requests and session context are sent directly to OpenAI.",
  },
  gemini: hosted(
    "Google Gemini",
    "Connect with a Gemini API key from Google AI Studio.",
    "https://aistudio.google.com/apikey",
    "geminiApiKey",
    "gemini-flash-latest",
  ),
  openrouter: hosted(
    "OpenRouter",
    "One key for models from every lab: Claude, GPT, Gemini, Llama, Qwen, DeepSeek and more.",
    "https://openrouter.ai/settings/keys",
    "openrouterApiKey",
    "openrouter/auto",
  ),
  mistral: hosted(
    "Mistral",
    "Connect with a Mistral API key from La Plateforme.",
    "https://console.mistral.ai/api-keys",
    "mistralApiKey",
    "mistral-large-latest",
  ),
  groq: hosted(
    "Groq",
    "Very fast open models with a Groq API key. Choose one that supports tools.",
    "https://console.groq.com/keys",
    "groqApiKey",
  ),
  deepseek: hosted(
    "DeepSeek",
    "Connect with a DeepSeek API key.",
    "https://platform.deepseek.com/api_keys",
    "deepseekApiKey",
    "deepseek-chat",
  ),
  xai: hosted(
    "xAI Grok",
    "Connect with an xAI API key to use Grok.",
    "https://console.x.ai",
    "xaiApiKey",
  ),
  ollama: {
    name: "Ollama",
    description:
      "Models running in Ollama on this computer. Nothing leaves your machine. Choose a model that supports tools.",
    help: "https://ollama.com/download",
    destination:
      "Requests and session context stay on this computer, in Ollama.",
    kind: "local",
    urlPath: "ollamaBaseUrl",
    defaultUrl: "http://127.0.0.1:11434/v1",
    defaultModel: "",
  },
  lmstudio: {
    name: "LM Studio",
    description:
      "Models running in LM Studio on this computer. Nothing leaves your machine. Choose a model that supports tools.",
    help: "https://lmstudio.ai",
    destination:
      "Requests and session context stay on this computer, in LM Studio.",
    kind: "local",
    urlPath: "lmstudioBaseUrl",
    defaultUrl: "http://127.0.0.1:1234/v1",
    defaultModel: "",
  },
  compatible: {
    name: "Other compatible server",
    description:
      "Any server that speaks the OpenAI chat API with tool calls, local or hosted.",
    help: "",
    destination:
      "Requests and session context go to the server address you choose.",
    kind: "endpoint",
    keyPath: "compatibleApiKey",
    urlPath: "compatibleBaseUrl",
    defaultModel: "",
  },
} satisfies Record<string, ProviderInfo>;
/** How the service picker groups them. */
export const providerGroups: [string, Provider[]][] = [
  ["Your account", ["codex", "claude"]],
  [
    "API key",
    [
      "anthropic",
      "openai",
      "gemini",
      "openrouter",
      "mistral",
      "groq",
      "deepseek",
      "xai",
    ],
  ],
  ["On this computer", ["ollama", "lmstudio"]],
  ["Other", ["compatible"]],
];
export const providerInfo = (provider: Provider): ProviderInfo =>
  providers[provider];
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
      const result = await native<Connection>("agent.connection");
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
    return "ryolune could not reach your AI service. Check your connection and, for a local model, make sure its server is running.";
  return unsent
    ? "Your message was not sent. It is still in the box below; check the details and try again."
    : "The agent could not finish this request. Completed edits are kept in your project and in Changes.";
}
