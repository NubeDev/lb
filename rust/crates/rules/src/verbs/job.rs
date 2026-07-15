//! The `job` scope handle — checkpoints, progress, and cooperative stop for long-running runs
//! (long-running-rules-scope). Pushed into EVERY run's scope so one body works in both modes:
//!
//! - **Job-backed** (`rules.run_async`): `set`/`step` persist through the [`JobSeam`] (the
//!   `lb-jobs` transcript), `progress` records beats, and `should_stop()` reflects the shared
//!   [`RunControl`]. On resume the host folds persisted checkpoints back into the handle, so a
//!   `job.step("expensive", || …)` replays as a lookup — no re-spend.
//! - **Synchronous** (`rules.run`): no seam — state is ephemeral in-memory, `should_stop()` is
//!   always false. Same script, no branching.
//!
//! Caps: checkpoints and durable progress beats are bounded (a rule cannot flood the store); the
//! caps are per-run, reset on resume (the transcript slots are upserts, so a replayed write is the
//! same slot, not growth).

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use rhai::{Dynamic, Engine, EvalAltResult, FnPtr, NativeCallContext};

use crate::control::RunControl;
use crate::grid::{dynamic_to_json, json_to_dynamic};
use crate::seam::{JobSeam, SeamError};

/// Max durable checkpoints per run (author error past this — a checkpoint is a real store write).
const MAX_CHECKPOINTS: u32 = 256;
/// Max durable progress beats per run (silently dropped past this — progress is advisory).
const MAX_PROGRESS: u32 = 1_000;

/// The `job` scope handle. Clone-cheap (Arcs) so rhai can pass it by value.
#[derive(Clone)]
pub struct JobHandle {
    /// The run's job id (empty for a synchronous run).
    id: String,
    /// Durable boundary — `None` for a synchronous run (ephemeral mode).
    seam: Option<Arc<dyn JobSeam>>,
    /// The checkpoint state: persisted values folded back in on resume + values set this run.
    state: Arc<Mutex<rhai::Map>>,
    /// The shared pause/cancel intent (a fresh default = never stopping, for sync runs).
    control: Arc<RunControl>,
    checkpoints: Arc<AtomicU32>,
    beats: Arc<AtomicU32>,
}

impl JobHandle {
    /// A job-backed handle: `state` is the persisted checkpoint map folded from the transcript.
    pub fn durable(
        id: String,
        seam: Arc<dyn JobSeam>,
        state: rhai::Map,
        control: Arc<RunControl>,
    ) -> Self {
        Self {
            id,
            seam: Some(seam),
            state: Arc::new(Mutex::new(state)),
            control,
            checkpoints: Arc::new(AtomicU32::new(0)),
            beats: Arc::new(AtomicU32::new(0)),
        }
    }

    /// An ephemeral handle for a synchronous run — same surface, in-memory only.
    pub fn ephemeral() -> Self {
        Self {
            id: String::new(),
            seam: None,
            state: Arc::new(Mutex::new(rhai::Map::new())),
            control: Arc::new(RunControl::default()),
            checkpoints: Arc::new(AtomicU32::new(0)),
            beats: Arc::new(AtomicU32::new(0)),
        }
    }

    fn lookup(&self, key: &str) -> Option<Dynamic> {
        self.state.lock().expect("job state lock").get(key).cloned()
    }

    fn persist(&self, key: &str, value: &Dynamic) -> Result<(), Box<EvalAltResult>> {
        if let Some(seam) = &self.seam {
            let n = self.checkpoints.fetch_add(1, Ordering::AcqRel);
            if n >= MAX_CHECKPOINTS {
                self.checkpoints.fetch_sub(1, Ordering::AcqRel);
                return Err(
                    format!("job checkpoint budget exceeded ({MAX_CHECKPOINTS} per run)").into(),
                );
            }
            seam.checkpoint(key, &dynamic_to_json(value))
                .map_err(seam_err)?;
        }
        self.state
            .lock()
            .expect("job state lock")
            .insert(key.into(), value.clone());
        Ok(())
    }

    fn beat(&self, pct: Option<u32>, msg: &str) -> Result<(), Box<EvalAltResult>> {
        if let Some(p) = pct {
            if p > 100 {
                return Err("job.progress pct must be 0..=100".into());
            }
        }
        if let Some(seam) = &self.seam {
            // Advisory: past the cap, beats stop being persisted but the run keeps going.
            if self.beats.fetch_add(1, Ordering::AcqRel) < MAX_PROGRESS {
                seam.progress(pct, msg).map_err(seam_err)?;
            }
        }
        Ok(())
    }
}

