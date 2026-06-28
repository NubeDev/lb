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
mod error;
mod get_doc;
mod grant_skill;
mod link_doc;
mod list_docs;
mod list_granted_skills;
mod load_skill;
mod put_doc;
mod put_skill;
mod share_doc;
mod tool;
mod visibility;

pub use add_member::add_member;
pub use error::AssetError;
pub use get_doc::get_doc;
pub use grant_skill::{grant_skill, revoke_skill};
pub use link_doc::link_doc;
pub use list_docs::list_docs;
pub use list_granted_skills::{list_granted_skills, SkillCatalogEntry};
pub use load_skill::load_skill;
pub use put_doc::put_doc;
pub use put_skill::put_skill;
pub use share_doc::share_doc;
pub use tool::call_asset_tool;
