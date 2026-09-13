# The Ondera agent

Ondera can be driven four ways: the window, `ondera-cli`, `ondera-mcp` and the built-in
agent panel. All four call the same command registry (`ondera-cli commands` lists it), so an
agent's edits are ordinary undo steps, appear in the Changes log with a Revert button, and are
marked in the arrangement with the accent colour.

## Providers

Settings > Agent chooses how the agent thinks:

| Provider | Sign-in | Where prompts go |
| --- | --- | --- |
| Codex CLI | `codex login` (Settings has a button) | OpenAI, through the CLI |
| Claude Code CLI | `claude auth login` (Settings has a button) | Anthropic, through the CLI |
| Anthropic API | API key in Settings or `ANTHROPIC_API_KEY` | `api.anthropic.com` directly |
| OpenAI API | API key in Settings or `OPENAI_API_KEY` | `api.openai.com` directly |
| OpenAI-compatible | base URL and optional key | your server (Ollama, LM Studio, OpenRouter…) |

The CLI providers run in their own process group with a read-only sandbox and see only this
window's Ondera tools over the local bridge; the CLI keeps the credentials. The API providers
stream replies and tool calls directly; keys live in `settings.json` (mode 0600) and are shown
masked everywhere, including `settings.get`.

## Permissions

Settings > Agent > Permissions gates what any agent (the panel, MCP clients, `ondera-cli
--agent`) may do: file operations, transport, replacing the session, changing settings and
application control. Plain document edits are always allowed and always undoable.

## The panel

- **Conversation** shows your prompts, the streamed reply and one card per tool call. Click a
  card for its CLI form and result; Revert undoes that edit and everything after it.
- **Changes** lists every command an agent ran against this window, including MCP and CLI
  clients, with the same Revert/Redo controls.
- ⌘↵ sends; Stop cancels the task and keeps finished edits; New conversation clears context.

## From the outside

`agent.send`, `agent.stop`, `agent.status`, `agent.transcript`, `agent.providers` and
`agent.clear` drive the panel from `ondera-cli` or MCP. `ui.screenshot` returns a PNG of the
window so a model can see it; `ui.showPanel`, `view.set` and `ui.status` complete the picture.
