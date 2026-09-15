/** Marks identify the model's maker, including models served by a compatible API. */
export function modelBrand(
  provider: string,
  model = "",
): { name: string; icon: string } | null {
  const id = model.toLowerCase().split("/").pop() ?? "";
  if (/^(qwen|qwq)/.test(id)) return { name: "Qwen", icon: "qwen-color" };
  if (/^(kimi|moonshot)/.test(id)) return { name: "Kimi", icon: "kimi-color" };
  if (/^deepseek/.test(id)) return { name: "DeepSeek", icon: "deepseek-color" };
  if (/^gemini/.test(id))
    return { name: "Google Gemini", icon: "gemini-color" };
  if (/^(mistral|mixtral|codestral|devstral|magistral)/.test(id))
    return { name: "Mistral", icon: "mistral-color" };
  if (/^(llama|meta-llama)/.test(id))
    return { name: "Meta", icon: "meta-color" };
  if (/^minimax/.test(id)) return { name: "MiniMax", icon: "minimax-color" };
  if (/^grok/.test(id)) return { name: "xAI", icon: "grok" };
  if (provider === "claude" || provider === "anthropic" || /^claude/.test(id))
    return { name: "Anthropic", icon: "claude-color" };
  if (
    provider === "codex" ||
    provider === "openai" ||
    /^(gpt-|o[1-9](?:-|$))/.test(id)
  )
    return { name: "OpenAI", icon: "openai" };
  return null;
}

export function ProviderLogo({
  provider,
  model = "",
}: {
  provider: string;
  model?: string;
}) {
  const brand = modelBrand(provider, model);
  return (
    <span
      className="provider-logo"
      title={brand?.name ?? "Compatible provider"}
    >
      {brand ? (
        <img
          src={`/providers/${brand.icon}.svg`}
          alt={`${brand.name} logo`}
          width="20"
          height="20"
        />
      ) : (
        <svg
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          aria-label="Compatible provider"
        >
          <rect x="3" y="5" width="18" height="14" rx="3" />
          <path d="m8 9-3 3 3 3m8-6 3 3-3 3m-3-7-2 8" />
        </svg>
      )}
    </span>
  );
}
