//! Real mDNS over the machine's actual interfaces — a live responder and a live browser, no mocks
//! (rule 4). These exercise the path a booting node takes: advertise, be found, be dialable.
//!
//! **On skipping.** These need a multicast-capable host, which some CI containers lack. The naive
//! design — "return early if nothing resolves" — makes a broken advertiser indistinguishable from
//! a restricted network, and every test passes vacuously (this was caught by planting a defect and
//! watching all three still go green). So capability is probed ONCE, up front, with a
//! known-good round trip: if that probe resolves, mDNS demonstrably works on this host and every
//! subsequent failure-to-resolve is a REAL failure that must fail the test. Only a host that
//! cannot complete the probe at all skips, and it says so loudly.

use std::time::Duration;

use lb_bus::NodeId;
use lb_discovery::{
    advertise, browse, Advertisement, Browse, Discovered, DiscoveredPeer, NodeIdentity, ServiceType,
};

/// How long to wait for a record to propagate on a working host. Generous: mDNS is chatty and a
/// loaded machine can take a few seconds.
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);

/// Every test uses its own service type so concurrent runs (and anything else on the developer's
/// LAN) cannot bleed into each other's results.
fn service_type(suffix: &str) -> ServiceType {
    ServiceType::new(format!("_lbt{suffix}._tcp")).expect("test service type must be valid")
}

/// Wait for a peer with the given node id, or `None` on timeout.
async fn await_peer(browser: &Browse, want: &NodeId, within: Duration) -> Option<DiscoveredPeer> {
    tokio::time::timeout(within, async {
        loop {
            match browser.recv().await {
                Some(Discovered::Found(p)) if &p.node == want => return Some(p),
                Some(_) => continue,
                None => return None,
            }
        }
    })
    .await
    .ok()
    .flatten()
}

