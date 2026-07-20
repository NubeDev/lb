//! The **multiplexed control connection** for one child *generation* (native-call-concurrency
//! scope). One `Conn` owns one launched [`Channel`]: a background reader task holding the read half,
//! a mutex over the write half, and a pending-reply map correlating replies to waiters by `id`.
//!
//! Why this exists: `Sidecar::request` used to write a frame and then read replies *inline*, holding
//! the sidecar's mutex across the whole round-trip. Concurrency to a native child was therefore 1,
//! node-wide, per `(ws, ext_id)` — a 13-tile dashboard ran one query thirteen times, each caller
//! billed for the whole queue (measured: 0.93 s at N=1, 12.7 s at N=13, a perfect serial staircase).
//!
//! The shape here is the standard one: **a call registers a `oneshot`, takes the write lock just long
//! enough to write ONE frame, releases it, and awaits its receiver.** The only mutually-exclusive
//! section is a frame write (microseconds), never the child's work.
//!
//! Three invariants this file exists to hold, each of which is a silent-corruption bug if broken:
//!
//! 1. **One `Conn` per channel generation, never per sidecar.** `restart`/`rearm` build a NEW `Conn`;
//!    the old one is closed and every waiter on it failed. `next_id` restarting at 0 in a fresh
//!    generation therefore cannot collide with a live waiter from the dead one — the map died with
//!    the generation. (Sharing one map across generations is the concrete misrouting mechanism the
//!    scope calls out: caller A waiting on id 3 woken by the restarted child's id 3, which belongs to
//!    caller F. Valid JSON, wrong rows, no error.)
//! 2. **The reader fails ALL outstanding waiters on exit.** A waiter whose sender is dropped without
//!    a value gets `Transport` from the `Err` arm of its receiver — never a silent forever-hang,
//!    which is strictly worse than the failure it replaces.
//! 3. **Exactly one task reads.** Two readers would steal each other's frames; the old
//!    `if reply.id != id { continue }` discard becomes routing, not filtering.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::{oneshot, Mutex as AsyncMutex};
use tokio::task::JoinHandle;

use crate::error::SupervisorError;
use crate::frame::{read_frame, write_frame};
use crate::launcher::{Channel, ChildWrite, Kill};
use crate::rpc::{CallParams, Caller, Method, Reply, Request};

/// The pending-reply map: `id` → the waiter that asked. Held behind a std `Mutex` (never across an
/// await — only insert/remove/drain), and **per-`Conn`**, so it is per-generation AND per-sidecar.
/// Never node-global: an id collision therefore cannot cross a workspace (the ws wall stays
/// structural at `SidecarMap`'s key, and nothing below it shares state).
type Pending = Arc<Mutex<Option<HashMap<u64, oneshot::Sender<Reply>>>>>;

/// One live, multiplexed control line to a child generation.
pub struct Conn {
    write: AsyncMutex<ChildWrite>,
    pending: Pending,
    next_id: AtomicU64,
    reader: JoinHandle<()>,
    kill: Mutex<Option<Box<dyn Kill>>>,
}

impl Conn {
    /// Take ownership of `channel` and start the reader task. The caller must have already completed
    /// any raw handshake it needs (see `Sidecar::spawn`: `init` runs synchronously on the raw channel
    /// *before* this, so the bootstrap has no reader to race).
    pub(crate) fn start(channel: Channel) -> Self {
        let Channel { write, read, kill } = channel;
        let pending: Pending = Arc::new(Mutex::new(Some(HashMap::new())));
        let reader = tokio::spawn(read_loop(read, pending.clone()));
        Self {
            write: AsyncMutex::new(write),
            pending,
            next_id: AtomicU64::new(0),
            reader,
            kill: Mutex::new(Some(kill)),
        }
    }

    /// Send `method`/`params` and await this caller's own reply.
    ///
    /// The lock discipline is the whole point: register the waiter FIRST (so a reply cannot arrive
    /// before there is somewhere to route it), then hold the write lock for exactly one frame, then
    /// await unlocked.
    pub async fn request(
        &self,
        method: Method,
        params: String,
    ) -> Result<String, SupervisorError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        // Register before writing. The reverse order races: the child can reply before we insert.
        {
            let mut guard = self.pending.lock().unwrap();
            match guard.as_mut() {
                Some(map) => {
                    map.insert(id, tx);
                }
                // The reader already exited and drained — this generation is dead.
                None => return Err(SupervisorError::Transport("sidecar is not running".into())),
            }
        }

        let req = Request { id, method, params };
        let bytes = match serde_json::to_vec(&req) {
            Ok(b) => b,
            Err(e) => {
                self.forget(id);
                return Err(SupervisorError::Transport(e.to_string()));
            }
        };

