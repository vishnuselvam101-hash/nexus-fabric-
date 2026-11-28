//! The AMOEBA operator. It is AMOEBA's own body, not a separate program: the hands are the AMOEBA tool bridge
//! (`nf-amoeba-agent`), the checks are the server's gates and the engine. The eyes are the server's own page
//! reader: it stores every reading and the operator takes them from it (`eyes::PageRead`). The brain (a planner)
//! is a replaceable part behind a trait.
//!
//! - `Brain`: today `RulePlanner`, a deterministic plan that calls the same tools a model would call; later a
//!   local model that proposes the same `Action`s. The tools check what it proposes.
//! - `Registry`: the tools by name. A tool does one thing, says what it did as events and returns data. A new
//!   kind of work (for example NF CAD: import a DXF, inspect entities, measure, edit, export) is a new tool
//!   registered under a new name; the planner trait, the cursor and the audit do not change, because an event
//!   names its target as a page box, a UI element or a CAD canvas point/entity.
//!
//! The operator proposes; the engine and the server decide. It never approves or releases, never marks a result
//! checked, and a reading that carries a reader flag is never written as confirmed.

pub mod events;
pub mod eyes;
pub mod layout;
pub mod operator;
pub mod replay;
