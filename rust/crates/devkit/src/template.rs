use crate::{Feature, TemplateInfo, Tier};

pub const WORLD: &str = "lazybones:ext/extension@0.2.0";

pub fn templates() -> Vec<TemplateInfo> {
    vec![
        TemplateInfo {
            tier: Tier::Wasm,
            label: "Tier-1 WASM component".into(),
            features: Feature::all(),
            world: WORLD.into(),
        },
        TemplateInfo {
            tier: Tier::Native,
            label: "Tier-2 native sidecar".into(),
            features: Feature::all(),
            world: WORLD.into(),
        },
    ]
}
