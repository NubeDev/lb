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
    advertise, browse, Advertisement, Browse, Discovered, DiscoveredPeer, ServiceType,
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
