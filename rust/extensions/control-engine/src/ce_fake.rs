//! The ONE sanctioned external fake (control-engine scope §"CE test backend";
//! CLAUDE §9 / testing §0): an in-memory `ControlEngine` implementation named
//! EXACTLY `ce_fake`. CE is a C++20 engine we cannot build in Rust CI, so it — and
//! ONLY it — is stubbed behind the `rubix-ce` trait. Everything else (the gate, the
//! host, the supervisor, the bus) is exercised for real.
//!
//! It honours the S3 READ verbs (`get_tree`/`get_schema`) over a seeded graph; write
//! verbs are inert (S5). Every trait method bumps `calls` (an `AtomicUsize`) so a
//! test can prove "0 trait calls before a denied call".
//!
//! Compiled into the binary two ways: as the crate's own `#[cfg(test)]` module
//! (driving the dispatch layer + its counter) and under the `ce-fake` cargo feature
//! (so the host integration test can build a sidecar serving this stub, gated
//! per-call by `LB_CE_FAKE=1`). OFF in a shipped binary — the real path always uses
//! the real `rubix-ce` REST/WS client (CLAUDE §9: the fake never leaks into it).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rubix_ce::{
    cov_channel, ActionResult, BulkResult, BulkSpec, ComponentDto, ControlEngine, CopySpec,
    CovEvent, CovScope, CovSender, CovStream, DeletedItems, EdgeSpec, EngineInstanceId,
    ExtensionManifest, FlexValue, ManifestComponent, NewNode, NodeKey, NodeRef, Params, PropChange,
    PropPatch, RestoreResult, Result, Tree, UidKind, ValueFrame, MSG_UPDATE,
};

/// A guard that decrements the fake's active-COV-subscription counter when the pump task holding it
/// exits (which happens when the consumer drops the `CovStream` → `CovSender::send` errors). This is
/// how the arm/disarm test observes "the fake's COV subscription dropped" tied to the REAL stream
/// lifecycle, not a fake bookkeeping call.
struct ActiveGuard(Arc<AtomicUsize>);
impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// An in-memory CE stub honouring the S3 read verbs, with a per-instance call
/// counter (deny-before-call proof) and S6 COV instrumentation.
#[derive(Default)]
pub struct CeFake {
    /// Bumped by EVERY trait method — the deny test asserts this stays 0 until a
    /// call is actually dispatched.
    pub calls: AtomicUsize,
    /// Live COV subscriptions currently held (a pump has a `CovStream` open). Bumped on
    /// `subscribe_cov`, decremented when the consumer drops the stream (via `ActiveGuard`). The
    /// arm-on-first / disarm-on-last test asserts this returns to 0.
    pub active_cov: Arc<AtomicUsize>,
    /// Total `subscribe_cov` calls. A value > the peak distinct subscriptions proves a **reconnect**
    /// happened (the pump re-subscribed after a WS drop) — the reconnect test asserts on this.
    pub cov_subscribes: Arc<AtomicUsize>,
    /// The most-recent live subscriber's sender — a test injects extra COV events through it (drives
    /// the routed-watch + real-engine-ish assertions).
    pub last_sender: Arc<Mutex<Option<CovSender>>>,
    /// A "stop the current feeder" flag set by `drop_ws` to end the live subscription's stream (a
    /// simulated CE WS drop). The feeder observes it, drops its sender → the pump's `CovStream` ends →
    /// the pump re-subscribes (bumping `cov_subscribes`) — a gap, not a dead stream.
    pub drop_flag: Arc<std::sync::atomic::AtomicBool>,
    nodes: Vec<ComponentDto>,
    manifests: Vec<ExtensionManifest>,
}

impl CeFake {
    /// A fake seeded with one component (`test-math::add`) and one manifest — the
    /// minimal graph the read-verb tests assert against.
    #[must_use]
    pub fn seeded() -> Arc<Self> {
        let node = ComponentDto {
            uid: 1,
            name: "Add1".to_string(),
            r#type: "test-math::add".to_string(),
            type_id: 0,
            path: "/Add1".to_string(),
            parent: 0,
            status_flags: 0,
            properties: Default::default(),
            metadata: None,
            children: vec![],
        };
        let manifest = ExtensionManifest {
            vendor: "test".to_string(),
            name: "math".to_string(),
            version: "0.1.0".to_string(),
            components: vec![ManifestComponent {
                name: "add".to_string(),
                ..Default::default()
            }],
        };
        Arc::new(Self {
            nodes: vec![node],
            manifests: vec![manifest],
            ..Default::default()
        })
    }

    fn bump(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
    }

    /// Push one COV event to the current live subscriber (a test seam). Returns `false` if no
    /// subscriber is live or it has gone away — the injected event is dropped (fire-and-forget motion).
    pub async fn inject(&self, event: CovEvent) -> bool {
        let sender = self.last_sender.lock().unwrap().clone();
        match sender {
            Some(tx) => tx.send(Ok(event)).await.is_ok(),
            None => false,
        }
    }

    /// Simulate a CE WS drop: signal the live feeder to stop and drop its sender so the pump's
    /// `CovStream` ends. The pump re-subscribes (bumping `cov_subscribes`) — a gap, not a dead stream.
    pub fn drop_ws(&self) {
        self.drop_flag.store(true, Ordering::SeqCst);
        *self.last_sender.lock().unwrap() = None;
    }

