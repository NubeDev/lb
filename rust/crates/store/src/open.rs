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

use serde::de::DeserializeOwned;
use surrealdb::engine::local::{Db, Mem, SurrealKv};
use surrealdb::Surreal;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::boot_guard::open_would_not_fit;
use crate::boot_pass::boot_compact;
use crate::compaction_record::CompactionRecord;

/// How [`Store::open_with`] treats this machine's memory. Built from `default()` and mutated
/// through the builder methods — the struct is `#[non_exhaustive]` so a future knob stays additive
/// for every embedder.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct OpenOptions {
    /// Disable the **open guard** only (never the compaction preconditions — skipping a pass is
    /// always safe, so there is nothing to force). Filled at the binary boundary from
    /// `LB_STORE_OPEN_UNGUARDED=1`; the store crate reads no env itself.
    pub unguarded: bool,
    /// Use this figure as the machine's available RAM instead of probing `/proc/meminfo`.
    ///
    /// For an embedder that measures its own budget (a cgroup limit is a truer ceiling than the
    /// host's `MemAvailable`), and for tests, which pin the gigabyte-scale decisions by feeding the
    /// real functions a real integer rather than seeding 617 MB.
    pub available_ram_bytes: Option<u64>,
}

impl OpenOptions {
    /// Turn the open guard off (`LB_STORE_OPEN_UNGUARDED=1`).
    pub fn allow_unguarded(mut self, yes: bool) -> Self {
        self.unguarded = yes;
        self
    }

    /// Override the measured available RAM.
    pub fn with_available_ram(mut self, bytes: Option<u64>) -> Self {
        self.available_ram_bytes = bytes;
        self
    }
}

/// `#[non_exhaustive]` since the boot memory guard (issue #128) added [`StoreError::WontFit`]:
/// embedders match with a `_` arm, and a future variant stays source-compatible.
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
    /// The commit log is too large for this machine's memory to replay: opening would build the
    /// whole live-set index in RAM and, on the incident that motivated this guard, take the
    /// **machine** down with the kernel's global OOM killer rather than just the node
    /// (boot-memory-guard scope, issue #128). Refused before a byte is allocated.
    ///
    /// The message names both numbers and every remedy, because it is the entire diagnostic an
    /// operator gets from `journalctl` on a box they may only just have got back.
    #[error(
        "store at {path} will not fit in memory: the commit log is {log_bytes} bytes and only \
         {available_ram} bytes of RAM are available, so replaying it would likely OOM this \
         machine. Refusing to open (this is a heuristic guard). Remedies: add RAM or swap; \
         compact the store on a larger machine; lower retention so the next compaction shrinks \
         the live set; or, if you know it fits, set LB_STORE_OPEN_UNGUARDED=1 to force the open."
    )]
    WontFit {
        path: String,
        log_bytes: u64,
        available_ram: u64,
    },
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

