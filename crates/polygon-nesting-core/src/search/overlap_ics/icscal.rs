//! **The persisted calibrated-work plan: the file format, and only its writer.**
//!
//! docs/economics-round-spec.md, funded change 3, verbatim on what the file
//! must pin:
//!
//! > The file pins request hash, currency version, binary/feature key,
//! > workers=8, executor implementation, per-phase safe units/s; read/write
//! > separate; no live probe on a gated trajectory.
//!
//! Wave 1 builds the schema and the write path. **There is no reader in this
//! module and there is no `Pacer` change anywhere in this round**, because the
//! spec sequences the pacer after the strike experiment and the executor
//! freeze, and because a reader that exists is a reader something can call.
//!
//! # Why "read/write separate" is a layout rule and not a style note
//!
//! The pre-named defect this format exists under is (3) in the spec's ranked
//! list: *probe-on-cheap-bites*. A plan calibrated on bites 1-21 overstates
//! iterations per second by about 1.5x, because those bites publish in one to
//! fifteen master iterations from a nearly-feasible parent while the 179 shelf
//! does not publish at all. The census in
//! docs/experiments/overlap-ics/economics-round/census/ measures that ratio
//! directly.
//!
//! The structural half of the defence is that **a gated trajectory must not be
//! able to acquire a fresh probe**. So the writer takes a finished
//! [`WorkPlan`] and turns it into bytes; it cannot measure anything, it cannot
//! run anything, and it holds no engine type. When the reader is built it will
//! be a separate entry point with its own type, and the rule it has to satisfy
//! is that nothing on a gated trajectory calls the writer and nothing that
//! writes can also decide.
//!
//! # The keys, and why each one is in the file
//!
//! A plan is only valid for the exact machine-and-question it was measured
//! for. Every field of [`PlanKey`] is a thing that, if it changed, would make
//! the units-per-second number a lie rather than an approximation:
//!
//! * `request_sha256` - a different fixture is different work per unit. The
//!   campaign's three fixtures differ by more than 2x on
//!   `sampleEvaluationsPerRelocate` alone.
//! * `currency_version` - `U` is a *vector reduced by coefficients*
//!   (`sample_evaluations + B*master_batches + E*publication_attempts +
//!   R*repair_rows + D*disruption_moves`). Coefficients measured under one
//!   version may not be read under another, so the version is a key and not a
//!   comment. Wave 1 can only honestly write [`CurrencyVersion::U0Samples`];
//!   `U1` is Wave 3's to write once B/E/R/D are measured.
//! * `binary_key` - the executable's own sha256 **and** the feature set. A
//!   `ics-profile` build and a default build are different binaries and the
//!   census proves they take the same trajectory, not that they take the same
//!   *time*; a plan measured on one may not be spent by the other.
//! * `workers` - frozen at 8 by the spec, written down anyway, because a plan
//!   is a statement about a machine with eight sweeps in flight.
//! * `executor` - ephemeral scope or persistent pool. The whole point of the
//!   economics round's second funded change is that these two have different
//!   per-iteration costs; a plan that did not name which one measured it would
//!   be exactly the "stable but false work accounting" the spec ranks as this
//!   round's worst defect class.
//!
//! # "Safe" units per second
//!
//! [`PhasePlan::safe_units_per_second`] is deliberately not the measured mean.
//! It is a **conservative** rate - the one a plan may spend against - and the
//! measured mean sits beside it so the discount is visible instead of baked
//! in. [`PhasePlan::from_measurement`] applies the discount in one place, and
//! a plan built any other way carries `derivation` saying so.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// The schema tag written into every file, and the only value a future reader
/// may accept without a migration.
pub const SCHEMA: &str = "icscal/v1";

/// How `U` was reduced from the work vector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrencyVersion {
    /// `U = sample_evaluations`. The only currency Wave 1 measured, and an
    /// honest one: it is the member's own work unit and it needs no
    /// coefficient. It is **not** the spec's `U` and must not be read as it.
    #[serde(rename = "U0-sample-evaluations")]
    U0Samples,
    /// The spec's currency: `sample_evaluations + B*master_batches +
    /// E*actual_publication_attempt_calls + R*repair_rows + D*disruption_moves`,
    /// with B/E/R/D from timing-only microbenchmarks on all three fixtures and
    /// conservative rounding. Nothing in this round may write it: the
    /// coefficients do not exist yet.
    #[serde(rename = "U1-weighted-vector")]
    U1Weighted,
}

