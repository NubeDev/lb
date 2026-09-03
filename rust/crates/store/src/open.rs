//! Open an embedded SurrealDB. Two engines are compiled into **every** node: `Mem` (in-memory,
//! for tests/dev — [`Store::memory`]) and `SurrealKv` (persistent on-disk — [`Store::open`]).
//! Which constructor a node calls is a **config** decision at boot (`LB_STORE_PATH`), never a
//! code branch on role (symmetric nodes, rule #1). The handle type is identical for both, so
//! every read/write/list/write_tx caller is unchanged above this seam.
//!
//! The persistent engine is **SurrealKV** (pinned by the S9 store spike: pure-Rust, no C++
//! toolchain, the "builds anywhere / on a Pi" posture; durability across restart and the
//! LOAD-BEARING feature set verified — see `docs/scope/store/persistent-backend-scope.md`).
//! Log compaction (boot-time and online) lives in `compact.rs`.

use std::sync::Arc;

use surrealdb::engine::local::{Db, Mem, SurrealKv};
use surrealdb::Surreal;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::compaction_record::CompactionRecord;
use crate::scoped_response::{check_absent_table_as_empty, ScopedResponse};

/// How [`Store::open_with`] treats this machine's memory. Built from `default()` and mutated
/// through the builder methods — the struct is `#[non_exhaustive]` so a future knob stays additive
/// for every embedder.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct OpenOptions {
    /// Use this figure as the machine's available RAM instead of probing `/proc/meminfo`.
    ///
    /// For an embedder that measures its own budget (a cgroup limit is a truer ceiling than the
    /// host's `MemAvailable`), and for tests, which pin the gigabyte-scale decisions by feeding the
    /// real functions a real integer rather than seeding 617 MB.
    pub available_ram_bytes: Option<u64>,
}

impl OpenOptions {
    /// Override the measured available RAM.
    pub fn with_available_ram(mut self, bytes: Option<u64>) -> Self {
        self.available_ram_bytes = bytes;
        self
    }
}

/// `#[non_exhaustive]` so a future variant stays source-compatible: embedders match with a `_`
/// arm. (It was added when the boot memory guard introduced `WontFit`; that guard is gone — see
/// [`Store::open_with`] — but the compatibility promise it established is kept.)
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    #[error("store backend error: {0}")]
    Backend(String),
    #[error("value did not deserialize: {0}")]
    Decode(String),
    /// A first-write (`create`) hit an existing record at that id — the first write already bound
    /// (agent-run scope Part 2 first-settle). The caller treats this as "someone else decided
    /// first", not a backend failure.
    #[error("record already exists (first-write conflict)")]
    Conflict,
}

impl From<surrealdb::Error> for StoreError {
    fn from(e: surrealdb::Error) -> Self {
        StoreError::Backend(e.to_string())
    }
}