/// Probe whether the HOST's mDNS stack works, using `mdns-sd` **directly** — deliberately NOT
/// through `lb_discovery`.
///
/// This independence is the whole point, and it was learned the hard way: a probe built on
/// `advertise`/`browse` shares their bugs, so planting a defect in the advertiser made the probe
/// fail too, every test skipped, and the suite stayed green while the code was broken. Going
/// straight to `mdns-sd` means this answers only "can this machine do mDNS at all", which is the
/// one question a skip may legitimately turn on.
async fn mdns_available() -> bool {
    use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

    let Ok(daemon) = ServiceDaemon::new() else {
        return false;
    };
    let ty = "_lbtprobe._tcp.local.";
    let Ok(events) = daemon.browse(ty) else {
        return false;
    };
    // The empty TXT set needs an explicit element type — `&[]` alone is ambiguous across
    // `mdns-sd`'s several `IntoTxtProperties` impls.
    let props: &[(String, String)] = &[];
    let Ok(info) = ServiceInfo::new(ty, "probe", "probe.local.", "", 1234, props) else {
        return false;
    };
    if daemon.register(info.enable_addr_auto()).is_err() {
        return false;
    }

    tokio::time::timeout(RESOLVE_TIMEOUT, async {
        loop {
            match events.recv_async().await {
                Ok(ServiceEvent::ServiceResolved(_)) => return true,
                Ok(_) => continue,
                Err(_) => return false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

/// Guard every test: skip only on a genuinely incapable host, and make that visible.
macro_rules! require_mdns {
    () => {
        if !mdns_available().await {
            eprintln!(
                "SKIPPED — this host cannot complete an mDNS round trip (no multicast interface \
                 or mDNS is filtered). These tests did NOT run."
            );
            return;
        }
    };
}

/// The headline: a node that advertises is found by a browsing peer, with the endpoint it
/// published — the whole bootstrap contract.
#[tokio::test]
async fn an_advertised_node_is_discovered_with_a_dialable_endpoint() {
    require_mdns!();

    let ty = service_type("a");
    let node = NodeId::new("node:gw-01").unwrap();

    let browser = browse(&ty).expect("mDNS proven available by the probe");
    let mut ad = Advertisement::new(node.clone(), 8099);
    ad.service_type = ty;
    ad.version = Some("0.4.5".into());
    let _held = advertise(&ad).expect("mDNS proven available by the probe");

    let peer = await_peer(&browser, &node, RESOLVE_TIMEOUT)
        .await
        .expect("an advertised node MUST be discovered on an mDNS-capable host");

    assert_eq!(peer.node, node);
    assert_eq!(peer.port, 8099);
    assert_eq!(peer.version.as_deref(), Some("0.4.5"));
    assert!(
        !peer.addresses.is_empty(),
        "a resolved peer must carry at least one address or it cannot be dialed"
    );
    let endpoint = peer.endpoint().expect("a resolved peer yields an endpoint");
    assert!(
        endpoint.ends_with(":8099"),
        "endpoint {endpoint:?} must carry the advertised port"
    );
}

/// The advertisement's product half (embedder-build-info scope), asserted on a live browse
/// round-trip: `prod` carries the EMBEDDER's build, and `ver` still carries lb's **in the same
/// record**. The second half is the point — it is what makes "add, never repurpose" true on the LAN
/// as well as over HTTP, and it fails the moment someone widens `version` to mean the product.
#[tokio::test]
async fn the_product_build_rides_prod_and_leaves_version_alone() {
    require_mdns!();

    let ty = service_type("p");
    let node = NodeId::new("node:gw-04").unwrap();

    let browser = browse(&ty).expect("mDNS proven available by the probe");
    let mut ad = Advertisement::new(node.clone(), 8099);
    ad.service_type = ty;
    ad.version = Some("0.4.5".into());
    // A fabricated embedder — no lb test names a real product (rule 10).
    ad.product_version = Some("2.4.0+gdeadbeef1234".into());
    let _held = advertise(&ad).expect("mDNS proven available by the probe");

    let peer = await_peer(&browser, &node, RESOLVE_TIMEOUT)
        .await
        .expect("an advertised node MUST be discovered on an mDNS-capable host");

    assert_eq!(peer.product_version.as_deref(), Some("2.4.0+gdeadbeef1234"));
    assert_eq!(
        peer.version.as_deref(),
        Some("0.4.5"),
        "`ver` still means lb's core — the product rides its own key, never this one"
    );
}

/// No embedder ⇒ the `prod` key is not advertised at all, so a browsing peer can tell "no product"
/// from "a product advertised as empty" (an empty string is a legal TXT value).
#[tokio::test]
async fn an_absent_product_advertises_no_prod_key() {
    require_mdns!();

    let ty = service_type("q");
    let node = NodeId::new("node:gw-05").unwrap();

    let browser = browse(&ty).expect("mDNS proven available by the probe");
    let mut ad = Advertisement::new(node.clone(), 8099);
    ad.service_type = ty;
    ad.version = Some("0.4.5".into());
    // No `product_version` — the stock lb binary's posture.
    let _held = advertise(&ad).expect("mDNS proven available by the probe");

    let peer = await_peer(&browser, &node, RESOLVE_TIMEOUT)
        .await
        .expect("an advertised node MUST be discovered on an mDNS-capable host");

    assert_eq!(peer.product_version, None);
    assert_eq!(peer.version.as_deref(), Some("0.4.5"));
}

/// The workspace wall, asserted on the wire (lib docs / rule 6): the advertisement carries
/// reachability and an opaque tag, and the type has nowhere to put a workspace, persona, roster or
/// capability. This fails if someone later adds a "convenient" TXT field that leaks tenancy onto
/// an unauthenticated LAN.
#[tokio::test]
async fn the_advertisement_carries_reachability_only_never_tenancy() {
    require_mdns!();

    let ty = service_type("b");
    let node = NodeId::new("node:gw-02").unwrap();

    let browser = browse(&ty).expect("mDNS proven available by the probe");
    let mut ad = Advertisement::new(node.clone(), 9100);
    ad.service_type = ty;
    ad.fleet = Some("floor-3".into());
    let _held = advertise(&ad).expect("mDNS proven available by the probe");

    let peer = await_peer(&browser, &node, RESOLVE_TIMEOUT)
        .await
        .expect("an advertised node MUST be discovered on an mDNS-capable host");

    assert_eq!(peer.node, node);
    assert_eq!(peer.port, 9100);
    assert_eq!(peer.fleet.as_deref(), Some("floor-3"));
}

/// Discovery must not adopt a node advertising under a different service type — the mechanism that
/// lets one LAN host several unrelated fleets.
///
/// Note this one asserts an ABSENCE, so it cannot distinguish "correctly ignored" from "mDNS
/// broken" on its own. The probe is what gives it teeth: mDNS is proven working before it runs.
#[tokio::test]
async fn a_node_on_another_service_type_is_not_discovered() {
    require_mdns!();

    let mine = service_type("c");
    let theirs = service_type("d");
    let stranger = NodeId::new("node:stranger").unwrap();
    let mine_node = NodeId::new("node:mine").unwrap();

    let browser = browse(&mine).expect("mDNS proven available by the probe");

    let mut theirs_ad = Advertisement::new(stranger.clone(), 7000);
    theirs_ad.service_type = theirs;
    let _theirs = advertise(&theirs_ad).expect("mDNS proven available by the probe");

    // A positive control on the SAME browse: this must be found, which proves the browse is live
    // and the stranger's absence below is a real exclusion rather than a dead subscriber.
    let mut mine_ad = Advertisement::new(mine_node.clone(), 7001);
    mine_ad.service_type = mine;
    let _mine = advertise(&mine_ad).expect("mDNS proven available by the probe");

    assert!(
        await_peer(&browser, &mine_node, RESOLVE_TIMEOUT)
            .await
            .is_some(),
        "the positive control must resolve, or this test proves nothing about exclusion"
    );
    assert!(
        await_peer(&browser, &stranger, Duration::from_secs(3))
            .await
            .is_none(),
        "a node advertising under a different service type must never appear in this browse"
    );
}

/// The identity trio survives the wire: an operator-set `name` and an opaque `machine_id` reach a
/// browsing peer alongside the addressable `node` id — the "available over discovery" half of the
/// node-identity contract.
///
/// Asserts the two halves that matter together: the extras arrive intact, AND the node id is
/// unchanged by them. A rename must never re-address the node, and this is where that would show
/// up if `name` ever became load-bearing on the wire.
#[tokio::test]
async fn discovery_carries_the_full_identity_trio() {
    require_mdns!();

    let ty = service_type("e");
    let node = NodeId::new("node:gw-03").unwrap();
    let identity = NodeIdentity::new(node.clone())
        .with_name("front office")
        .with_machine_id("mid-abc123");

    let browser = browse(&ty).expect("mDNS proven available by the probe");
    let mut ad = Advertisement::with_identity(identity, 8300);
    ad.service_type = ty;
    let _held = advertise(&ad).expect("mDNS proven available by the probe");

    let peer = await_peer(&browser, &node, RESOLVE_TIMEOUT)
        .await
        .expect("an advertised node MUST be discovered on an mDNS-capable host");

    assert_eq!(peer.name.as_deref(), Some("front office"));
    assert_eq!(peer.machine_id.as_deref(), Some("mid-abc123"));
    // The invariant: naming a node does not rename its address.
    assert_eq!(peer.node, node);
}

/// A node that publishes no machine id advertises no `mid` key at all — `None` on the peer, not an
/// empty string. The distinction is what lets an operator tell "this node has no machine-id source"
/// from "it published a blank one", and it is the reason the key is conditional in `advertise`.
#[tokio::test]
async fn an_absent_machine_id_is_absent_on_the_wire() {
    require_mdns!();

    let ty = service_type("f");
    let node = NodeId::new("node:gw-04").unwrap();

    let browser = browse(&ty).expect("mDNS proven available by the probe");
    // `Advertisement::new` — the minimal form, no machine id, no explicit name.
    let mut ad = Advertisement::new(node.clone(), 8400);
    ad.service_type = ty;
    let _held = advertise(&ad).expect("mDNS proven available by the probe");

    let peer = await_peer(&browser, &node, RESOLVE_TIMEOUT)
        .await
        .expect("an advertised node MUST be discovered on an mDNS-capable host");

    assert_eq!(
        peer.machine_id, None,
        "no source ⇒ no key, never an empty string"
    );
    // `name` still arrives, because it defaults to the node id rather than being empty.
    assert_eq!(peer.name.as_deref(), Some("node:gw-04"));
}