impl CurrencyVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::U0Samples => "U0-sample-evaluations",
            Self::U1Weighted => "U1-weighted-vector",
        }
    }
}

/// Which master-iteration executor produced the measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Executor {
    /// `std::thread::scope` per master iteration: the shipped implementation,
    /// and the one the census measured.
    #[serde(rename = "ephemeral-scope")]
    EphemeralScope,
    /// A local pool of eight persistent slots refilled with `clone_from`, with
    /// an ordinal merge. Reachable only if the census's pre-committed gate
    /// opens; named here so a plan written by either one says which.
    #[serde(rename = "persistent-pool")]
    PersistentPool,
}

impl Executor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EphemeralScope => "ephemeral-scope",
            Self::PersistentPool => "persistent-pool",
        }
    }
}

/// The trajectory phase a rate was measured on. The two phases have different
/// economics - explore separates from a nearly-feasible parent, compress does
/// not - and one blended rate is the probe-on-cheap-bites defect wearing a
/// different hat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanPhase {
    #[serde(rename = "explore")]
    Explore,
    #[serde(rename = "compress")]
    Compress,
}

impl PlanPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Explore => "explore",
            Self::Compress => "compress",
        }
    }
}

/// The binary a plan was measured on: its own hash and the features it was
/// built with, in the order the caller lists them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryKey {
    pub executable_sha256: String,
    pub features: Vec<String>,
}

/// Everything that has to match before a plan may be spent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanKey {
    pub request_sha256: String,
    pub currency_version: CurrencyVersion,
    pub binary_key: BinaryKey,
    /// Frozen at 8 by the spec. Written because a plan is a claim about a
    /// machine running that many sweeps at once.
    pub workers: usize,
    pub executor: Executor,
}

/// One phase's rate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhasePlan {
    pub phase: PlanPhase,
    /// The rate a plan may spend against: **conservative**, never the mean.
    pub safe_units_per_second: f64,
    /// The measurement the safe rate was discounted from, so a reader can see
    /// the discount instead of trusting it.
    pub measured_units_per_second: f64,
    /// `safe / measured`, restated so a reader does not have to divide.
    pub safety_factor: f64,
    /// How many units the measurement observed. A rate from a short window is
    /// a rate with a wide error bar and the file says how short.
    pub observed_units: u64,
    pub observed_seconds: f64,
    /// Free text naming the cell the rate came from. The census writes the
    /// bite range here, because "which bites" is the difference between a
    /// plan and the probe-on-cheap-bites defect.
    pub derivation: String,
}

impl PhasePlan {
    /// The one place the safety discount is applied.
    ///
    /// `safety_factor` must be in `(0, 1]`: a "safe" rate above the measured
    /// one is a plan that overspends by construction, and this refuses to
    /// write one rather than rounding it away.
    pub fn from_measurement(
        phase: PlanPhase,
        observed_units: u64,
        observed_seconds: f64,
        safety_factor: f64,
        derivation: impl Into<String>,
    ) -> Result<Self, String> {
        // NaN fails both of these, which is the point: a window or a discount
        // that is not a number has no honest reading, and refusing it here is
        // cheaper than discovering it in a plan.
        if observed_seconds.is_nan() || observed_seconds <= 0.0 {
            return Err(format!(
                "{}: a rate needs a positive window, not {observed_seconds}",
                phase.as_str()
            ));
        }
        if safety_factor.is_nan() || safety_factor <= 0.0 || safety_factor > 1.0 {
            return Err(format!(
                "{}: the safety factor must be in (0, 1], not {safety_factor}",
                phase.as_str()
            ));
        }
        let measured = observed_units as f64 / observed_seconds;
        Ok(Self {
            phase,
            safe_units_per_second: measured * safety_factor,
            measured_units_per_second: measured,
            safety_factor,
            observed_units,
            observed_seconds,
            derivation: derivation.into(),
        })
    }
}