/// A statement selector for [`ScopedResponse::take`], shifted by one to skip the injected `USE` at
/// real index 0. Mirrors the selectors SurrealDB's `Response::take` accepts — a statement index
/// (`usize`), a field of the first statement (`&str`), or a field of statement N (`(usize, &str)`) —
/// so every existing caller idiom works verbatim while the caller's index 0 maps to real index 1.
pub trait ScopedIndex {
    /// The real (USE-inclusive) selector this caller-facing one maps to.
    type Shifted;
    fn shift(self) -> Self::Shifted;
}
impl ScopedIndex for usize {
    type Shifted = usize;
    fn shift(self) -> usize {
        self + 1
    }
}
impl<'a> ScopedIndex for &'a str {
    type Shifted = (usize, &'a str);
    fn shift(self) -> (usize, &'a str) {
        // `take("field")` means "field of the caller's FIRST statement" — real statement 1.
        (1, self)
    }
}
impl<'a> ScopedIndex for (usize, &'a str) {
    type Shifted = (usize, &'a str);
    fn shift(self) -> (usize, &'a str) {
        (self.0 + 1, self.1)
    }
}

/// The result of a scoped store query. Wraps SurrealDB's `Response` and hides the leading `USE`
/// statement's result slot: `take(0)` returns the caller's FIRST statement (the USE lives at the
/// real index 0), so every one of the ~140 `query_ws` callers keeps its existing selectors.
pub struct ScopedResponse(surrealdb::Response);

impl ScopedResponse {
    /// Extract a result selected 0-based over the caller's OWN statements (the injected `USE` at real
    /// index 0 is invisible here). Accepts the same selectors as `Response::take`, each shifted past
    /// the USE by [`ScopedIndex`].
    // The selector is an `impl ScopedIndex` ARGUMENT (a hidden generic), so `R` is the only turbofish
    // param — `take::<Vec<Foo>>(0)` binds the result type exactly as `Response::take::<Vec<Foo>>(0)`
    // does. The associated-type bound threads the shifted selector into SurrealDB's `QueryResult`.
    // `surrealdb::Error` is ~144 bytes and is NOT ours to box: it is the type every one of the ~140
    // `query_ws` callers already matches on, so wrapping it here would be an API break across the
    // workspace to move bytes we do not own.
    #[allow(clippy::result_large_err)]
    pub fn take<R: DeserializeOwned>(
        &mut self,
        index: impl ScopedIndex<Shifted: surrealdb::opt::QueryResult<R>>,
    ) -> Result<R, surrealdb::Error> {
        self.0.take(index.shift())
    }

