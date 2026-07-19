# rubixd scope — rescue desk (Tauri operator app + emergency SSH lane)

Status: scope (the ask). Slice 11 of [`README.md`](README.md); parent:
[`../rubixd-rartifacts-scope.md`](../rubixd-rartifacts-scope.md). Pairs with
[`self-update-scope.md`](self-update-scope.md) — this is the surface an operator holds
when that slice's gate lands in `Failed`.

A small desktop app (Tauri) for the person standing next to the box: see a node's
status, publish/update/rollback packages, and trigger rubixd self-update — over the
normal REST API when rubixd is healthy, and over a **provisioned emergency SSH lane**
when it is not. The emergency lane is capability-first to the bone: a dedicated user
whose entire reachable surface is *recover rubixd* and *factory reset* — no shell, no
forwarding, no file transfer, nothing else.

**Owning repo**: `rubix-fleet` — `apps/rescue-desk/` (Tauri v2, TS UI + Rust core) and
a new tiny crate `crates/rubixd-rescue/` (the ForceCommand binary). sshd provisioning
lands in `crates/rubixd/` as a `rescue` CLI verb. No lb/SDK surface changes.

## Goals

- **Desk, normal mode (REST)**: node list (host + token per node, token in the OS
  keychain — never a plaintext file), status/instances views, rollback + clear-bad
  buttons, package upload via slice 8's `POST /packages` (streaming multipart from the
  app, signed-only — same wall as slice 9's browser upload), and self-update
  trigger/progress via slice 10's verbs. **No new rubixd server verbs** — the desk is
  a client of the existing surface, exactly as the embedded UI is.
- **`rubixd-rescue`** (new crate, blocking std only): a stdio line-JSON protocol binary
  with exactly five verbs — `status` (journal + ledger-summary read-only),
  `self-rollback`, `self-reinstall <version>` (re-stage from the **local blob cache
  only** — recovery must not depend on the network), `factory-reset`, `protocol-info`.
  Every other input returns a typed `denied` — the deny path *is* the product.
- **The SSH lane**: a dedicated `rubixd-rescue` system user, key-only, locked to the
  binary via sshd `Match User` drop-in: `ForceCommand rubixd-rescue`,
  `DisableForwarding yes`, `PermitTTY no`, `AllowTcpForwarding no`,
  `X11Forwarding no`, plus `restrict` on the authorized_keys line. The account has no
  interactive shell (`/usr/sbin/nologin` is not enough — ForceCommand is the wall,
  nologin-as-shell would break it; use a real shell entry but ForceCommand always
  wins) and owns nothing outside its protocol socket to the guard journal/state.
- **Provisioning is an operator verb**: `rubixd rescue enable --pubkey <key>` creates
  the user, writes the sshd drop-in + restricted authorized_keys, and validates with
  `sshd -t` before reload; `rescue disable`, `rescue rotate-key`, `rescue status`.
  Enable/disable are bearer-gated over REST too so the desk can provision the lane
  *while the node is still healthy* — the fire escape is built before the fire.
- **Factory reset** (destructive, so double-walled): wipes ledger/state, blob cache,
  desired specs, claims a fresh admin token, and removes rubixd-managed instances
  (systemd units + labelled containers it owns — the ownership labels are the wall;
  foreign units/containers and **data dirs / named volumes are never touched**, the
  slice-3/5 rule verbatim). Requires a challenge round-trip: the verb returns a nonce,
  the client must echo it signed by the same SSH session within 60 s.
- **Desk recovery mode**: when a node's REST is unreachable, the desk offers the SSH
  lane (russh or system `ssh` subprocess — decide in-session; key from the OS
  keychain/agent), renders `status`, and exposes exactly the rescue verbs with the
  journal's last words shown verbatim.

## Non-goals

- No fleet control plane — the desk manages nodes one at a time by address; rartifacts
  remains the distribution story. No mobile build (Tauri makes it possible later).
- No general remote shell, port-forward, or file browser — ever; that is the point.
- No password auth, no rescue-over-REST (when REST is up, the normal verbs exist).
- Rescue cannot install *new* versions from the network — only re-stage what the blob
  cache already holds. Getting fresh bits onto a dead box is sneakernet or reflash.

## Intent / approach