/// A calibrated-work plan, as it is written to disk.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkPlan {
    pub schema: String,
    pub key: PlanKey,
    pub phases: Vec<PhasePlan>,
    /// **The `U1` coefficients, when the plan is denominated in `U1`.**
    ///
    /// Wave 2b. `key.currency_version` says *which* currency the rates were
    /// measured in; this says what that currency's exchange rates were, so a
    /// plan is self-contained and a reader never has to find the calibration
    /// that produced it. Absent for [`CurrencyVersion::U0Samples`], which has
    /// no coefficients at all - `U = sample_evaluations` - and the two are
    /// cross-checked in [`WorkPlan::validate`] rather than left to agree.
    ///
    /// `skip_serializing_if` keeps a `U0` plan's bytes exactly what Wave 1
    /// wrote: the census's committed plan re-serialises byte for byte, and the
    /// vector in `search/overlap_ics_meter/pacer.rs` proves it against the
    /// committed file rather than asserting it here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<crate::search::overlap_ics_meter::currency::Coefficients>,
    /// What produced this file. Never parsed; it is the line a human reads
    /// first when a plan turns out to be wrong.
    pub provenance: String,
}

impl WorkPlan {
    pub fn new(key: PlanKey, phases: Vec<PhasePlan>, provenance: impl Into<String>) -> Self {
        Self {
            schema: SCHEMA.to_owned(),
            key,
            phases,
            currency: None,
            provenance: provenance.into(),
        }
    }

    /// The same plan, carrying the coefficients its currency version names.
    pub fn with_currency(
        mut self,
        coefficients: crate::search::overlap_ics_meter::currency::Coefficients,
    ) -> Self {
        self.currency = Some(coefficients);
        self
    }

    /// The plan's own validity clauses, checked before it is written rather
    /// than after it is read.
    ///
    /// A file that fails any of these has no honest reading, so writing it and
    /// leaving the reader to notice would be a way of shipping the defect with
    /// a diagnostic attached.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA {
            return Err(format!("schema `{}` is not `{SCHEMA}`", self.schema));
        }
        if self.key.workers == 0 {
            return Err("a plan with zero workers describes no machine".to_owned());
        }
        if self.key.request_sha256.len() != 64 {
            return Err(format!(
                "requestSha256 `{}` is not a sha256",
                self.key.request_sha256
            ));
        }
        if self.key.binary_key.executable_sha256.len() != 64 {
            return Err(format!(
                "executableSha256 `{}` is not a sha256",
                self.key.binary_key.executable_sha256
            ));
        }
        if self.phases.is_empty() {
            return Err("a plan with no phase rates cannot pace anything".to_owned());
        }
        // The currency version and the coefficients have to say the same
        // thing. A `U1` plan without coefficients is a rate whose exchange
        // rates nobody can reconstruct; a `U0` plan *with* them is a plan
        // claiming a currency it was not measured in.
        match (self.key.currency_version, self.currency.is_some()) {
            (CurrencyVersion::U1Weighted, false) => {
                return Err(
                    "U1 is a weighted vector: the plan must pin B/E/R/D and pins none".to_owned(),
                );
            }
            (CurrencyVersion::U0Samples, true) => {
                return Err(
                    "U0 is sample evaluations alone and has no coefficients to pin".to_owned(),
                );
            }
            _ => {}
        }
        for phase in &self.phases {
            if !(phase.safe_units_per_second > 0.0 && phase.safe_units_per_second.is_finite()) {
                return Err(format!(
                    "{}: safeUnitsPerSecond {} is not a positive finite rate",
                    phase.phase.as_str(),
                    phase.safe_units_per_second
                ));
            }
            if phase.safe_units_per_second > phase.measured_units_per_second {
                return Err(format!(
                    "{}: a safe rate above the measured one overspends by construction",
                    phase.phase.as_str()
                ));
            }
        }
        for (index, phase) in self.phases.iter().enumerate() {
            if self.phases[..index].iter().any(|other| other.phase == phase.phase) {
                return Err(format!("{} appears twice", phase.phase.as_str()));
            }
        }
        Ok(())
    }

    /// The bytes of the file. **The whole write path**, and it does not touch
    /// the filesystem: the caller owns where a plan lands, so nothing here can
    /// write into a tree it was not handed.
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut text = serde_json::to_string_pretty(self)
            .map_err(|error| format!("serialising the plan: {error}"))?;
        text.push('\n');
        Ok(text.into_bytes())
    }

    /// A one-line human summary, for a driver's log. Never the file.
    pub fn summary(&self) -> String {
        let mut line = format!(
            "{} {} workers={} executor={}",
            self.schema,
            self.key.currency_version.as_str(),
            self.key.workers,
            self.key.executor.as_str()
        );
        for phase in &self.phases {
            let _ = write!(
                line,
                " {}={:.0}u/s",
                phase.phase.as_str(),
                phase.safe_units_per_second
            );
        }
        line
    }
}
