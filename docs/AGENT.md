# The Ondera agent

Ondera can be driven four ways: the window, `ondera-cli`, `ondera-mcp` and the built-in
agent panel. All four call the same command registry (`ondera-cli commands` lists it), so an
agent's edits are ordinary undo steps, appear in the Activity log with an Undo button, and are
marked in the arrangement with the accent colour.

## Connect and start a conversation

Open the Agent panel and choose **Set up agent**, or use its settings button. Both open
**Settings > Agent** directly. Choose an AI service; only the fields for that service appear.

- **Codex / Claude Code:** Ondera looks for the installed companion and checks its sign-in.
  If it is missing or outdated, use the installation help, install/update it, and choose
  **Check connection**. If it is installed but signed out, choose **Sign in**, finish in your
  browser, and return to Ondera. Existing CLI credentials stay with the provider. Fresh
  companion installation is still a separate step; Ondera does not install vendor software.
- **Anthropic / OpenAI API:** paste a key and choose **Save connection**. These APIs have
  their own billing, separate from a chat subscription. Stored keys are never prefilled in the
  input; entering a replacement or removing a key is explicit.
- **Local or compatible model:** enter the running server's address and its model name, add
  a key only if the server requires one, and choose **Save connection**. The model must support
  tool calls. Prompts go to that configured server, which may be local or remote.

**Account connected** means the CLI reports a successful sign-in. **Ready to try** means an
API key or server configuration is available; it does not claim that network access, model
availability or billing has been verified. No model request is sent by **Check connection**.
The first message exercises actual model access. A disabled local bridge is reported rather
than silently re-enabled. Changing service resets the model override to the service default;
compatible servers require their model name again. Optional model/executable overrides, standing
instructions and limits remain in **Advanced connection settings**.

Choose **Start chatting** and describe what you want in your own language. The musical starter
buttons fill the composer for you to adapt; they never send automatically. Enter sends,
Shift+Enter inserts a newline, and Cmd/Ctrl+Enter also sends. Drafts survive collapsing the panel,
failed sends preserve the message, and repeated clicks cannot submit the same pending request twice.
Unsaved connection edits survive switching Settings sections; closing asks whether to discard them.

## Providers

Settings > Agent chooses how the agent thinks:

| Provider | Sign-in | Where prompts go |
| --- | --- | --- |
| Codex CLI | `codex login` (Settings has a button) | OpenAI, through the CLI |
| Claude Code CLI | `claude auth login` (Settings has a button) | Anthropic, through the CLI |
| Anthropic API | API key in Settings or `ANTHROPIC_API_KEY` | `api.anthropic.com` directly |
| OpenAI API | API key in Settings or `OPENAI_API_KEY` | `api.openai.com` directly |
| OpenAI-compatible | base URL and optional key | your server (Ollama, LM Studio, OpenRouter…) |

The CLI providers run in their own process group and receive this window's Ondera tools.
Codex streams through app-server dynamic tools; Claude Code uses the local MCP bridge.
Personal shell tools, hooks and unrelated integrations are excluded from these music sessions. The API providers
stream replies and tool calls directly; keys live in `settings.json` (mode 0600) and are shown
masked everywhere, including `settings.get`.

## Permissions

Settings > Agent > Permissions gates what any agent (the panel, MCP clients, `ondera-cli
--agent`) may do: file operations, transport, replacing the session, changing settings and
application control. Plain document edits are always allowed and always undoable.

## The panel

- **Conversation** shows your prompts and streamed Markdown replies, with a short human-readable
  activity status. Technical command names, raw results and provider diagnostics live in **Activity**.
- **Activity** keeps the command history and **Undo from here / Redo to here** controls. Undo
  also removes later manual edits. These controls are disabled while the agent is working.
- The model picker groups the models reported by connected accounts and APIs, shows maker logos,
  supports search, and offers each model's advertised reasoning levels. Refreshing lists models
  without sending an inference request. A model's actual use still depends on provider access.
- Type `/` for commands, including `/diagnose` for any project problem, arrangement/mixing prompts,
  `/takes`, `/variation` and `/rhythm`. Commands fill a prompt or open a local feature; the prompt
  remains editable before sending.
- **Takes A/B** preserves the original before exploring a variation. Up to eight alternatives travel
  inside the saved project. Selecting a take preserves current edits, stops playback and is undoable.
- **Rhythm Lab** builds interlocking drum patterns, previews them through the system audio output,
  and creates an editable MIDI track with Drum Machine in one undo step. The preview does not
  alter the arrangement. Existing Solo states are reported rather than silently changed.
- MIDI region menus also offer humanize, velocity ramps, fit-to-scale, reverse, legato and repeat.
- Enter or Cmd/Ctrl+Enter sends; Shift+Enter adds a newline. Stop cancels the task and keeps
  finished edits. New conversation asks before clearing context and leaves the project intact.
- Errors explain the next step in plain language; raw provider messages remain in Activity.

## Appearance and connection limits

**Settings > Interface > Appearance** switches immediately between **Frutiger Aero** and the
original **Anthracite** theme; the choice is saved. Aero uses smoked glass frames, green convex
controls and silver bevels, with opaque reading/editing surfaces.

Model discovery uses Codex app-server `model/list`, Claude Code's initialized model list, or an
API's authenticated `/models` endpoint. It does not invent future model IDs or thinking modes.
If the server does not advertise thinking capabilities, the picker uses its default. One
OpenAI-compatible endpoint can be saved at a time; it can serve several vendors' models.
Codex currently needs file-backed CLI authentication (`cli_auth_credentials_store="file"`);
a keychain-only sign-in produces setup guidance. Third-party plugin control covers the parameters,
state, presets and native editor exposed by the host; it cannot guarantee every vendor plugin.

## From the outside

`agent.send`, `agent.stop`, `agent.status`, `agent.transcript`, `agent.providers` and
`agent.clear` and `agent.configure` drive the panel from `ondera-cli` or MCP. `ui.screenshot` returns a PNG of the
window so a model can see it; `ui.showPanel`, `view.set` and `ui.status` complete the picture.
