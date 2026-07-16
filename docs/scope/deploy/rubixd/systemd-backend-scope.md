# rubixd scope — systemd backend

Status: scope (the ask). Slice 3 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md).

The first real `Backend` implementation: install/update/remove a `systemd`-kind package
as versioned release dirs + a per-instance `current` symlink, a generated unit + env
file, driven through the `service-manager` crate.

## Goals

- `backend/systemd/` as a folder of verbs: `install.rs`, `update.rs` (stage + swap
  only — the *transaction* around it is slice 4), `remove.rs`, `status.rs`,
  `layout.rs` (path derivation), `unit.rs` (unit + env-file rendering).
- The filesystem contract (from the parent scope):
  - `/opt/rubix/<pkg>/releases/<version>/<bin>` — immutable, verified, shared across
    instances; written atomically (temp file, `chmod 0755`, rename — the lb
    `install_dir.rs` pattern, survives `ETXTBSY`).
  - `/opt/rubix/<pkg>/instances/<instance>/current` → release dir symlink; swap =
    `symlink_at` new + atomic rename.
  - `/etc/rubix/<instance>.env` rendered from bundle config (0640).
  - `/etc/systemd/system/rubix-<pkg>-<instance>.service` rendered from one template:
    `ExecStart=<current>/<bin>`, `EnvironmentFile=`, `Restart=on-failure`,
    `WantedBy=multi-user.target`.
  - `/var/lib/rubix/<instance>/` — created on install, **never written, never removed**
    (remove leaves it with a `.orphaned` marker; purging data is a human's job).
- `service-manager` for install/enable/start/stop/uninstall/status; daemon-reload after
  unit writes. **User-scope support** (`--user` level) is first-class because CI tests
  run there.
- **Ownership wall**: every unit rubixd manages carries `X-Rubixd=…` marker lines
  (package/instance/bundle) in the unit file; every destructive verb re-reads the unit
  and refuses (`ForeignUnit` error) if the marker is absent or mismatched.
- Arch guard: before staging, verify the ELF header matches the machine arch (the ems
  ARM scope's requirement).

## Non-goals

- No health gating, no rollback, no bad-version marks — slice 4 wraps these verbs in
  the transaction. No download/verify (the artifact arrives via slice 6's fetcher; this
  slice takes a local verified blob path as input). No Windows service backend.

## Intent / approach

`Backend` verbs stay dumb and total: each does one filesystem/systemd effect and
returns a typed result; policy (when to call, what order, what to do on failure) lives
in `reconcile/` + slice 4. Rendering is plain string templates in `unit.rs` — no
templating engine for two files. Alternative rejected: `systemd-run`/D-Bus direct —
`service-manager` already abstracts init systems and gives us the future Windows door.

## How it fits the core

One responsibility per file (six verb files, no `utils.rs`). Ownership wall = the
isolation rule translated. Everything lb-specific N/A.

## Example flow

1. `install(pkg=rubix-ai@0.4.5, instance=rubix-main, blob=/var/cache/rubixd/…)` →
   release dir staged, env + unit rendered, daemon-reload, enable + start → `Active`.
2. `update(…, 0.4.6)` → stage `releases/0.4.6/`, stop, flip symlink, start (caller
   checks health — slice 4).
3. `remove(rubix-main)` → stop, disable, delete unit + env + instance dir symlink;
   `/var/lib/rubix/rubix-main/` left with `.orphaned`.

## Testing plan

Real user-scope systemd (`systemd --user`) in CI — no mocks:

- install → unit active; binary runs from `current`; env file consumed (test binary
  echoes an env var to a file in its data dir).
- update → symlink points at new release, service active, old release dir retained.
- remove → unit gone, data dir intact with marker.
- **Ownership deny**: a hand-seeded `rubix-fake-x.service` without markers → every
  destructive verb refuses with `ForeignUnit`.
- Arch guard: an ELF for the wrong arch is refused before any systemd call.
- Atomicity: re-install over a *running* instance does not `ETXTBSY` (temp+rename
  asserted by running the old binary during stage).

## Risks & hard problems

- Root vs user scope path duality (`/opt`, `/etc/systemd/system` vs `~/.local/…`,
  `~/.config/systemd/user`) — isolate in `layout.rs` behind one `Scope` enum; CI runs
  user, production runs system, same verbs.
- `service-manager` API coverage (enable-at-boot, daemon-reload) — verify early in the
  session; fall back to `systemctl` shell-out **only** behind the same verb seam if a
  gap is real, and record it in debugging/.

## Open questions

- Keep N previous releases: N=2 default — confirm (disk-constrained edge boxes may
  want 1).

## Related

[`rollback-health-scope.md`](rollback-health-scope.md) (wraps these verbs) · lb
`rust/crates/host/src/ext/install_dir.rs` (the atomic-write pattern) ·
`ems/docs/scope/platform-targets/arm-raspberry-pi-build-scope.md` (arch guard, systemd
unit precedent).
