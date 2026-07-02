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
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rubix_ce::{
    cov_channel, ActionResult, BulkResult, BulkSpec, ComponentDto, ControlEngine, CopySpec,
    CovScope, CovStream, DeletedItems, EdgeSpec, EngineInstanceId, ExtensionManifest, FlexValue,
    ManifestComponent, NewNode, NodeKey, NodeRef, Params, PropPatch, RestoreResult, Result, Tree,
    UidKind,
};

/// An in-memory CE stub honouring the S3 read verbs, with a per-instance call
/// counter (deny-before-call proof).
#[derive(Default)]
pub struct CeFake {
    /// Bumped by EVERY trait method — the deny test asserts this stays 0 until a
    /// call is actually dispatched.
    pub calls: AtomicUsize,
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
            calls: AtomicUsize::new(0),
            nodes: vec![node],
            manifests: vec![manifest],
        })
    }

    fn bump(&self) {
        self.calls.fetch_add(1, Ordering::SeqCst);
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
        // S6 wires real COV; here return an empty, immediately-closed stream.
        let (_tx, rx) = cov_channel(1);
        Ok(rx)
    }
}
