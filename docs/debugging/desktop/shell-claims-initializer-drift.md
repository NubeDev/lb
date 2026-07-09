# lazybones-shell: `Claims` initializer drift (E0063)

**Symptom.** Any `tauri build` of the shell (Windows cross or Linux) failed:

```
error[E0063]: missing fields `constraint` and `run_id` in initializer of `Claims`
  --> src/state.rs:28:22
```

**Cause.** `lb_auth::Claims` grew two fields (`constraint`, `run_id`) in recent
auth work, and `ui/src-tauri/src/state.rs` (the shell's S2 demo-session mint)
wasn't updated. The shell has its own lockfile/target and isn't built by
`cargo test --workspace` in `rust/`, so the drift went unnoticed until a
packaging build.

**Fix.** Initialize both to `None` in `NodeHandle::boot` — the demo member
session has no constraint and no agent run id.

**Regression guard.** `cargo test -p lazybones-shell` (headless) compiles this
path; run it whenever `lb-auth` types change. Longer term: add the shell crate
to CI's build matrix so `lb_auth` changes fail fast instead of at packaging time.
