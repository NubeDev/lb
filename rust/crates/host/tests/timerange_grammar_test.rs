//! Unit tests for `timerange/grammar.rs` — sibling test file per FILE-LAYOUT (the source file
//! sits near the 400-line ceiling, so its tests live here, over the same public API).

use lb_host::timerange::{parse, CalUnit, Endpoint, EndpointBase, RangeExpr, Unit, Window};

#[test]
fn endpoints_parse() {
    assert_eq!(
        parse("now").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            base: EndpointBase::Now { offset: None },
            snap: None
        })
    );
    assert_eq!(
        parse("now-4h").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            base: EndpointBase::Now {
                offset: Some((-4, Unit::Hour))
            },
            snap: None
        })
    );
    // m is minute, M is month (the Grafana convention the scope pins).
    assert!(matches!(
        parse("now-30m").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            base: EndpointBase::Now {
                offset: Some((-30, Unit::Minute))
            },
            ..
        })
    ));
    assert!(matches!(
        parse("now-1M").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            base: EndpointBase::Now {
                offset: Some((-1, Unit::Month))
            },
            ..
        })
    ));
    // Snap suffix, with and without an offset.
    assert!(matches!(
        parse("now-1d/d").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            snap: Some(Unit::Day),
            ..
        })
    ));
    assert!(matches!(
        parse("now/M").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            snap: Some(Unit::Month),
            ..
        })
    ));
    assert!(matches!(
        parse("2026-07-01").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            base: EndpointBase::IsoDay(_),
            ..
        })
    ));
    assert!(matches!(
        parse("2026-07-01T06:00:00Z").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            base: EndpointBase::InstantFixed(_),
            ..
        })
    ));
    assert!(matches!(
        parse("1785283200000").unwrap(),
        RangeExpr::Endpoint(Endpoint {
            base: EndpointBase::EpochMs(1_785_283_200_000),
            ..
        })
    ));
}

#[test]
fn windows_parse() {
    assert_eq!(parse("today").unwrap(), RangeExpr::Window(Window::Today));
    assert_eq!(
        parse("this-year").unwrap(),
        RangeExpr::Window(Window::This(CalUnit::Year))
    );
    assert_eq!(
        parse("last-quarter").unwrap(),
        RangeExpr::Window(Window::LastCal(CalUnit::Quarter))
    );
    assert_eq!(
        parse("next-week").unwrap(),
        RangeExpr::Window(Window::Next(CalUnit::Week))
    );
    // The two spellings are DIFFERENT tokens (the scope's headline decision).
    assert_eq!(
        parse("last-month").unwrap(),
        RangeExpr::Window(Window::LastCal(CalUnit::Month))
    );
    assert_eq!(
        parse("last-1-month").unwrap(),
        RangeExpr::Window(Window::Trailing {
            n: 1,
            unit: Unit::Month
        })
    );
    assert_eq!(
        parse("last-3-months").unwrap(),
        RangeExpr::Window(Window::Trailing {
            n: 3,
            unit: Unit::Month
        })
    );
    assert_eq!(
        parse("last-6-hours").unwrap(),
        RangeExpr::Window(Window::Trailing {
            n: 6,
            unit: Unit::Hour
        })
    );
    // The short counted form, and quarters normalizing to months.
    assert_eq!(
        parse("last-2w").unwrap(),
        RangeExpr::Window(Window::Trailing {
            n: 2,
            unit: Unit::Week
        })
    );
    assert_eq!(
        parse("last-2-quarters").unwrap(),
        RangeExpr::Window(Window::Trailing {
            n: 6,
            unit: Unit::Month
        })
    );
}

/// A refusal names the bad token AND the legal set — nothing defaults silently.
#[test]
fn refusals_name_the_token_and_the_legal_set() {
    let e = parse("last-fortnight").unwrap_err().to_string();
    assert!(e.contains("last-fortnight"), "names the token: {e}");
    assert!(e.contains("yesterday"), "names the legal set: {e}");
    let e = parse("").unwrap_err().to_string();
    assert!(e.contains("empty"), "empty is named: {e}");
    assert!(parse("now-d").is_err(), "an offset needs a count");
    assert!(parse("now-1x").is_err(), "x is not a unit");
    assert!(parse("last-0-days").is_err(), "a zero count is refused");
}