        // The ONLY mutually-exclusive section: one frame write. Not the round-trip.
        {
            let mut w = self.write.lock().await;
            if let Err(e) = write_frame(&mut *w, &bytes).await {
                drop(w);
                self.forget(id);
                return Err(e);
            }
        }

        // Unlocked. A dropped sender (reader exited) surfaces as a transport error, never a hang.
        match rx.await {
            Ok(reply) => match reply.error {
                Some(err) => Err(SupervisorError::Child(err)),
                None => Ok(reply.result.unwrap_or_default()),
            },
            Err(_) => Err(SupervisorError::Transport(
                "child connection closed before reply".into(),
            )),
        }
    }

    /// Dispatch a tool call over this generation, stamping `caller` into the frame.
    ///
    /// The detached-handle counterpart of [`Sidecar::call_with_caller`](crate::Sidecar::call_with_caller):
    /// the host clones an `Arc<Conn>` out from under its per-sidecar mutex, drops the guard, and then
    /// calls this **unlocked** — which is what actually removes the serialization. Frame construction
    /// lives here rather than in the host so both paths build byte-identical frames.
    pub async fn call_with_caller(
        &self,
        tool: &str,
        input: &str,
        caller: Option<Caller>,
    ) -> Result<String, SupervisorError> {
        let params = serde_json::to_string(&CallParams {
            tool: tool.to_string(),
            input: input.to_string(),
            caller,
        })
        .map_err(|e| SupervisorError::Transport(e.to_string()))?;
        self.request(Method::Call, params).await
    }

    /// Drop a waiter that never got as far as the wire (serialize/write failed), so a failed call
    /// cannot leak an entry that would later be woken by an unrelated reply.
    fn forget(&self, id: u64) {
        if let Some(map) = self.pending.lock().unwrap().as_mut() {
            map.remove(&id);
        }
    }

    /// Close this generation: stop the reader, fail every remaining waiter, and kill the child.
    ///
    /// Ordering matters. The map is sealed (`None`) FIRST, so a call racing this transition is
    /// refused outright rather than registering a waiter nobody will ever wake. Then the reader is
    /// aborted, then the child is killed and its exit awaited — a respawn must not race a living
    /// predecessor.
    pub(crate) async fn close(&self) {
        self.seal();
        self.reader.abort();
        let kill = self.kill.lock().unwrap().take();
        if let Some(kill) = kill {
            kill.kill().await;
        }
    }

    /// Seal the pending map and drop every waiter's sender — each one's `rx.await` resolves `Err`,
    /// i.e. a transport error. This is invariant 2: no orphaned waiter, ever.
    ///
    /// The two statements are deliberate and must stay two. The `lock()` temporary is released at the
    /// end of the FIRST statement, so the senders (and the waker each one fires) drop on the second
    /// with **no lock held**. Collapsing this into `drop(self.pending.lock().unwrap().take())` holds
    /// the `std::sync::Mutex` across every wake — a self-deadlock the moment anything a waiter runs
    /// on wake touches `pending` again.
    fn seal(&self) {
        let drained = self.pending.lock().unwrap().take();
        drop(drained); // dropping the senders wakes every waiter with Err — unlocked, see above
    }
}

impl Drop for Conn {
    fn drop(&mut self) {
        // A `Conn` dropped without `close()` (an error path, or the whole sidecar going away) must
        // still not leak its reader task or strand its waiters.
        self.reader.abort();
        let drained = self.pending.lock().unwrap().take();
        drop(drained);
    }
}

/// The reader task: own the read half, route every reply to its waiter by `id`, and on ANY exit
/// (EOF, decode error, child death) fail all outstanding waiters.
///
/// A reply whose `id` has no waiter is dropped — that is a late reply for a call that already
/// failed, not another caller's data. It is the ONLY discard left, and it discards nothing a caller
/// is still waiting for.
async fn read_loop(mut read: crate::launcher::ChildRead, pending: Pending) {
    loop {
        let body = match read_frame(&mut read).await {
            Ok(b) => b,
            Err(_) => break, // child closed / broke the line
        };
        let reply: Reply = match serde_json::from_slice(&body) {
            Ok(r) => r,
            Err(_) => break, // a desynced stream cannot be trusted to re-frame
        };
        let waiter = pending
            .lock()
            .unwrap()
            .as_mut()
            .and_then(|map| map.remove(&reply.id));
        if let Some(tx) = waiter {
            let _ = tx.send(reply); // receiver gone = caller abandoned; fine
        }
    }
    // Invariant 2: seal + drop every sender so no waiter hangs forever.
    let drained = pending.lock().unwrap().take();
    drop(drained);
}