/// A handle to the embedded datastore. Cloneable; cheap to pass around the host.
///
/// **Session-namespace safety — WITHOUT serializing every operation** (store-concurrency scope).
/// The embedded SurrealDB connection carries one mutable session (its selected namespace + database).
/// Mutating it with `use_ns(ws)` is a *global* change to the shared session, distinct from the query
/// it guards, so two ops for different workspaces could interleave (`use_ns(A)` … `use_ns(B)` … A's
/// query runs against B's namespace) — the flaky-login workspace-wall race
/// (debugging/store/concurrent-use-ns-namespace-race.md). The OLD fix serialized every store op
/// behind one session mutex held across the whole query. That made a single slow scan (a reactor's
/// unbounded `SELECT`) block EVERY other store op node-wide — foreground reads stalled ~400ms behind
/// continuous background scans (debugging/store/single-mutex-serializes-all-ops.md; the dashboard-12s
/// report). The engine itself executes concurrent queries in parallel — only that mutex serialized us.
///
/// The fix: **never mutate the session.** Each op prepends `USE NS <ws> DB main;` to its own query
/// ([`scope_sql`]), so the namespace is scoped PER QUERY CALL — SurrealDB isolates it to that call's
/// execution even under concurrency (proven: cross-ns concurrent reads never contaminate). No shared
/// session state, so no mutex is needed for the wall, and ops run concurrently.
///
/// **The handle still needs an exclusive swap** for online compaction (drop → compact on disk →
/// reopen → swap back). An `RwLock` gives both: every data op takes the READ guard (shared — all
/// concurrent) and holds it across its query, so the engine can't be swapped mid-query; compaction
/// takes the WRITE guard (exclusive), which waits for in-flight ops to finish, swaps, and releases.
#[derive(Clone)]
pub struct Store {
    /// The ONE shared connection, behind an `RwLock`. Data ops hold the READ guard across their
    /// query (concurrent); compaction holds the WRITE guard to swap the handle. No `use_ns` mutation
    /// — the namespace is scoped per query via [`scope_sql`]. See the type-level note above.
    handle: Arc<RwLock<Surreal<Db>>>,
    /// The on-disk directory for a persistent store; `None` for `memory()` (which cannot
    /// compact — there is no log). Used by `compact`/`status`, never by the data verbs.
    path: Option<Arc<str>>,
    /// Outcome of the most recent compaction pass (boot or online), served by `status`.
    /// In-memory only — a restart re-seeds it from the boot pass.
    last_compaction: Arc<std::sync::Mutex<Option<CompactionRecord>>>,
    /// Per-workspace READ-ONLY handles for `store.query`, built on first use. See `reader.rs`:
    /// the caller's own SurrealQL runs in a session the engine will not let write, and which is
    /// scoped to the one workspace database, so neither guarantee depends on inspecting the SQL.
    readers: Arc<crate::reader::Readers>,
    /// The per-boot password for those reader users. In memory only, never logged.
    reader_secret: Arc<str>,
}

/// A workspace id is a slug — validate it before interpolating into `USE NS <ws>`, so a `ws` can
/// never carry SurrealQL past the namespace position (the per-query USE is the workspace wall; it
/// must be uninjectable). Accepts `[A-Za-z0-9_.-]+`, the shape every real workspace/id uses; anything
/// else is refused rather than escaped, keeping the wall a bright line.
fn scope_sql(ws: &str, sql: &str) -> Result<String, StoreError> {
    if ws.is_empty()
        || !ws
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        return Err(StoreError::Decode(format!("invalid workspace id: {ws:?}")));
    }
    // Backtick-quote the ns so a legal slug that isn't a bare SurrealQL ident still parses — e.g.
    // `ws-a` bare is read as `ws - a`. The validated charset above excludes the backtick, so the
    // quoting cannot be broken out of (the per-query USE is the workspace wall; it must be uninjectable).
    Ok(format!("USE NS `{ws}` DB main;\n{sql}"))
}

