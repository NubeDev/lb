//! Clearing an axis back to "inherit" — the other half of the write vocabulary (`PrefsAxis`).
//!
//! Before this existed a patch could only ever SET an axis, so a member who had once stored a
//! `ui_theme` shadowed the workspace default forever: the whole-fold axes are all-or-nothing, and
//! there was no representable value meaning "unset". These run against a REAL `mem://` store, so
//! they also pin the SurrealDB behaviour the design rests on — `UPSERT ... MERGE` with an explicit
//! null DOES drop a column.

use lb_prefs::{
    get_user_prefs, get_workspace_prefs, resolve_chain, set_user_prefs, set_workspace_prefs,
    DateStyle, Prefs, PrefsAxis, UnitSystem,
};
use lb_store::Store;
use serde_json::json;

/// The regression this whole change exists for: a member with their own theme stops inheriting the
/// workspace default, and clearing the axis restores inheritance.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn clearing_ui_theme_restores_the_workspace_default() {
    let store = Store::memory().await.unwrap();

    set_workspace_prefs(
        &store,
        "nube",
        &Prefs {
            ui_theme: Some(json!({ "preset": "corporate" })),
            ..Prefs::default()
        },
        &[],
    )
    .await
    .unwrap();

    // The member picks their own theme — now shadowing the workspace default entirely.
    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs {
            ui_theme: Some(json!({ "preset": "neon" })),
            ..Prefs::default()
        },
        &[],
    )
    .await
    .unwrap();
    let shadowed = resolve_chain(&store, "nube", "user:test", None)
        .await
        .unwrap();
    assert_eq!(
        shadowed.ui_theme,
        Some(json!({ "preset": "neon" })),
        "the member's own theme wins while it is set"
    );

    // Clear it: the member inherits the workspace default again.
    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs::default(),
        &[PrefsAxis::UiTheme],
    )
    .await
    .unwrap();

    let resolved = resolve_chain(&store, "nube", "user:test", None)
        .await
        .unwrap();
    assert_eq!(
        resolved.ui_theme,
        Some(json!({ "preset": "corporate" })),
        "clearing the member axis falls back through to the workspace default"
    );
    let own = get_user_prefs(&store, "nube", "user:test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        own.ui_theme, None,
        "the stored member axis is genuinely unset"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn clear_leaves_the_other_axes_untouched() {
    let store = Store::memory().await.unwrap();
    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs {
            language: Some("es".into()),
            date_style: Some(DateStyle::Eu),
            unit_system: Some(UnitSystem::Imperial),
            ui_theme: Some(json!({ "preset": "neon" })),
            ..Prefs::default()
        },
        &[],
    )
    .await
    .unwrap();

    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs::default(),
        &[PrefsAxis::UiTheme],
    )
    .await
    .unwrap();

    let got = get_user_prefs(&store, "nube", "user:test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.ui_theme, None, "the named axis is cleared");
    assert_eq!(got.language, Some("es".into()), "language survives");
    assert_eq!(got.date_style, Some(DateStyle::Eu), "date_style survives");
    assert_eq!(
        got.unit_system,
        Some(UnitSystem::Imperial),
        "unit_system survives"
    );
}

/// A patch and a clear in ONE call: set some axes while releasing another. This is what the theme
/// customizer's "reset" sends.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn patch_and_clear_apply_together() {
    let store = Store::memory().await.unwrap();
    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs {
            language: Some("es".into()),
            ui_theme: Some(json!({ "preset": "neon" })),
            ..Prefs::default()
        },
        &[],
    )
    .await
    .unwrap();

    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs {
            language: Some("en".into()),
            ..Prefs::default()
        },
        &[PrefsAxis::UiTheme],
    )
    .await
    .unwrap();

    let got = get_user_prefs(&store, "nube", "user:test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.language, Some("en".into()), "the patch applied");
    assert_eq!(got.ui_theme, None, "the clear applied in the same write");
}

/// An axis named in BOTH the patch and the clear list is cleared — the caller asked for it to
/// inherit, and honouring the set would silently drop half the request.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn clear_wins_over_a_set_of_the_same_axis() {
    let store = Store::memory().await.unwrap();
    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs {
            ui_theme: Some(json!({ "preset": "neon" })),
            ..Prefs::default()
        },
        &[PrefsAxis::UiTheme],
    )
    .await
    .unwrap();

    let got = get_user_prefs(&store, "nube", "user:test").await.unwrap();
    assert_eq!(
        got.and_then(|p| p.ui_theme),
        None,
        "clear beats a set of the same axis in one call"
    );
}

