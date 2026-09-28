# Contributing to Ondera

Thank you for helping. Ondera is a digital audio workstation with a Rust engine and a React
window; [DEVELOPMENT.md](docs/DEVELOPMENT.md) explains the layout, the build and the checks.

## Reporting a problem

Open an [issue](https://github.com/ludovic111/ondera/issues) with your Ondera version (Help menu),
your system, what you did, what you expected and what happened. A song file or a screenshot
helps. Crashes while loading a plugin: say which plugin, its format (CLAP, VST3, AU) and version.

## Sending a change

1. Open an issue first for anything larger than a fix, so we can agree on the approach.
2. Work on a branch and keep the change focused.
3. Follow the rules in [DEVELOPMENT.md](docs/DEVELOPMENT.md): a user-facing action goes into the
   command registry so the window, the CLI, MCP and the agent get it together; nothing allocates,
   blocks or logs in the audio callback; visual values live in `frontend/src/theme` only.
4. Run the checks listed in the pull request template before you open the pull request.

## Contributor License Agreement

Pull requests are accepted once you agree to the [Contributor License Agreement](CLA.md) by
ticking its box in the pull request template. You keep the copyright in your work; the agreement
lets the project keep being maintained, relicensed or passed on without finding every contributor
again, and promises that every MIT release that includes your work stays MIT.