    /// Surface any statement error. `query_ws` already `check`s internally, so this is a no-op that
    /// preserves the `…await?.check()?` caller idiom.
    #[allow(clippy::result_large_err)] // see `take` above — the error type is surrealdb's.
    pub fn check(self) -> Result<Self, surrealdb::Error> {
        Ok(self)
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
        })
    }

    /// Open a **persistent** embedded store at `path` (a real node). Durable across restart:
    /// write, drop the handle, reopen at the same `path`, and the records are still there. This
    /// is the one thing `memory()` cannot do — the foundation of every must-deliver/ingest
    /// guarantee. The engine is SurrealKV; the namespace-per-workspace wall holds identically to
    /// the in-memory engine (all workspaces live in one on-disk store, scoped by `use_ns`).
    ///
    /// The commit log is compacted first (see [`compact_log`]) — SurrealKV is append-only and
    /// replays every byte of the log at open, so a long-running node otherwise pays its whole
    /// write history on every boot (measured: a 1.5 GB log ≈ 13 s to open, live set ~2% of it).
    /// The boot pass and the open guard both apply — see [`Store::open_with`], of which this is
    /// the default-options form (the guard on, the machine measured).
    pub async fn open(path: &str) -> Result<Self, StoreError> {
        Self::open_with(path, &OpenOptions::default()).await
    }

    /// Open a persistent store with the boot memory guards configured (boot-memory-guard scope,
    /// issue #128). Three things happen, in this order:
    ///
    /// 1. Any pending `.merge/` is completed — **always**, before any decision (the P0 in
    ///    `compact.rs`); skipping compaction must never mean skipping merge completion.
    /// 2. The boot compaction pass runs **only if the machine can afford it and it is expected to
    ///    pay** ([`crate::boot_compaction_skip`]). A skip is logged at warn with every number and
    ///    surfaces in `store.status` as `last_compaction.skipped`.
    /// 3. The open itself is **refused** with [`StoreError::WontFit`] when the (possibly
    ///    uncompacted) log is larger than available RAM — unless `opts.unguarded`. `lb-node` turns
    ///    that into a clean nonzero exit and never falls back to `mem://`: a silently-empty node
    ///    serving a workspace that "lost" its data is strictly worse than a down node with a
    ///    legible reason (scope decision 3).
    ///
    /// Both guards **fail open** on a machine whose memory cannot be measured (`/proc/meminfo`
    /// absent or unreadable): today's behaviour, byte for byte.
    pub async fn open_with(path: &str, opts: &OpenOptions) -> Result<Self, StoreError> {
        let available_ram = opts
            .available_ram_bytes
            .or_else(crate::meminfo::available_ram_bytes);
        let owned = path.to_string();
        // The pass is synchronous file I/O over the whole log — keep it off the async workers.
        // Best-effort by design: a failed compaction only means a slower boot.
        //
        // The caller's `tracing` dispatcher is carried ONTO the blocking thread: a subscriber is
        // thread-local unless it was installed globally, and the guard's whole contract is that its
        // decision is loud. A warn line emitted on a pool thread that no subscriber is listening to
        // is a silent skip — the exact failure mode this scope exists to remove.
        let dispatch = tracing::dispatcher::get_default(|d| d.clone());
        let boot_pass = tokio::task::spawn_blocking(move || {
            tracing::dispatcher::with_default(&dispatch, || boot_compact(&owned, available_ram))
        })
        .await
        .ok();

        // Re-stat AFTER the pass: a productive pass is exactly what can bring a log back under the
        // guard, and refusing on the pre-pass number would refuse a store that now fits.
        let (log_bytes, _) = crate::status::log_stats(path);
        if open_would_not_fit(log_bytes, available_ram) {
            let available_ram = available_ram.unwrap_or(0);
            if opts.unguarded {
                tracing::warn!(
                    path = %path,
                    log_bytes,
                    available_ram,
                    "store: the commit log is larger than available RAM, but the open guard is \
                     DISABLED (LB_STORE_OPEN_UNGUARDED=1) — attempting the open anyway; if this \
                     machine OOMs, that is why"
                );
            } else {
                let err = StoreError::WontFit {
                    path: path.to_string(),
                    log_bytes,
                    available_ram,
                };
                tracing::error!(path = %path, log_bytes, available_ram, "{err}");
                return Err(err);
            }
        }

        let db = Surreal::new::<SurrealKv>(path).await?;
        Ok(Self {
            handle: Arc::new(RwLock::new(db)),
            path: Some(Arc::from(path)),
            last_compaction: Arc::new(std::sync::Mutex::new(boot_pass)),
        })
    }

    /// The handle cell, for the online compaction pass only (`compact.rs`). Compaction takes the
    /// WRITE guard to swap the engine; every data op takes the READ guard, so the swap waits for
    /// in-flight ops and no query ever runs against a half-open engine.
    pub(crate) fn session_cell(&self) -> Arc<RwLock<Surreal<Db>>> {
        Arc::clone(&self.handle)
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
        let resp = q.await?.check()?;
        // `guard` (the RwLock read lock) is still held here — dropping it now, AFTER the query has
        // executed and the response is materialized, is correct: compaction's WRITE guard could not
        // have swapped the engine while this read guard was alive.
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
    /// Use this for a `series`-table MUTATION that runs concurrently with other writers or the GC
    /// pass (ingest `commit_batch`, raw/rollup eviction). It is safe to wrap a whole `BEGIN…COMMIT`
    /// here because a retried transaction is **atomic** (a conflict aborts and fully rolls back — no
    /// partial state to reconcile) and the ingest writes are **idempotent** (the commit UPSERTs keyed
    /// on `[series, producer, seq]` and deletes exactly the staged rows it read), so a retry
    /// re-applies the batch exactly once — the same exactly-once guarantee the drain already relies
    /// on. A plain read (e.g. the drain `SELECT`) does not need this.
    pub async fn query_ws_retrying(
        &self,
        ws: &str,
        sql: &str,
        bindings: Vec<(String, serde_json::Value)>,
    ) -> Result<ScopedResponse, StoreError> {
        use crate::conflict::{conflict_backoff, is_retryable_conflict, MAX_CONFLICT_RETRIES};

        let mut attempt = 0;
        loop {
            // `bindings` is consumed by `query_ws`, so re-clone per attempt. Cheap next to a store
            // round-trip, and only paid on the rare retry path.
            match self.query_ws(ws, sql, bindings.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) if is_retryable_conflict(&e) && attempt < MAX_CONFLICT_RETRIES => {
                    attempt += 1;
                    tokio::time::sleep(conflict_backoff(attempt)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