fn seam_err(e: SeamError) -> Box<EvalAltResult> {
    match e {
        SeamError::Denied => "denied".into(),
        SeamError::Failed(m) => m.into(),
    }
}

/// Register the `job.*` methods. The handle itself is pushed as the `job` scope var by the engine.
pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<JobHandle>("Job");

    engine.register_fn("id", |h: &mut JobHandle| h.id.clone());
    engine.register_fn("is_durable", |h: &mut JobHandle| h.seam.is_some());
    engine.register_fn("should_stop", |h: &mut JobHandle| {
        h.control.stop_requested()
    });

    engine.register_fn("get", |h: &mut JobHandle, key: &str| {
        h.lookup(key).unwrap_or(Dynamic::UNIT)
    });
    engine.register_fn("has", |h: &mut JobHandle, key: &str| {
        h.lookup(key).is_some()
    });
    engine.register_fn(
        "set",
        |h: &mut JobHandle, key: &str, value: Dynamic| -> Result<(), Box<EvalAltResult>> {
            h.persist(key, &value)
        },
    );

    // The memoized step: persisted → lookup (no re-spend on resume); fresh → run the closure,
    // persist the result, return it. The unit of resumable work.
    engine.register_fn(
        "step",
        |ctx: NativeCallContext,
         h: &mut JobHandle,
         key: &str,
         f: FnPtr|
         -> Result<Dynamic, Box<EvalAltResult>> {
            if let Some(v) = h.lookup(key) {
                return Ok(v);
            }
            let v: Dynamic = f.call_within_context(&ctx, ())?;
            // A Grid/handle can't round-trip a checkpoint; require a plain value.
            let json = dynamic_to_json(&v);
            let restored = json_to_dynamic(&json);
            h.persist(key, &restored)?;
            Ok(restored)
        },
    );

    engine.register_fn(
        "progress",
        |h: &mut JobHandle, msg: &str| -> Result<(), Box<EvalAltResult>> { h.beat(None, msg) },
    );
    engine.register_fn(
        "progress",
        |h: &mut JobHandle, pct: i64, msg: &str| -> Result<(), Box<EvalAltResult>> {
            let pct = u32::try_from(pct).map_err(|_| "job.progress pct must be 0..=100")?;
            h.beat(Some(pct), msg)
        },
    );
}

/// Catalog rows for the `job` family (data-stdlib maintenance rule: every `register_fn` has a row).
pub(crate) const CATALOG: &[crate::catalog::FnEntry] = &[
    crate::catalog::FnEntry {
        name: "job.id",
        family: "job",
        signature: "job.id() -> String",
        description: "The run's job id (empty string in a synchronous rules.run).",
    },
    crate::catalog::FnEntry {
        name: "job.is_durable",
        family: "job",
        signature: "job.is_durable() -> bool",
        description: "True when this run is job-backed (checkpoints persist; pause/resume applies).",
    },
    crate::catalog::FnEntry {
        name: "job.should_stop",
        family: "job",
        signature: "job.should_stop() -> bool",
        description: "True when a pause or cancel was requested — finish the current step and return early.",
    },
    crate::catalog::FnEntry {
        name: "job.get",
        family: "job",
        signature: "job.get(key: String) -> Dynamic",
        description: "Read a checkpoint value ((), i.e. unit, when absent).",
    },
    crate::catalog::FnEntry {
        name: "job.has",
        family: "job",
        signature: "job.has(key: String) -> bool",
        description: "Whether a checkpoint key exists.",
    },
    crate::catalog::FnEntry {
        name: "job.set",
        family: "job",
        signature: "job.set(key: String, value: Dynamic)",
        description: "Write a checkpoint (durable in a job-backed run; bounded per run).",
    },
    crate::catalog::FnEntry {
        name: "job.step",
        family: "job",
        signature: "job.step(key: String, f: || -> Dynamic) -> Dynamic",
        description: "Memoized unit of resumable work: runs f once, persists the result; a resume replays it as a lookup (no re-spend).",
    },
    crate::catalog::FnEntry {
        name: "job.progress",
        family: "job",
        signature: "job.progress(msg: String)  |  job.progress(pct: int, msg: String)",
        description: "Record a progress beat (visible in rules.runs.get; advisory, bounded per run).",
    },
];