    /// A seeded first `cov` frame every subscription emits on arm (so a watcher observes a frame without
    /// extra plumbing — the routed-watch exit gate). One property change, one clean value.
    fn seed_frame() -> CovEvent {
        CovEvent::Values(ValueFrame {
            msg_type: MSG_UPDATE,
            timestamp_ms: 1,
            changes: vec![PropChange {
                uid: 1_000_100,
                value: FlexValue::Float(4.2),
                status_flags: 0,
            }],
        })
    }

    /// An empty-changes `cov` tick the feeder sends periodically. Two jobs: it is the liveness probe
    /// (the send errors once the consumer drops the `CovStream`, ending the feeder → dropping its
    /// `ActiveGuard` → decrementing `active_cov` — the disarm signal), and it is a legitimate no-change
    /// tick a real CE also emits. Watchers key on property UIDs, so an empty tick is inert to them.
    fn heartbeat() -> CovEvent {
        CovEvent::Values(ValueFrame {
            msg_type: MSG_UPDATE,
            timestamp_ms: 0,
            changes: vec![],
        })
    }
}

#[async_trait]
impl ControlEngine for CeFake {
    async fn add_node(&self, _parent: NodeRef, _spec: NewNode) -> Result<NodeKey> {
        self.bump();
        Ok(NodeKey::new(
            EngineInstanceId::edge(),
            UidKind::Component,
            99,
        ))
    }
    async fn patch(&self, _node: &NodeKey, _props: Vec<PropPatch>) -> Result<ComponentDto> {
        self.bump();
        Ok(self.nodes[0].clone())
    }
    async fn set_layout(
        &self,
        _node: &NodeKey,
        _position: Option<(i32, i32)>,
        _new_parent: Option<u32>,
    ) -> Result<ComponentDto> {
        self.bump();
        Ok(self.nodes[0].clone())
    }
    async fn set_override(
        &self,
        _node: &NodeKey,
        _prop: &str,
        _value: FlexValue,
        _ttl: Duration,
    ) -> Result<()> {
        self.bump();
        Ok(())
    }
    async fn clear_override(&self, _node: &NodeKey, _prop: &str) -> Result<()> {
        self.bump();
        Ok(())
    }
    async fn add_edge(&self, _edge: EdgeSpec) -> Result<NodeKey> {
        self.bump();
        Ok(NodeKey::new(EngineInstanceId::edge(), UidKind::Edge, 42))
    }
    async fn remove_edge(&self, _edge: &NodeKey) -> Result<()> {
        self.bump();
        Ok(())
    }
    async fn remove_node(&self, _node: &NodeKey) -> Result<DeletedItems> {
        self.bump();
        Ok(DeletedItems::default())
    }
    async fn restore_items(&self, _items: DeletedItems) -> Result<RestoreResult> {
        self.bump();
        Ok(RestoreResult::default())
    }
    async fn copy_nodes(&self, _spec: CopySpec) -> Result<Vec<NodeKey>> {
        self.bump();
        Ok(vec![])
    }
    async fn call_action(
        &self,
        _node: &NodeKey,
        _action: &str,
        _args: Params,
    ) -> Result<ActionResult> {
        self.bump();
        Ok(ActionResult::default())
    }
    async fn bulk(&self, _batch: BulkSpec) -> Result<BulkResult> {
        self.bump();
        Ok(BulkResult::default())
    }
    async fn get_tree(&self, _root: NodeRef, _depth: i32) -> Result<Tree> {
        self.bump();
        Ok(Tree {
            nodes: self.nodes.clone(),
            edges: vec![],
        })
    }
    async fn get_schema(&self) -> Result<Vec<ExtensionManifest>> {
        self.bump();
        Ok(self.manifests.clone())
    }
    async fn subscribe_cov(&self, _scope: CovScope) -> Result<CovStream> {
        self.bump();
        self.cov_subscribes.fetch_add(1, Ordering::SeqCst);
        self.active_cov.fetch_add(1, Ordering::SeqCst);

        // A fresh subscription clears any prior drop signal (a reconnect re-arms cleanly).
        self.drop_flag.store(false, Ordering::SeqCst);
        let (tx, rx) = cov_channel(16);
        // Publish the sender so a test can inject further events through it.
        *self.last_sender.lock().unwrap() = Some(tx.clone());

        // A background feeder holds the `ActiveGuard` for this subscription's lifetime. It seeds one
        // frame, then a slow heartbeat doubles as the liveness probe: the send errors once the consumer
        // drops the `CovStream`, ending the task → dropping the guard → decrementing `active_cov` (the
        // disarm signal). `drop_flag` (set by `drop_ws`) ends the feeder to simulate a CE WS drop.
        let guard = ActiveGuard(self.active_cov.clone());
        let drop_flag = self.drop_flag.clone();
        tokio::spawn(async move {
            let _guard = guard; // decrements active_cov on task exit
            if tx.send(Ok(CeFake::seed_frame())).await.is_err() {
                return;
            }
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                if drop_flag.load(Ordering::SeqCst) {
                    return; // simulated WS drop → CovStream ends → pump reconnects
                }
                if tx.send(Ok(CeFake::heartbeat())).await.is_err() {
                    return; // consumer dropped the stream → disarm
                }
            }
        });
        Ok(rx)
    }
}
