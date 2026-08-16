use std::collections::BTreeMap;

use polygon_nesting_protocol::{
    DiagnosticTraceMode, EngineProfile, EngineRequest, EngineSettings, GeometrySettings,
    HistoryMode, OptimizerSettings, PreparedPiece, ProtocolError, ProtocolVersion, SheetSpec,
    SourcePiece,
};
use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_diagnostic_trace_mode() -> String {
    "full".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterError {
    pub category: String,
    pub operation: String,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub context: BTreeMap<String, String>,
}

impl AdapterError {
    fn new(
        category: impl Into<String>,
        operation: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category: category.into(),
            operation: operation.into(),
            message: message.into(),
            context: BTreeMap::new(),
        }
    }

    fn with_context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("adapter errors always serialize")
    }

    fn malformed_request_json(error: impl std::fmt::Display) -> Self {
        Self::new(
            "worker_protocol_error",
            "decodeRequestJson",
            format!("request JSON did not decode as a NestingRequest: {error}"),
        )
    }

    fn native_api_version_mismatch(received: u32) -> Self {
        Self::new(
            "worker_protocol_error",
            "nativeApiVersion",
            format!("expected NestingRequest.version 1, received {received}"),
        )
        .with_context("nativeApiVersion", "1")
        .with_context("receivedVersion", received.to_string())
    }

    fn revalidation_failed(message: impl Into<String>) -> Self {
        Self::new(
            "irregular_geometry_invalid",
            "nativeBoundaryRevalidation",
            message,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedDesktopRequest {
    pub job_id: String,
    pub strategy_run_id: Option<String>,
    pub request: EngineRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopRequestDto {
    version: u32,
    job_id: String,
    sheet: SheetSpec,
    padding: f64,
    #[serde(default)]
    sheet_edge_clearance_mm: Option<f64>,
    pieces: Vec<PreparedPiece>,
    #[serde(default)]
    source_pieces: Vec<SourcePiece>,
    options: DesktopOptionsDto,
    strategy_run_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopOptionsDto {
    allow_global_rotation: bool,
    #[serde(default = "default_true")]
    allow_global_mirror: bool,
    timeout_ms: f64,
    worker_mode: String,
    history_mode: String,
    #[serde(default = "default_diagnostic_trace_mode")]
    diagnostic_trace_mode: String,
    irregular_settings: DesktopIrregularSettingsDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopIrregularSettingsDto {
    geometry: GeometrySettings,
    optimizer: DesktopOptimizerSettingsDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DesktopOptimizerSettingsDto {
    #[serde(flatten)]
    settings: OptimizerSettings,
    intrinsic_objective_profile_id: DesktopObjectiveProfileId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum DesktopObjectiveProfileId {
    Compact,
    ShortSide,
}

impl From<DesktopObjectiveProfileId> for EngineProfile {
    fn from(value: DesktopObjectiveProfileId) -> Self {
        match value {
            DesktopObjectiveProfileId::Compact => Self::Compact,
            DesktopObjectiveProfileId::ShortSide => Self::CompactShortSide,
        }
    }
}

pub fn adapt_desktop_request_to_engine_json(input: &str) -> Result<String, AdapterError> {
    let prepared = decode_desktop_request(input)?;
    serde_json::to_string(&prepared.request).map_err(AdapterError::malformed_request_json)
}

pub fn decode_desktop_request(input: &str) -> Result<PreparedDesktopRequest, AdapterError> {
    let dto: DesktopRequestDto = serde_json::from_str(input).map_err(|error| {
        if error.to_string().contains("number out of range") {
            AdapterError::revalidation_failed(format!(
                "request numeric field must be finite and within binary64 range: {error}"
            ))
        } else {
            AdapterError::malformed_request_json(error)
        }
    })?;
    dto.prepare()
}

impl DesktopRequestDto {
    fn prepare(self) -> Result<PreparedDesktopRequest, AdapterError> {
        if self.version != ProtocolVersion::CURRENT.get() {
            return Err(AdapterError::native_api_version_mismatch(self.version));
        }
        if self.job_id.trim().is_empty() {
            return Err(AdapterError::revalidation_failed(
                "jobId must be a non-empty string",
            ));
        }
        if let Some(strategy_run_id) = &self.strategy_run_id {
            require_non_empty("strategyRunId", strategy_run_id)?;
        }
        if self.options.worker_mode != "irregular-convex-v2" {
            return Err(AdapterError::revalidation_failed(format!(
                "options.workerMode must be 'irregular-convex-v2', received {:?}",
                self.options.worker_mode
            )));
        }
        let history_mode = match self.options.history_mode.as_str() {
            "stream" => HistoryMode::Stream,
            "final" => HistoryMode::Final,
            "off" => HistoryMode::Off,
            other => {
                return Err(AdapterError::revalidation_failed(format!(
                    "options.historyMode must be one of 'stream' | 'final' | 'off', received {other:?}"
                )));
            }
        };
        let diagnostic_trace_mode = match self.options.diagnostic_trace_mode.as_str() {
            "full" => DiagnosticTraceMode::Full,
            "off" => DiagnosticTraceMode::Off,
            other => {
                return Err(AdapterError::revalidation_failed(format!(
                    "options.diagnosticTraceMode must be one of 'full' | 'off', received {other:?}"
                )));
            }
        };

        let request = EngineRequest {
            version: ProtocolVersion::CURRENT,
            timeout_ms: self.options.timeout_ms,
            profile: self
                .options
                .irregular_settings
                .optimizer
                .intrinsic_objective_profile_id
                .into(),
            sheet: self.sheet,
            pieces: self.pieces,
            source_pieces: self.source_pieces,
            settings: EngineSettings {
                padding: self.padding,
                sheet_edge_clearance_mm: self.sheet_edge_clearance_mm,
                allow_global_rotation: self.options.allow_global_rotation,
                allow_global_mirror: self.options.allow_global_mirror,
                geometry: self.options.irregular_settings.geometry,
                optimizer: self.options.irregular_settings.optimizer.settings,
            },
            history_mode,
            diagnostic_trace_mode,
        };
        request.validate().map_err(map_protocol_error)?;

        Ok(PreparedDesktopRequest {
            job_id: self.job_id,
            strategy_run_id: self.strategy_run_id,
            request,
        })
    }
}

fn require_non_empty(field: &str, value: &str) -> Result<(), AdapterError> {
    if value.is_empty() {
        return Err(AdapterError::revalidation_failed(format!(
            "{field} must be a non-empty string"
        )));
    }
    Ok(())
}

fn map_protocol_error(error: ProtocolError) -> AdapterError {
    match error {
        ProtocolError::UnsupportedVersion { received, .. } => {
            AdapterError::native_api_version_mismatch(received)
        }
        error => AdapterError::revalidation_failed(error.to_string()),
    }
}