One protocol, two transports. The rescue binary speaks the same line-JSON on stdio
regardless of who spawned it, so the desk's recovery client, an `ssh` one-liner from a
laptop, and the integration tests all exercise the identical code path. Capability
enforcement lives in **sshd's config, the account, and the binary's verb table** —
three independent walls, each sufficient. Alternatives rejected: a second "emergency
HTTP server" on another port (if rubixd is dead its process is dead — the rescuer must
not live in the patient); a restricted login shell (shells accrete escapes; a verb
table cannot); giving the desk raw root SSH with a documented "please only" (not a
capability, a prayer).

## How it fits the core

Capability-first: the deny is named per verb (`denied: unknown-verb`,
`denied: factory-reset-nonce-expired`, `ForeignUnit` on reset). Workspace/data wall:
factory reset removes rubixd's own state and owned instances, never data dirs or
volumes. Secrets: the desk stores tokens/keys in the OS keychain; the rescue lane
holds no secrets at all — authorized_keys is the credential.

## Example flow

1. Day 1 (healthy): desk → `rescue enable` with the operator's pubkey; `rescue status`
   shows the lane armed. Desk uploads `app-1.4.2.pkg`, watches the instance gate green.
2. Day 40: a rubixd self-update lands `Failed` (both versions down, slice 10's
   page-a-human state). REST is dark; the desk flips to recovery mode.
3. SSH lane → `status` returns the journal: `Failed`, last gate `ui-probe-failed`,
   releases on disk `0.6.0, 0.6.1`. Operator runs `self-reinstall 0.6.0` → rescue
   re-stages from the blob cache, restarts, gate (same probe set as slice 10) green.
4. Node answers REST again; desk drops back to normal mode; the journal history shows
   the whole story.
5. Separately, a box being decommissioned: `factory-reset` + signed nonce → state
   wiped, units/containers removed, data dirs still on disk; next boot is claim-fresh.

## Testing plan

Real sshd, real systemd, no mocks:

- **Protocol crate**: verb-table unit tests; every non-verb input → typed deny.
- **SSH integration** (throwaway sshd on a high port with the generated drop-in, CI
  container): each rescue verb end-to-end over real `ssh`; then the adversarial set —
  `ssh rubixd-rescue@host bash` gets the protocol not a shell, `-L`/`-R` forwarding
  refused, `scp`/`sftp` refused, PTY refused, second pubkey not in authorized_keys
  refused.
- **Provisioning**: enable → `sshd -t` clean, lane works; disable → key auth refused;
  rotate-key → old key dead, new key live; enable is idempotent.
- **Factory reset**: owned unit + container gone, foreign unit + data dir + named
  volume untouched, token rotated, ledger empty; nonce replay/expiry refused.
- **Desk**: Rust core client tested against a live rubixd (REST) and the sshd fixture
  (recovery); UI driven by a manual runbook (`docs/testing/`) — claim, upload,
  rollback, recovery — with real output pasted, per the runbook rule.

## Risks & hard problems

- sshd config is append-order-sensitive and distro-variant; the drop-in must be
  validated (`sshd -t`) before reload and the verb must fail loudly without reloading
  on a dirty check — a broken sshd locks *everyone* out, not just the rescue user.
- Rescue runs verbs that need root (restart unit, wipe state). Recommendation: the
  rescue user gets a **single** sudoers line to the rescue binary itself
  (`NOPASSWD: /usr/bin/rubixd-rescue-exec`) with the verb table re-checked on the
  privileged side — never blanket sudo. This is the scope's hairiest review item.
- Tauri app updates are out of band (it updates itself via Tauri's updater, signed) —
  don't let the desk's own update story leak into rubixd's.
- Keychain behaviour differs per OS; degrade to encrypted-at-rest file with an
  explicit warning, never plaintext.

## Open questions

- Bundle the desk's SSH client (russh, static) vs shelling to system `ssh`?
  Recommendation: russh — no dependence on the host's ssh config surprises, and the
  tests pin one behaviour.
- Should `factory-reset` also be a slice-10 journal state so a reset that dies midway
  resumes? Recommendation: yes — reuse the journal file, same recovery discipline.

## Decisions resolved in implementation

Slice 11 was built on master 2026-07-18. The scope left several seams to the
session; each is recorded here (the slice-9 pattern).

- **SSH client — russh, confirmed.** The open question is resolved as
  recommended: the desk bundles `russh` (static, pure-Rust) rather than shelling
  to the system `ssh`. It is behind a `russh-lane` cargo feature (on by default
  for the desktop build) so the desk's core-logic tests (framing, keychain, REST
  routing, the whole factory-reset orchestration against an in-memory
  `FakeLane`) run WITHOUT the SSH stack. The transport is abstracted behind a
  `RescueLane` trait, so the protocol sequencing is tested with no network.

