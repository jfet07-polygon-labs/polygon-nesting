//! **The meter: strike-work and currency primitives for the economics round.**
//!
//! Wave 2b of docs/economics-round-spec.md. Three primitives, none of them
//! wired into a trajectory by this wave:
//!
//! * [`strike_meter`] - the two-arm strike accounting. The control arm is the
//!   frozen literals `200 / 3 / 100 / 5 / 0.98`; the treatment arm is the
//!   work-denominated impatient policy with the frozen KNOB quanta
//!   `1_630_000` / `815_000`. Both arms call the **same, untouched**
//!   [`observe_raw`](crate::search::overlap_ics::observe_raw) classifier, and
//!   strike semantics are the only delta between them.
//! * [`currency`] - `U = sample_evaluations + B*master_batches
//!   + E*actual_publication_attempt_calls + R*repair_rows + D*disruption_moves`,
//!   the conservative derivation of `B/E/R/D` from timing-only measurements,
//!   and the spec's >10 % wall-prediction reject check.
//! * [`pacer`] - consume-units-between-master-batches bookkeeping, the frozen
//!   80/20 share spent in calibrated units, compress decay advanced by
//!   consumed compress-work, and a clock the pacer structurally cannot read.
//!
//! # Why this module is a sibling of `overlap_ics` and not a child of it
//!
//! Sol review 19 §5 assigns `search/overlap_ics/mod.rs` - `run_cutclose`, the
//! `Pacer` wiring and the document schema - to **one** integration agent, and
//! forbids the two parallel wave-2 agents from editing it. A child module
//! would have to be declared inside that file, which is the one edit the
//! workflow exists to prevent and the one edit that would collide with the
//! executor agent working in parallel. So the primitives live here, reached as
//! `crate::search::overlap_ics_meter::*`, and `search/overlap_ics/mod.rs` is
//! **byte-for-byte unchanged by this wave**. The integration agent moves them
//! wherever it wants them; nothing here depends on the location.
//!
//! # What "not wired" means, structurally
//!
//! No type in this module holds an [`Engine`](crate::search::overlap_ics::Engine),
//! and nothing in `search/overlap_ics/` names this module. Every primitive is
//! driven by values the caller already has - a raw Φ, a batch's sample
//! evaluations, a plan that was read somewhere else - so a trajectory can only
//! acquire one by an integration diff that says so.
//!
//! The clock rule the evidence audit verified holds here by construction and by
//! test: `std::time::Instant` does not appear in this module, and
//! [`pacer::PoisonedClock`] is a test double that fails the vector if a paced
//! trajectory ever reads one.

/// The calibrated-work currency `U`, its coefficients, and the reject check.
pub mod currency;
/// The calibrated-work pacer primitive: unit bookkeeping between master
/// batches, with no clock.
pub mod pacer;
/// The two-arm strike meter: iteration-denominated control, work-denominated
/// treatment, one classifier.
pub mod strike_meter;
