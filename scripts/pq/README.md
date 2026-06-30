# pq — prompt & workflow CLI

A tiny, standalone CLI for **reusable prompts and workflows**. It does **not** call an
LLM — it assembles text (filling variables, running tools, chaining steps) and drops the
result on your clipboard (or stdout) so you can paste it into whatever AI you like.

Two concepts:

- **role** = a saved prompt (a Markdown file).
- **macro** = a workflow: an ordered list of steps that can run a tool (e.g. `gh`, `git`),
  render a role, and chain one step's output into the next.

> Status: **implemented CLI + seed content.** The `roles/` and `macros/` here are real,
> usable examples and double as test fixtures for the `pq` binary in `src/main.rs`.

## Layout

```
pq/
├── roles/<name>.md      # a prompt
└── macros/<name>.yaml   # a workflow
```

The library dir is resolved as: `--dir <path>` flag → `$PQ_DIR` → this folder (next to the
binary) → `~/.config/pq`. First match wins.

## Role format

YAML frontmatter + body. The body is the prompt; placeholders are filled at render time.

```md
---
desc: Conventional commit message from a diff
---
Write a Conventional Commit for the diff below.
...
{{input}}
```

Placeholders:
- `{{input}}` — the primary input (stdin, a CLI arg, or the previous macro step's output).
- `{{name}}` — any named variable passed as `name=value`.
- If a role has no `{{input}}`, the input is appended after the body.

## Macro format

```yaml
desc: One-line description
vars:                       # optional; declared variables
  - name: repo              #   required (no default) -> must be passed or prompted
  - name: idea
    rest: true              #   slurp remaining positional args into this var
    default: "..."          #   fallback if not supplied
steps:                      # run top to bottom
  - tool: git diff --staged #   run a shell command; stdout becomes {{last}}
  - role: commit            #   render a role; its {{input}} defaults to {{last}}
    input: "{{idea}}"       #   ...or set input explicitly
output: clipboard           # clipboard | stdout   (default: clipboard)
```

- `{{last}}` — output of the previous step. `{{var}}` — a declared variable.
- A `tool:` step runs a real command and captures stdout (this is the `gh`/`git` hook).
- A `role:` step renders that role with the current `{{input}}`/vars.
- `output:` decides where the final text goes.

## CLI surface

```
pq                       # fuzzy-pick a role or macro, render it
pq <role>                # render a role -> clipboard         (e.g. pq commit)
pq <role> -p             # render a role -> stdout (for piping)
git diff | pq commit     # stdin feeds {{input}}
pq run <macro> [k=v...]   # run a workflow                     (e.g. pq run triage repo=NubeDev/lb)
pq ls                    # list roles + macros with their desc
pq edit <name>           # open a role/macro in $EDITOR
```

## Suggested Rust crates (all cross-platform incl. Windows)

| Need              | Crate                                  |
|-------------------|----------------------------------------|
| CLI args          | `clap` (derive)                        |
| YAML / frontmatter| `serde` + `serde_yml` (`serde_yaml` is archived) |
| `{{...}}` render  | `minijinja`                            |
| Fuzzy picker      | `inquire` (`Select`) or `nucleo`       |
| Clipboard         | `arboard`                              |
| Run tools         | `std::process::Command` or `duct`      |

Avoid `skim` — it's Unix-only and breaks the Windows requirement.

## Examples shipped here

- roles: `commit`, `explain`, `issue`, `triage`
- macros: `commit` (git diff → commit msg), `make-issue` (idea → GitHub issue),
  `triage` (gh issue list → triage table)
