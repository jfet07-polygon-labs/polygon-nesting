//! **The `icscal/v1` reader. Only the reader.**
//!
//! docs/economics-round-spec.md, funded change 3, verbatim: *"read/write
//! separate; no live probe on a gated trajectory"*. Wave 1 built the schema and
//! [`the writer`](super::icscal) and deliberately built no reader at all, on
//! the rule that *a reader that exists is a reader something can call*. Wave 3
//! calls one, so it exists - and this module is the whole of it.
//!
//! # What "separate" buys, concretely
//!
//! The pre-named defect this format exists under is the spec's (3),
//! *probe-on-cheap-bites*: a rate calibrated on the cheap 0.1 % prefix
//! overstates iterations per second by about 1.5x, and the census measured
//! exactly that. The structural defence is that a gated trajectory must not be
//! able to acquire a fresh rate. Three properties keep it:
//!
//! 1. **This module cannot measure.** It holds no engine type, no clock and no
//!    counter. It turns bytes into a [`WorkPlan`] and refuses bytes that are
//!    not one.
//! 2. **This module cannot write.** There is no `to_bytes` here and no `fs`
//!    import; the writer is a different file and neither imports the other.
//! 3. **This module cannot *find* a plan.** It takes bytes. It does not take a
//!    path, it does not know a default location, and it never touches the
//!    filesystem, so the only way a plan reaches a trajectory is that a caller
//!    outside the engine went and got one and said which.
//!
//! The engine's own half of the rule is in [`Budget::CalibratedWork`]: the
//! pacer arrives already built, so `Engine` cannot construct a plan out of
//! anything it holds even if this function were in scope.
//!
//! [`Budget::CalibratedWork`]: super::Budget::CalibratedWork
//!
//! # Refusal is the only failure mode
//!
//! A plan that does not parse, whose schema tag is not `icscal/v1`, or that
//! fails [`WorkPlan::validate`], is an error and never a warning. There is no
//! "fall back to measuring", because that fallback *is* the defect: it would
//! turn a stale or mistyped file into a live probe on a gated trajectory,
//! silently.

use super::icscal::{WorkPlan, SCHEMA};

/// **Bytes to a plan.** The whole read path.
///
/// The schema tag is checked before `serde` is trusted with the rest, so a
/// file from a future version is refused by name rather than by whichever
/// field happens to have moved. The plan's own clauses
/// ([`WorkPlan::validate`]) are then re-checked on the way *in*, not only on
/// the way out: a file may have been edited since it was written, and the
/// writer's validation says nothing about the bytes on disk now.
pub fn plan_from_bytes(bytes: &[u8]) -> Result<WorkPlan, String> {
    let tag: SchemaTag = serde_json::from_slice(bytes)
        .map_err(|error| format!("this is not an icscal plan: {error}"))?;
    if tag.schema != SCHEMA {
        return Err(format!(
            "schema `{}` is not `{SCHEMA}`: this reader accepts one version and refuses the rest",
            tag.schema
        ));
    }
    let plan: WorkPlan = serde_json::from_slice(bytes)
        .map_err(|error| format!("reading an {SCHEMA} plan: {error}"))?;
    plan.validate()?;
    Ok(plan)
}

/// Just enough of the document to check the version before parsing the rest.
#[derive(serde::Deserialize)]
struct SchemaTag {
    schema: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::overlap_ics::icscal::{
        BinaryKey, CurrencyVersion, Executor, PhasePlan, PlanKey, PlanPhase,
    };

    const SHA: &str = "ecfe126f431f08b817813d4af1ad438399585c6cc1c4f16b835e5b6874878bb3";
    const BIN: &str = "d9ef083e41beeee1cf189773a04ce7d789fc238dadcb349c6ad05e55cbd8120d";

    fn plan() -> WorkPlan {
        WorkPlan::new(
            PlanKey {
                request_sha256: SHA.to_owned(),
                currency_version: CurrencyVersion::U0Samples,
                binary_key: BinaryKey {
                    executable_sha256: BIN.to_owned(),
                    features: vec!["overlap-ics".to_owned()],
                },
                workers: 8,
                executor: Executor::EphemeralScope,
            },
            vec![
                PhasePlan::from_measurement(PlanPhase::Explore, 4_000_000, 2.0, 0.8, "a vector")
                    .unwrap(),
                PhasePlan::from_measurement(PlanPhase::Compress, 1_000_000, 1.0, 0.8, "a vector")
                    .unwrap(),
            ],
            "a vector",
        )
    }

    /// The round trip the pacer depends on: what the writer wrote is what the
    /// reader reads, field for field.
    #[test]
    fn a_written_plan_reads_back_identical() {
        let written = plan();
        let bytes = written.to_bytes().expect("the plan must serialise");
        let read = plan_from_bytes(&bytes).expect("the reader must accept the writer's bytes");
        assert_eq!(read, written, "the reader must not change the plan");
    }

    /// **The committed census plan parses.** Not a synthetic file: the bytes
    /// Wave 1 measured and committed, which is the only plan in the tree with
    /// a provenance outside a test.
    #[test]
    fn the_committed_census_plan_parses() {
        const COMMITTED: &str = include_str!(
            "../../../../../docs/experiments/overlap-ics/economics-round/census/evidence/mixed61-w8-seed0.icscal.json"
        );
        let read = plan_from_bytes(COMMITTED.as_bytes()).expect("the committed plan must parse");
        assert_eq!(read.schema, SCHEMA);
        assert_eq!(read.key.currency_version, CurrencyVersion::U0Samples);
        assert_eq!(read.key.workers, 8);
        assert_eq!(read.key.executor, Executor::EphemeralScope);
        assert!(
            read.currency.is_none(),
            "a U0 plan pins no coefficients: {read:?}"
        );
        // And back out again, byte for byte. The writer's own vector proves
        // this from the writer's side; this proves the reader is not the loose
        // end.
        assert_eq!(
            String::from_utf8(read.to_bytes().expect("re-serialise")).expect("utf-8"),
            COMMITTED,
            "the committed plan must re-serialise byte for byte through the reader"
        );
    }

    #[test]
    fn a_future_schema_is_refused_by_name() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&plan().to_bytes().unwrap()).unwrap();
        value["schema"] = serde_json::json!("icscal/v2");
        let error = plan_from_bytes(serde_json::to_string(&value).unwrap().as_bytes())
            .expect_err("a v2 plan must be refused");
        assert!(error.contains("icscal/v2"), "{error}");
        assert!(error.contains(SCHEMA), "{error}");
    }

    #[test]
    fn bytes_that_are_not_json_are_refused() {
        let error = plan_from_bytes(b"not a plan").expect_err("garbage must be refused");
        assert!(error.contains("not an icscal plan"), "{error}");
    }

    /// A file edited after it was written is re-checked on the way in. The
    /// writer's validation is a statement about the bytes it produced, not
    /// about the bytes on disk now.
    #[test]
    fn a_tampered_plan_fails_validation_on_read() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&plan().to_bytes().unwrap()).unwrap();
        // A "safe" rate above the measured one overspends by construction.
        value["phases"][0]["safeUnitsPerSecond"] = serde_json::json!(
            value["phases"][0]["measuredUnitsPerSecond"]
                .as_f64()
                .unwrap()
                * 2.0
        );
        let error = plan_from_bytes(serde_json::to_string(&value).unwrap().as_bytes())
            .expect_err("an overspending plan must be refused on read");
        assert!(error.contains("overspends"), "{error}");
    }
}