- **The factory-reset nonce is same-session by CONSTRUCTION, not a journal
  state.** The scope's second open question (make factory-reset a slice-10
  journal state so a mid-death reset resumes) was reconsidered and NOT taken:
  both legs of the challenge run inside ONE `rubixd-rescue` process (one SSH
  session — one stdio connection), so the nonce lives in that process's memory
  and dies with the connection. "Signed by the SAME SSH session within 60 s" is
  then enforced by the fact that a new session is a new process with no issued
  nonce — leg 2 without a matching leg 1 in the same process is always
  `factory-reset-nonce-invalid`. A journal-backed nonce would have to be
  compared against a session identifier and could be replayed from a *different*
  session; the in-memory nonce cannot. Resumability of the WIPE itself is
  handled by the wipe being idempotent (each half re-runs cleanly), not by
  journaling the nonce.

- **The factory-reset work is SPLIT by privilege, along the crate seam.** The
  `rubixd-rescue` crate is blocking-std (no tokio, no surrealDB) like
  `rubixd-guard`, so it cannot itself open the ledger or drive the backend. It
  does the plain-filesystem half of a reset (`state/`, `blobs/`, `bundles.d/`)
  directly, and shells out to TWO new HIDDEN rubixd verbs for the rest:
  `rubixd rescue-wipe` (enumerate owned instances from the ledger, tear each
  down through `backend::remove` — so the `ForeignUnit` ownership wall is the
  SAME one slices 3/5 wrote, never a second copy — and rotate the admin token)
  and `rubixd rescue-restage <version>` (re-stage from the LOCAL blob cache via
  the slice-8 `resolve_local` trust wall, never the network). This keeps the
  single most dangerous rule ("never touch what we don't own, never touch data")
  enforced in exactly one place.

- **The privileged split IS the sudoers answer.** The scope's hairiest review
  item — rescue verbs need root — is resolved with two binaries: `rubixd-rescue`
  (the unprivileged ForceCommand front) re-execs `sudo -n rubixd-rescue-exec`
  over a SINGLE sudoers line (`rubixd-rescue ALL=(root) NOPASSWD:
  /usr/bin/rubixd-rescue-exec`), and the privileged half re-checks the five-verb
  table before doing anything. The whole session runs in the one privileged
  process, which is also what keeps the factory-reset nonce same-session.

- **`rescue enable` renders the sudoers line but does not write it.** Enable
  writes the sshd drop-in + restricted authorized_keys (validate-before-reload),
  and PRINTS the single sudoers line for the operator to install with `visudo
  -c`. A bad sudoers file is its own lockout class; the desk/CLI does not drop
  it into `/etc/sudoers.d` without an explicit human step.

