# gj — git-jobs

Scheduled, **AI-free** "commit everything and push" jobs. A job is a repo + an interval;
`gj` stores it in a YAML file and installs a `systemd --user` timer that runs the
commit+push on that interval. A clean working tree is a no-op, so a tick that finds no
changes costs nothing.

Sibling to [`pq`](../pq) (prompts/workflows): `pq` preps text for an AI; `gj` just commits.

## Build

```sh
cd scripts/gj && cargo build --release      # binary at target/release/gj
```

(This crate ships its own `.cargo/config.toml` to link via the zig toolchain, since it
lives outside `rust/` and can't inherit `rust/.cargo/config.toml`.)

## Use

```sh
gj add <repo> --every 10m [--branch B] [--id ID] [--message TPL] [--no-enable]
gj ls
gj disable <id>        # stop the timer (keeps the job)
gj enable <id>         # start it again
gj rm <id>             # delete the job + its timer units
gj run <id>            # commit+push once now (also what the timer calls)
gj install <id>        # (re)write + enable the timer for an existing job
```

Examples:

```sh
gj add . --every 10m --branch improverules-ui     # auto-commit this repo every 10 min
gj ls
gj disable lb
```

- **Duration**: `30s`, `10m`, `1h`, `2d`.
- **Message template**: `{n}` = staged file count, `{t}` = UTC timestamp. Default:
  `chore(autocommit): {n} files @ {t}`.
- **Branch**: omit to use the repo's current branch; push goes to `origin/<branch>`.
- **Jobs file**: `--file PATH` → `$GJ_FILE` → `~/.config/gj/jobs.yaml`.
- **Auth**: push uses your existing git credentials (`gh auth setup-git` token or SSH key) —
  `gj` handles no tokens.

## How scheduling works

`gj add` writes two units to `~/.config/systemd/user/`:
`gj-<id>.service` (a `oneshot` running `gj run <id>`) and `gj-<id>.timer`
(`OnBootSec`/`OnUnitActiveSec` from `--every`), then `systemctl --user enable --now` the
timer. `enable`/`disable`/`rm` drive `systemctl --user` accordingly.

Linux + systemd only. (Windows would use Task Scheduler — not built.) On a headless box
with no user systemd session, `gj add --no-enable` still records the job; install the timer
later with `gj install <id>`.

## Relation to the platform

This is the lightweight standalone path. The platform-native equivalent (a `reminder` cron
firing a `git.commit_push` MCP tool through `lb-jobs`) is scoped in
[`docs/scope/git-sync/`](../../docs/scope/git-sync/autocommit-scope.md) but not built.
