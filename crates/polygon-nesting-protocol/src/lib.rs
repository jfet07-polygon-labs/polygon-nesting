mod error;
mod event;
mod request;
pub mod result;
mod version;

use serde::{Deserialize, Serialize};

pub use error::{EngineError, EngineErrorCode, ProtocolError};
pub use event::{EngineEvent, PortfolioPhase, PortfolioProgress, SequencedEngineEvent};
pub use request::{
    ArchiveIneligibilityReason, CutRowReference, DiagnosticTraceMode, EllipseSource,
    EllipseSourceKind, EngineProfile, EngineRequest, EngineSettings, GeometrySettings, HistoryMode,
    OptimizerSettings, PlacementPolicy, PreparedPiece, Rect, RectWithMetrics, SheetSpec,
    SourceArcSegment, SourceEntityHandle, SourceGeometry, SourceGeometryEntityType,
    SourceGeometrySegment, SourceLineSegment, SourcePiece, SourceWarning,
};
pub use result::{
    Bounds, CapacityTrace, CollisionGeometry, CollisionTransform, EngineOutcome, EngineResult,
    ExactDecimalString, ExecutionDiagnostics, FocusedCompleteReconstructionTrace,
    FreeMaterialSnapshot, IntrinsicAnytimeSchedulerTrace, IntrinsicShortSideObserverTrace,
    IntrinsicShortSidePairFoldTrace, IrregularTransformReason, LayoutScore, LayoutScoreSummary,
    PlacedCollisionGeometry, Placement, PlacementReference, PlacementTransform, Point, Polygon,
    PortfolioResult, PortfolioStatus, PortfolioTerminationReason, PreparedCollisionGeometry,
    PriorityOrderKey, ResultDiagnostic, SearchSource, SnapshotPreparedPiece, StateSnapshot,
    StateSnapshotSource,
};
pub use version::{ProtocolVersion, PROTOCOL_VERSION};

pub const EXACT_DECIMAL_FIELD_NAMES: [&str; 5] = [
    "maximumSingletonSpanPressurePpm",
    "minimumCollisionAreaPressurePpm",
    "minimumDoubledCollisionAreaSumGrid2",
    "placedDoubledMaterialAreaGrid2",
    "sheetDoubledAreaGrid2",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineInfo {
    pub name: String,
    pub version: String,
}

pub fn decode_request(input: impl AsRef<[u8]>) -> Result<EngineRequest, ProtocolError> {
    let request: EngineRequest =
        serde_json::from_slice(input.as_ref()).map_err(|error| ProtocolError::MalformedInput {
            message: error.to_string(),
        })?;
    request.validate()?;
    Ok(request)
}

pub fn encode_request(request: &EngineRequest) -> Result<Vec<u8>, ProtocolError> {
    request.validate()?;
    serde_json::to_vec(request).map_err(|error| ProtocolError::Encoding {
        message: error.to_string(),
    })
}

pub fn encode_outcome(outcome: &EngineOutcome) -> Result<Vec<u8>, ProtocolError> {
    let outcome_value = serde_json::to_value(outcome).map_err(|error| ProtocolError::Encoding {
        message: error.to_string(),
    })?;
    validate_exact_decimal_fields(&outcome_value)?;

    #[derive(Serialize)]
    struct Envelope<'a> {
        version: ProtocolVersion,
        outcome: &'a EngineOutcome,
    }

    serde_json::to_vec(&Envelope {
        version: ProtocolVersion::CURRENT,
        outcome,
    })
    .map_err(|error| ProtocolError::Encoding {
        message: error.to_string(),
    })
}

pub fn encode_event(event: &SequencedEngineEvent) -> Result<Vec<u8>, ProtocolError> {
    serde_json::to_vec(event).map_err(|error| ProtocolError::Encoding {
        message: error.to_string(),
    })
}

fn validate_exact_decimal_fields(value: &serde_json::Value) -> Result<(), ProtocolError> {
    match value {
        serde_json::Value::Object(object) => {
            for (field, value) in object {
                if is_exact_decimal_field(field) {
                    let Some(decimal) = value.as_str() else {
                        return Err(ProtocolError::InvalidDecimalString {
                            field: field.clone(),
                            value: value.to_string(),
                        });
                    };
                    if !result::is_canonical_decimal(decimal) {
                        return Err(ProtocolError::InvalidDecimalString {
                            field: field.clone(),
                            value: decimal.to_owned(),
                        });
                    }
                }
                validate_exact_decimal_fields(value)?;
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                validate_exact_decimal_fields(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn is_exact_decimal_field(field: &str) -> bool {
    EXACT_DECIMAL_FIELD_NAMES.contains(&field)
}
