//! Capacity tower. TS counterparts: src/workers/algorithm/irregular/intrinsicCapacity*.ts

pub mod endpoint;
pub mod material;
pub mod mode;
pub mod prefixes;
pub mod preflight;
pub mod search;
pub mod telemetry;

pub(crate) fn serialize_bigint_decimal_string<S>(
    value: &num_bigint::BigInt,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&value.to_string())
}