- **The `rescue enable/disable/rotate-key/status` REST verbs are the ONE
  documented server-verb addition** (parallel to slice 9's `GET /api/packages`).
  They are PROVISIONING verbs, not data verbs — the desk arms the lane while the
  box is still healthy, which by definition must happen over the normal REST
  surface. The rescue PROTOCOL verbs add nothing to the server; they live
  entirely on the SSH lane.

- **The newline-injection wall on public keys** is a security boundary, not a
  nicety: `validate_pubkey` refuses any control character, because a "key"
  containing a newline could smuggle a second, `restrict`-LESS authorized_keys
  line — an unrestricted backdoor. Asserted over the real REST wire, not just in
  a unit test.

- **Testing vs the environment.** The protocol + deny path are tested against
  the REAL compiled `rubixd-rescue-exec` binary over a pipe (the identical code
  path an `ssh` one-liner drives), including the `ssh … bash gets the protocol
  not a shell` / unknown-verb / malformed / bad-arity / nonce-leg-one adversarial
  set. The sshd-LEVEL walls (forwarding/scp/sftp/PTY refused, second pubkey
  refused) and the full enable→`sshd -t`→reload→real-ssh round trip need a real
  `sshd`, which the CI image lacks; they live in the manual runbook
  (`rubix-fleet/docs/testing/rescue-desk-runbook.md`) with real output pasted,
  per the runbook rule. `generated_config_passes_sshd_t` skips-and-reports where
  `sshd` is absent (the docker-suite posture). The Tauri desk itself needs a
  GTK/webkit toolchain to compile, absent here; its Rust CORE logic is
  unit-tested (keychain fallback, node parsing, REST reachability distinction,
  the whole recovery orchestration against `FakeLane`), and the UI is runbook-
  driven — the scope's "desk UI driven by a manual runbook" plan.

## Post-merge security review (2026-07-19)

An adversarial review of the shipped slice-11 code found two CRITICAL flaws in
the emergency SSH lane, plus one HIGH and four MEDIUM. All are fixed on master;
the lane was NOT provisioned on any real box before the fixes landed. Both
criticals shared one root cause: **argv was treated as trusted input on the
privileged side.**

- **CRITICAL — `rubixd rescue-wipe` was an unauthenticated local auth-bypass.**
  `hide = true` in clap is help text, not a capability: clap still parsed and
  dispatched the verb for any local caller, with no token, root, or caller
  check. Because the verb ROTATES THE ADMIN TOKEN, running it minted a fresh
  *unclaimed* token and re-opened the claim window — so any user who could
  execute the rubixd binary could rotate the desk's credential away and then
  claim the new one, converting local execute access into full admin-API
  control. **Fixed**: both hidden verbs (`rescue-wipe`, `rescue-restage`) are
  gated on `geteuid()==0` in a new `rescue::privilege` module, with an audit
  line to stderr. We deliberately did NOT add a "caller must be
  rubixd-rescue-exec" check — parent-PID inspection is racy and forgeable, and
  a check that looks like identity verification but is not is worse than an
  honest euid check.

- **CRITICAL — the sudoers line permitted arbitrary arguments, making
  `--data-root` root code execution.** A sudoers command spec with no argument
  list permits ANY argv; the rendered line had none, and the comment beside it
  claimed "the verb table is the argument wall". That was a misreading of sudo
  semantics — the verb table constrains WHICH VERB and never sees argv.
  `exec.rs` read `--data-root` off argv *before and independently of* the verb
  table, and `resolve_rubixd_bin` then executed `<data_root>/self/current/rubixd`
  AS ROOT. Planting a binary under an attacker-chosen root and passing
  `--data-root` yielded a root shell; a second, exec-free variant pointed
  `reset::wipe_filesystem`'s `remove_dir_all` at any directory on the box.
  **Fixed in three independent layers**, any one of which closes it:
  1. the sudoers grant is now `... NOPASSWD: /usr/bin/rubixd-rescue-exec ""`
     (the empty argument list — the only syntax that forbids arguments);
  2. `exec.rs` refuses ANY argv outright and derives its config on the
     privileged side (new `rubixd_rescue::config` module, so the logic is
     library-testable rather than reachable only by spawning a binary);
  3. `vet_rubixd_bin` requires the candidate to be under the trusted root,
     root-owned, and not group/other-writable before it is exec'd.

- **`SetEnv RUBIXD_DATA_ROOT` in the sshd drop-in is explicitly REJECTED.** The
  scope floated it as a way to support non-default data roots. It would punch a
  hole straight through sudo's `env_reset`, which is what stops the SSH user
  influencing the privileged side's paths. Non-default roots are instead read
  from the root-owned `/etc/rubixd/config.toml` on the trusted side. The one
  env var that remains (`RUBIXD_RESCUE_TEST_DATA_ROOT`) is honoured ONLY when
  the process is not root, so it is dead in production by construction.

- **HIGH — the factory-reset nonce was fully predictable.** It was FNV-1a over
  `(now_secs, subsec_nanos, pid, salt)`: no secret, brute-forceable offline in
  under a second, while the code compared it in constant time and the docs
  called it a guard against replay. The real wall is structural (a process that
  never issued a challenge refuses every echoed value), so impact was low — but
  a value the code calls a nonce must actually be unpredictable, or the next
  reader builds on a property that is not there. **Fixed**: 128 bits from the OS
  CSPRNG via `getrandom` (a leaf crate — the minimal-dependency posture holds).
  A CSPRNG failure now REFUSES to issue a challenge rather than falling back.

- **MEDIUM — `enable()` had a live-unvalidated-config window and an incomplete
  rollback.** The drop-in was written to its LIVE path and only removed after
  `sshd -t` failed, so a concurrent reload from any source could observe a
  broken config; and the rollback removed the drop-in but LEFT the
  authorized_keys, i.e. exactly the single-copy state `drop_in.rs` says it does
  not trust. **Fixed**: write to a `.staging` path (which the distro `*.conf`
  glob does not match), validate, then promote by atomic rename; restore the
  previous authorized_keys on every failure path; `status()` now reports
  `half_state` and `staged_leftover` so the two walls disagreeing is visible
  rather than looking like "disabled".

- **MEDIUM — `rotate_key()` could leave the lane accepting NEITHER key.** It
  delegates to `enable()`, which overwrote authorized_keys before validating
  with no backup. A rotation whose `sshd -t` or reload failed destroyed the old
  key and removed the drop-in — the worst outcome for a fire escape, reached by
  a routine rotation. **Fixed** by the snapshot/restore above; the scope's
  "no window where the lane accepts both or neither" claim is now true on the
  failure path too, not just the success path.

- **MEDIUM — `sshd -t` could pass without ever parsing our drop-in.** The check
  pointed at the main config, which only validates our fragment if that config
  `Include`s the drop-in directory. Debian-family does; RHEL-family and hardened
  configs often do not — there, validation returned 0 having never read our
  file and the reload pushed it live unvalidated. **Fixed**: a new
  `rescue::sshd_config` module asserts the include chain covers the drop-in dir
  and refuses (`RescueError::NotIncluded`, HTTP 409) when it cannot prove it,
  naming the exact `Include` line to add. Validation now runs against a probe
  config that includes the staged candidate and excludes the live copy. The docs
  no longer overstate `sshd -t`: it is syntax-only and does not evaluate `Match`
  blocks.

- **MEDIUM — execution failures were reported as `denied: malformed`.** This
  contradicted the `Effects` trait doc directly above the code (which promised a
  distinct error line) — and that promise was unimplementable, because
  `wire.rs`'s `Response` had only `Ok` and `Denied`. It was actively dangerous
  for factory-reset: leg 2 consumes the nonce and runs `rescue-wipe` (token
  ALREADY rotated), so a later failure told the operator `denied: malformed` —
  reasonably read as "refused, nothing happened" — while the box was half-wiped
  and the desk locked out. **Fixed**: a third `Response::Error { ok, error }`
  variant; `DenyReason::Malformed` now means ONLY "I could not parse your line".
  The factory-reset failure message states explicitly that the reset was
  authorised and started and the token may already be rotated.

- **LOW** — `wipe.rs` swallowed the ledger-delete error (now logged), and
  `foreign_skipped` was silently dropped by serde because the CONSUMING
  `WipeResult` in `effects.rs` omitted the field. The producer's promise that
  the ownership wall's action is "surfaced so the operator sees the wall held,
  not silence" was dead code; the field now reaches `ResetReport` and the wire,
  with tests asserting it survives both hops.

**Walls that were verified as genuinely holding** (reviewed, not changed):
`validate_pubkey`'s newline-injection refusal before any write; no path from a
denied verb to an effect; nonce single-use / consume-before-effect; and
ForeignUnit + data preservation in both backends (systemd re-reads the marker
and leaves `data_dir` intact, docker re-inspects labels with `RemoveVolumes`
false, and `wipe.rs` correctly skips the `self` row).

**File-layout**: the fixes pushed four files over the 400-line hard ceiling, so
they were split as part of the same work (`rescue::files`, `rescue::sshd_config`,
`rubixd_rescue::config`, plus sibling test files). `fetch/client.rs` — the one
remaining `make check-layout` failure, carried red through slices 6/9/11 — was
split into `fetch/client/{error,resolve,download,fetch,encode}.rs`.
`check-file-size.sh` is now GREEN for the first time.

## Related

[`self-update-scope.md`](self-update-scope.md) (the `Failed` state this rescues) ·
[`local-publish-scope.md`](local-publish-scope.md) (`POST /packages` the desk uploads
through; the blob cache rescue re-stages from) ·
[`ui-local-publish-scope.md`](ui-local-publish-scope.md) (the browser twin of the
desk's upload) · [`token-auth-scope.md`](token-auth-scope.md) (the bearer wall the
desk's normal mode lives behind).