/// How long to keep retrying an open that is blocked by the previous holder's directory lock.
/// Measured release is ~150 ms (`store/tests/store_lock_probe.rs`); five seconds is ~30x that, so it
/// absorbs a loaded box without turning a genuinely-held lock into a long hang.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Open the on-disk engine, waiting out a directory lock the previous holder has not yet released.
///
/// surrealkv 0.21 takes an exclusive lock on the store directory and does NOT release it
/// synchronously when the handle drops — an immediate reopen of the same path fails with
/// "Database at <path>/store/LOCK is already locked by another process". That is a race, not a
/// permanent state: `store_lock_probe.rs` measures the release at ~150 ms, and shows that a LOCK
/// left behind by a killed process does NOT block a later open (a clean close removes the file, and
/// a forged stale one is opened straight through). So a node restart is never bricked — but a
/// restart quick enough to beat the old handle's release would fail for no good reason, and five
/// `lb-host` tests plus the boot guard hit exactly that.
///
/// Retrying is therefore right, and bounded: a lock genuinely held by a LIVE second process must
/// still surface as an error rather than hang, which is what the deadline gives us.
async fn open_engine_awaiting_lock(path: &str) -> Result<Surreal<Db>, StoreError> {
    let deadline = std::time::Instant::now() + LOCK_WAIT;
    let mut attempts = 0u32;
    loop {
        attempts += 1;
        match Surreal::new::<SurrealKv>(path).await {
            Ok(db) => {
                if attempts > 1 {
                    tracing::info!(
                        path = %path,
                        attempts,
                        "store opened after waiting out the previous holder's directory lock"
                    );
                }
                return Ok(db);
            }
            // Match on the message because surrealkv surfaces this through SurrealDB as an opaque
            // `Other` error with no typed variant to match — checked against surrealdb 3.2.4.
            Err(e)
                if e.to_string().contains("is already locked")
                    && std::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

impl Store {
    /// Open an in-memory store (tests / dev). Each call is an isolated ephemeral instance — its
    /// data is gone when the handle drops. Use [`open`](Store::open) for a node that must survive
    /// a restart.
    pub async fn memory() -> Result<Self, StoreError> {
        let db = Surreal::new::<Mem>(()).await?;
        Ok(Self {
            handle: Arc::new(RwLock::new(db)),
            path: None,
            last_compaction: Arc::new(std::sync::Mutex::new(None)),
            readers: Arc::new(crate::reader::Readers::default()),
            reader_secret: Arc::from(crate::reader::new_secret()),
        })
    }

    /// Open a **persistent** embedded store at `path` (a real node). Durable across restart:
    /// write, drop the handle, reopen at the same `path`, and the records are still there. This
    /// is the one thing `memory()` cannot do — the foundation of every must-deliver/ingest
    /// guarantee. The engine is SurrealKV; the namespace-per-workspace wall holds identically to
    /// the in-memory engine (all workspaces live in one on-disk store, scoped by `use_ns`).
    ///
    /// Opening is cheap and does **not** replay the write history. Under surrealkv 0.9 it did —
    /// the engine was an append-only log, so a long-running node paid its whole history on every
    /// boot (a 1.5 GB log ≈ 13 s), which is why a boot compaction pass and a memory guard used to
    /// stand in front of this call. surrealkv 0.21 is an LSM tree that opens from a manifest plus
    /// the WAL tail. Measured on the same machine: a 92,657-byte store reopened in 114 ms and a
    /// 42,816,755-byte one — 462x the bytes — in 143 ms. Both are gone; see [`Store::open_with`].
    pub async fn open(path: &str) -> Result<Self, StoreError> {
        Self::open_with(path, &OpenOptions::default()).await
    }

    /// Open a persistent store with explicit [`OpenOptions`].
    ///
    /// # What used to be here, and why it is not
    ///
    /// This call carried two guards (boot-memory-guard scope, issue #128): a boot compaction pass,
    /// and a refusal (`WontFit`) when the commit log was larger than available RAM. Both existed
    /// because surrealkv 0.9 REPLAYED the whole log at open to rebuild its live set, so opening a
    /// log bigger than memory took the machine down with the kernel OOM killer.
    ///
    /// surrealkv 0.21 opens from a manifest plus the WAL tail. Measured: a 92,657-byte store
    /// reopened in 114 ms; a 42,816,755-byte store — 462x the bytes — in 143 ms. Open cost is no
    /// longer a function of history, so the premise is false.
    ///
    /// Keeping the refusal would have been worse than useless. It read `log_stats`, which counts a
    /// commit-log file the LSM engine does not create, so it could only ever refuse **zero** bytes
    /// — and had anyone "fixed" that measurement to report real on-disc size, the guard would have
    /// started refusing to open perfectly healthy large stores. A safety net that can only brick a
    /// working node is not a safety net, so it was removed rather than repaired.
    pub async fn open_with(path: &str, opts: &OpenOptions) -> Result<Self, StoreError> {
        let _ = opts;
        let db = open_engine_awaiting_lock(path).await?;
        Ok(Self {
            handle: Arc::new(RwLock::new(db)),
            path: Some(Arc::from(path)),
            // Seed from the record beside the store, so `store.status` still reports the last pass
            // across a restart. (The boot compaction pass used to do this; it is gone, the record
            // is not.)
            last_compaction: Arc::new(std::sync::Mutex::new(
                crate::last_pass::load_last_compaction(path),
            )),
            readers: Arc::new(crate::reader::Readers::default()),
            reader_secret: Arc::from(crate::reader::new_secret()),
        })
    }

    /// The on-disk directory (`None` for a memory store).
    pub(crate) fn dir(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// The last-compaction slot (`compact.rs` writes it; `status.rs` reads it).
    pub(crate) fn last_compaction_slot(&self) -> &std::sync::Mutex<Option<CompactionRecord>> {
        &self.last_compaction
    }

    /// Run a workspace-scoped SurrealQL statement, returning a [`ScopedResponse`] the caller extracts
    /// with 0-based `take` over its OWN statements. The namespace is scoped to THIS query via a
    /// prepended `USE NS <ws> DB main;` ([`scope_sql`]) — the hard workspace wall (README §7), now
    /// per-query rather than via a global session mutation, so concurrent ops never race on the
    /// session and never serialize behind one another. Also the escape hatch for `RELATE`/`DEFINE`/
    /// composite-ID statements the generic verbs don't express. A raw verb run *after* `caps::check`;
    /// not an authorization point.
    ///
    /// The READ guard is held across the query so the online-compaction WRITE swap can't run
    /// mid-query (it waits for in-flight ops); concurrent readers share the guard and run in parallel.
    pub async fn query_ws(
        &self,
        ws: &str,
        sql: &str,
        bindings: Vec<(String, serde_json::Value)>,
    ) -> Result<ScopedResponse, StoreError> {
        let scoped = scope_sql(ws, sql)?;
        let guard = self.handle.read().await;
        let mut q = guard.query(scoped);
        for (k, v) in bindings {
            q = q.bind((k, v));
        }
        let resp = check_absent_table_as_empty(q.await?)?;
        // `guard` (the RwLock read lock) is still held here — dropping it now, AFTER the query has
        // executed and the response is materialized, is correct: compaction's WRITE guard could not
        // have swapped the engine while this read guard was alive.
        drop(guard);
        Ok(ScopedResponse(resp))
    }

    /// Run a workspace-scoped statement on a **read-only** session — the path `store.query` uses
    /// for SurrealQL the caller wrote.
    ///
    /// Identical in shape to [`query_ws`], with one difference that is the whole point: the session
    /// is authenticated as a `VIEWER` on the workspace's own database, so the ENGINE refuses to
    /// write and refuses to read another workspace. Neither guarantee depends on parsing the SQL,
    /// which is what SurrealDB 3 took away (`reader.rs` documents why, and what it replaced).
    ///
    /// A statement that tries to write is not an error: it comes back having changed nothing, as
    /// `Ok([])`. Callers that want to *tell* the author they sent a write add that message
    /// themselves — this method's contract is only that the write cannot land.
    ///
    /// A retryable optimistic-transaction conflict is **retried**, and needs no opt-in from the
    /// caller: SurrealDB runs even a plain `SELECT` in a transaction that a concurrent writer can
    /// abort, and re-running a statement this session cannot use to write has no side effect to
    /// repeat. That is a property of the session, not of the SQL, so it holds for whatever the
    /// caller wrote.
    ///
    /// The READ guard is held across the query for the same reason [`query_ws`] holds it.
    pub async fn query_ws_readonly(
        &self,
        ws: &str,
        sql: &str,
        bindings: Vec<(String, serde_json::Value)>,
    ) -> Result<ScopedResponse, StoreError> {
        // Validate `ws` and build the prepended USE with the SAME function the writable path uses,
        // so the wall cannot be spelled two ways and drift apart.
        let scoped = scope_sql(ws, sql)?;
        retry_conflicts(|| self.readonly_once(ws, &scoped, bindings.clone())).await
    }

    /// One attempt of [`query_ws_readonly`], on the workspace's read-only session.
    async fn readonly_once(
        &self,
        ws: &str,
        scoped: &str,
        bindings: Vec<(String, serde_json::Value)>,
    ) -> Result<ScopedResponse, StoreError> {
        let guard = self.handle.read().await;
        let reader = self
            .readers
            .get_or_build(&guard, ws, &self.reader_secret)
            .await?;
        let mut q = reader.query(scoped.to_string());
        for (k, v) in bindings {
            q = q.bind((k, v));
        }
        let resp = check_absent_table_as_empty(q.await?)?;
        drop(guard);
        Ok(ScopedResponse(resp))
    }

    /// [`query_ws`] wrapped in the crate's bounded retry-on-conflict (`conflict.rs`). Identical
    /// signature and result on success; the ONLY difference is that a SurrealDB **retryable**
    /// optimistic-transaction abort (`read or write conflict … can be retried`) is re-run — up to
    /// [`MAX_CONFLICT_RETRIES`](crate::conflict) times, with the shared jittered backoff so two
    /// collided writers desynchronize rather than re-collide — instead of surfacing. A non-retryable
    /// error returns immediately, unchanged.
    ///
    /// # When to use it
    ///
    /// **Any pure read.** SurrealDB runs a plain `SELECT` inside a transaction, so a concurrent
    /// writer can abort it — the read did not fail, it lost a race. Re-running has no side effect
    /// to repeat, so every typed read verb (`read`, `read_versioned`, `list`, `scan`, `graph`,
    /// `tables`) goes through here, as does `lb_secrets`' own `SELECT`.
    ///
    /// The gap was not theoretical. Eight concurrent `update.status` calls each read the sealed
    /// credential while one of them, holding the seal lock, wrote it; the read lost, nothing
    /// retried, and the caller reported "secret read failed" for a transient race. Measured with
    /// the retry removed and restored: `update_seam_test::concurrent_triggers_seal_exactly_one_\
    /// credential` failed **10 of 10 runs** without it and **0 of 10** with it. That test is the
    /// regression proof; a synthetic store-level contention test could not reproduce the conflict
    /// at all, so none was kept — a test that passes either way would only give false confidence.
    ///
    /// **An IDEMPOTENT mutation** — a `series`-table write running against other writers or the GC
    /// pass (ingest `commit_batch`, raw/rollup eviction). It is safe to wrap a whole `BEGIN…COMMIT`
    /// because a retried transaction is **atomic** (a conflict aborts and fully rolls back — no
    /// partial state to reconcile) and the ingest writes are **idempotent** (the commit UPSERTs keyed
    /// on `[series, producer, seq]` and deletes exactly the staged rows it read), so a retry
    /// re-applies the batch exactly once — the same exactly-once guarantee the drain already relies
    /// on.
    ///
    /// **Not** a mutation whose effect depends on how many times it runs. Retrying one of those
    /// would apply it twice; that is the caller's judgement to make, and this method cannot make it.
    pub async fn query_ws_retrying(
        &self,
        ws: &str,
        sql: &str,
        bindings: Vec<(String, serde_json::Value)>,
    ) -> Result<ScopedResponse, StoreError> {
        // `bindings` is consumed per attempt, so it is re-cloned inside the loop. Cheap next to a
        // store round-trip, and only paid on the rare retry path.
        retry_conflicts(|| self.query_ws(ws, sql, bindings.clone())).await
    }
}

/// Run `op` again while it fails with SurrealDB's **retryable** optimistic-transaction conflict,
/// up to [`MAX_CONFLICT_RETRIES`](crate::conflict) times with the crate's shared jittered backoff.
///
/// The ONE retry loop in this file, so the read path and the write path cannot drift in how many
/// times they try or how long they wait. Whether retrying is *safe* is the caller's judgement, not
/// this function's: it re-runs whatever it is given.
async fn retry_conflicts<F, Fut>(mut op: F) -> Result<ScopedResponse, StoreError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<ScopedResponse, StoreError>>,
{
    use crate::conflict::{conflict_backoff, is_retryable_conflict, MAX_CONFLICT_RETRIES};

    let mut attempt = 0;
    loop {
        match op().await {
            Ok(resp) => return Ok(resp),
            Err(e) if is_retryable_conflict(&e) && attempt < MAX_CONFLICT_RETRIES => {
                attempt += 1;
                tokio::time::sleep(conflict_backoff(attempt)).await;
            }
            Err(e) => return Err(e),
        }
    }
}