/// Clearing the WORKSPACE default drops it a link further, to the built-in fallback.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn clearing_the_workspace_default_falls_through_to_builtin() {
    let store = Store::memory().await.unwrap();
    set_workspace_prefs(
        &store,
        "nube",
        &Prefs {
            ui_theme: Some(json!({ "preset": "corporate" })),
            unit_system: Some(UnitSystem::Imperial),
            ..Prefs::default()
        },
        &[],
    )
    .await
    .unwrap();

    set_workspace_prefs(&store, "nube", &Prefs::default(), &[PrefsAxis::UiTheme])
        .await
        .unwrap();

    let ws = get_workspace_prefs(&store, "nube").await.unwrap().unwrap();
    assert_eq!(ws.ui_theme, None, "the workspace axis is cleared");
    assert_eq!(
        ws.unit_system,
        Some(UnitSystem::Imperial),
        "the workspace's other axes survive"
    );

    let resolved = resolve_chain(&store, "nube", "user:bob", None)
        .await
        .unwrap();
    assert_eq!(
        resolved.ui_theme, None,
        "with neither link set, the built-in fallback (no theme) resolves"
    );
}

/// Clearing an axis that was never set is a no-op, not an error — so a blanket "reset" is safe to
/// send without first reading what the member had.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn clearing_an_unset_axis_is_a_no_op() {
    let store = Store::memory().await.unwrap();
    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs {
            language: Some("es".into()),
            ..Prefs::default()
        },
        &[PrefsAxis::UiTheme, PrefsAxis::UiBranding],
    )
    .await
    .unwrap();

    let got = get_user_prefs(&store, "nube", "user:test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.language, Some("es".into()));
    assert_eq!(got.ui_theme, None);
}

/// A clear on a record that does not exist yet must not resurrect a half-built row with surprising
/// contents — the upsert creates it with the axis unset, which resolves as "inherit".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn clearing_on_a_fresh_record_is_safe() {
    let store = Store::memory().await.unwrap();
    set_user_prefs(
        &store,
        "nube",
        "user:new",
        &Prefs::default(),
        &[PrefsAxis::UiTheme],
    )
    .await
    .unwrap();

    let resolved = resolve_chain(&store, "nube", "user:new", None)
        .await
        .unwrap();
    assert_eq!(resolved.ui_theme, None);
    // The i18n axes still fold to the built-in fallback.
    assert_eq!(resolved.language, "en");
}

/// Every axis is clearable — the enum is the closed set, and each name maps to a real column. A new
/// axis that forgets its `PrefsAxis` arm shows up here as a compile error, not a silent gap.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_axis_can_be_cleared() {
    let store = Store::memory().await.unwrap();
    let mut seeded = Prefs {
        language: Some("es".into()),
        timezone: Some("Europe/Madrid".into()),
        date_style: Some(DateStyle::Eu),
        unit_system: Some(UnitSystem::Imperial),
        ui_theme: Some(json!({ "preset": "neon" })),
        ui_branding: Some(json!({ "siteName": "Nube" })),
        insight_notifications: Some(false),
        agent_persona: Some("analyst".into()),
        push_muted: Some(true),
        ..Prefs::default()
    };
    seeded
        .unit_overrides
        .insert(lb_prefs::Dimension::Speed, lb_prefs::Unit::Knot);
    set_user_prefs(&store, "nube", "user:test", &seeded, &[])
        .await
        .unwrap();

    let all = [
        PrefsAxis::Language,
        PrefsAxis::Timezone,
        PrefsAxis::DateStyle,
        PrefsAxis::TimeStyle,
        PrefsAxis::FirstDayOfWeek,
        PrefsAxis::NumberFormat,
        PrefsAxis::UnitSystem,
        PrefsAxis::UnitOverrides,
        PrefsAxis::UiTheme,
        PrefsAxis::UiBranding,
        PrefsAxis::InsightNotifications,
        PrefsAxis::AgentPersona,
        PrefsAxis::PushMuted,
    ];
    set_user_prefs(&store, "nube", "user:test", &Prefs::default(), &all)
        .await
        .unwrap();

    let got = get_user_prefs(&store, "nube", "user:test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        got,
        Prefs::default(),
        "clearing every axis leaves a fully-inheriting record"
    );
}

/// A clear is namespace-scoped like every other write — clearing in one workspace must not touch
/// the same user's record in another.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn clear_is_workspace_isolated() {
    let store = Store::memory().await.unwrap();
    let themed = Prefs {
        ui_theme: Some(json!({ "preset": "neon" })),
        ..Prefs::default()
    };
    set_user_prefs(&store, "nube", "user:test", &themed, &[])
        .await
        .unwrap();
    set_user_prefs(&store, "globex", "user:test", &themed, &[])
        .await
        .unwrap();

    set_user_prefs(
        &store,
        "nube",
        "user:test",
        &Prefs::default(),
        &[PrefsAxis::UiTheme],
    )
    .await
    .unwrap();

    let nube = get_user_prefs(&store, "nube", "user:test")
        .await
        .unwrap()
        .unwrap();
    let globex = get_user_prefs(&store, "globex", "user:test")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(nube.ui_theme, None, "cleared in nube");
    assert_eq!(
        globex.ui_theme,
        Some(json!({ "preset": "neon" })),
        "globex is untouched"
    );
}
