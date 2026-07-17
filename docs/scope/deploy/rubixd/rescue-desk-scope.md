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

## Related

[`self-update-scope.md`](self-update-scope.md) (the `Failed` state this rescues) ·
[`local-publish-scope.md`](local-publish-scope.md) (`POST /packages` the desk uploads
through; the blob cache rescue re-stages from) ·
[`ui-local-publish-scope.md`](ui-local-publish-scope.md) (the browser twin of the
desk's upload) · [`token-auth-scope.md`](token-auth-scope.md) (the bearer wall the
desk's normal mode lives behind).
