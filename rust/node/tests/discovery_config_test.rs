//! `LB_DISCOVERY*` → `BootConfig::discovery` (LAN-discovery seam). The env boundary is the ONE
//! place `LB_*` is read; this pins the gating that decides whether a node broadcasts on the LAN at
//! all — the highest-consequence part of the feature, since the failure mode is a node advertising
//! itself on a network where nobody asked it to.
//!
//! One `#[test]` on purpose: env is process-global, so the cases run in sequence rather than racing
//! each other across cargo's test threads (the `store_budget_config_test` precedent).

use lb_node::BootConfig;

/// Clear every var this seam reads, so each case starts from a known state.
fn clear() {
    for k in [
        "LB_DISCOVERY",
        "LB_DISCOVERY_NODE_ID",
        "LB_DISCOVERY_SERVICE_TYPE",
        "LB_DISCOVERY_FLEET",
        "LB_GATEWAY_ADDR",
    ] {
        std::env::remove_var(k);
    }
}

#[test]
fn discovery_is_off_unless_explicitly_enabled_with_a_gateway_to_advertise() {
    // ---- unset ⇒ OFF. The default posture: a node broadcasts nothing until asked.
    clear();
    assert!(
        BootConfig::from_env().discovery.is_none(),
        "an unset LB_DISCOVERY must leave the node invisible on the LAN"
    );

    // ---- anything other than `1` ⇒ OFF. No truthy-string guessing; a network broadcast is not
    // something to enable on a fuzzy match.
    clear();
    std::env::set_var("LB_GATEWAY_ADDR", "127.0.0.1:8099");
    for v in ["0", "true", "yes", ""] {
        std::env::set_var("LB_DISCOVERY", v);
        assert!(
            BootConfig::from_env().discovery.is_none(),
            "LB_DISCOVERY={v:?} must not enable a LAN broadcast — only \"1\" does"
        );
    }

    // ---- enabled WITHOUT a gateway ⇒ OFF. A headless node has no endpoint to dial, so
    // advertising one would publish a port that refuses connections.
    clear();
    std::env::set_var("LB_DISCOVERY", "1");
    assert!(
        BootConfig::from_env().discovery.is_none(),
        "a headless node must not advertise an endpoint it does not serve"
    );

    // ---- enabled WITH a gateway ⇒ ON, advertising the gateway's port.
    clear();
    std::env::set_var("LB_DISCOVERY", "1");
    std::env::set_var("LB_GATEWAY_ADDR", "127.0.0.1:8099");
    std::env::set_var("LB_DISCOVERY_NODE_ID", "node:gw-01");
    let ad = BootConfig::from_env()
        .discovery
        .expect("LB_DISCOVERY=1 with a gateway must produce an advertisement");
    assert_eq!(ad.node().as_str(), "node:gw-01");
    assert_eq!(
        ad.port, 8099,
        "the advertised port must be the gateway's, or peers discover an unreachable endpoint"
    );
    assert_eq!(
        ad.service_type.as_str(),
        "_lb._tcp",
        "lb's default service type must stay product-agnostic (rule 10)"
    );
    assert!(
        ad.fleet.is_none(),
        "no fleet tag unless the operator sets one"
    );

    // ---- a product host may set its own service type.
    clear();
    std::env::set_var("LB_DISCOVERY", "1");
    std::env::set_var("LB_GATEWAY_ADDR", "127.0.0.1:8099");
    std::env::set_var("LB_DISCOVERY_SERVICE_TYPE", "_rubix-ai._tcp");
    std::env::set_var("LB_DISCOVERY_FLEET", "floor-3");
    let ad = BootConfig::from_env().discovery.expect("still enabled");
    assert_eq!(ad.service_type.as_str(), "_rubix-ai._tcp");
    assert_eq!(ad.fleet.as_deref(), Some("floor-3"));

    // ---- a MALFORMED service type ⇒ OFF, not a panic and not a silent fallback to `_lb._tcp`.
    // Falling back would put the node on a service type the operator did not choose, where its
    // intended peers are not listening — discoverable by the wrong fleet, invisible to the right
    // one. Refusing to advertise is the safe failure.
    clear();
    std::env::set_var("LB_DISCOVERY", "1");
    std::env::set_var("LB_GATEWAY_ADDR", "127.0.0.1:8099");
    std::env::set_var("LB_DISCOVERY_SERVICE_TYPE", "not-a-valid-type");
    assert!(
        BootConfig::from_env().discovery.is_none(),
        "a malformed service type must disable discovery, never fall back to a different one"
    );

    // ---- a key-UNSAFE node id ⇒ OFF. `NodeId` refuses ids that would change a bus key's shape;
    // a wildcard id must not reach the wire from the env boundary either.
    clear();
    std::env::set_var("LB_DISCOVERY", "1");
    std::env::set_var("LB_GATEWAY_ADDR", "127.0.0.1:8099");
    std::env::set_var("LB_DISCOVERY_NODE_ID", "gw-*");
    assert!(
        BootConfig::from_env().discovery.is_none(),
        "a wildcard node id must be refused at the env boundary, not advertised"
    );

    clear();
}
