# claude-switch

A small Rust CLI that toggles [Claude Code](https://docs.z.ai/devpack/tool/claude)
between server configurations — e.g. the official Anthropic API and the **Z.AI GLM
coding plan** (GLM-5.2 with 1M context).

It stores every provider in a YAML file and writes the active one into
`~/.claude/settings.json`, so `claude` just works against whichever backend you
last picked.

## Build

```bash
cd scripts/claude-switch
cargo build --release        # binary: ./target/release/claude-switch
```

Copy the binary onto your `PATH` (e.g. `~/.local/bin`) to use it as `claude-switch`.

## Usage

```bash
claude-switch                 # status (default): active provider + sync state
claude-switch list            # list configured providers
claude-switch use glm         # switch to GLM and write ~/.claude/settings.json
claude-switch use claude      # switch back to the official Claude API
claude-switch apply           # re-apply the current provider
claude-switch show glm        # show a provider's full env block
claude-switch add acme \      # add/replace a provider
    --base-url https://acme.test/api \
    --token sk-... \
    -e ANTHROPIC_DEFAULT_SONNET_MODEL=glm-4.7
claude-switch remove acme     # delete a provider
claude-switch edit            # open the YAML in $EDITOR
claude-switch where           # print the config file path
```

## Configuration

The YAML lives at `~/.config/claude-switch/config.yaml` (honours `$XDG_CONFIG_HOME`).
On first run it is seeded with two providers derived from the Z.AI docs:

- **glm** — `ANTHROPIC_BASE_URL=https://api.z.ai/api/anthropic`, the GLM-5.2 model
  mapping (`glm-5.2[1m]` for sonnet/opus, `glm-4.7` for haiku), the 1M-context
  compression window, and a `3000000` ms timeout.
- **claude** — the official `https://api.anthropic.com` endpoint.

> Replace the placeholder `ANTHROPIC_AUTH_TOKEN` values with your real keys
> (`claude-switch edit`, or `add` with `--token`) before running `claude`.

## Notes

- Switching **replaces** the entire `env` block of `settings.json`; all other
  top-level keys Claude Code wrote are preserved.
- Tokens are masked (`••••abcd`) in all terminal output.
