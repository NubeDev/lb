//! The asset service — the S4 capability + membership chokepoint for shared workspace assets
//! (docs, skills, team membership), mirroring the channel service's discipline (README §6.12,
//! files + skills scopes).
//!
//! Every verb here runs the gates FIRST, in order (capability-first, §3.5; isolation-first,
//! §3.6), before touching the store:
//!   - **gate 1** workspace isolation + **gate 2** capability — `authorize_doc`/`authorize_skill`
//!     via the shared `caps::check` (`store:doc/*`, `store:skill/*`);
//!   - **gate 3** membership/grant — `visibility::may_read_doc` for docs, the `grant` relation
//!     for skills. The capability gate says "may use the surface"; gate 3 says "may see *this*
//!     asset" (the second isolation layer the tenancy scope deferred).
//!
//! The raw store persistence lives in `lb_assets`; this layer is authorization + the membership
//! graph resolution only. One verb per file (FILE-LAYOUT §3).

mod add_member;
mod authorize;
mod backlinks;
mod delete_asset;
mod delete_doc;
mod deprecate_skill;
mod error;
mod extract;
mod get_asset;
mod get_doc;
mod grant_skill;
mod link_doc;
mod links;
mod list_assets;
mod list_docs;
mod list_granted_skills;
mod load_skill;
mod put_asset;
mod put_doc;
mod put_skill;
mod share_doc;
mod tool;
mod unshare_doc;
mod visibility;

pub use add_member::add_member;
pub use backlinks::backlinks;
pub use delete_asset::delete_asset;
pub use delete_doc::delete_doc;
pub use deprecate_skill::deprecate_skill;
pub use error::AssetError;
pub use extract::{
    call_docs_tool, docs_extract, extract_descriptor, get_extraction, ExtractRequest,
    ExtractResult, ExtractSvcError, Extraction, ItemOutcome, DERIVED_FROM, EXTRACTION_TABLE,
};
pub use get_asset::get_asset;
pub use get_doc::get_doc;
pub use grant_skill::{grant_skill, revoke_skill};
/// The skill-grant relation kind + fixed `b` scope, re-exported for the default-grant-at-creation
/// helper (`workspaces::default_skills`) so it writes the SAME edge `grant_skill`/`load_skill` use.
pub(crate) use grant_skill::{GRANT, GRANT_SCOPE};
pub use link_doc::link_doc;
pub use list_assets::list_assets;
pub use list_docs::list_docs;
pub use list_granted_skills::{list_granted_skills, SkillCatalogEntry, SkillTier};
pub use load_skill::load_skill;
pub use put_asset::{put_asset, MAX_ASSET_BYTES};
pub use put_doc::put_doc;
pub use put_skill::put_skill;
pub use share_doc::share_doc;
pub use tool::call_asset_tool;
pub use unshare_doc::unshare_doc;
