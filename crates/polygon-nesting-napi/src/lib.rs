use napi_derive::napi;
use polygon_nesting_protocol::EngineInfo;

#[napi]
pub fn engine_info_json() -> napi::Result<String> {
    let info: EngineInfo = polygon_nesting_core::engine_info();
    serde_json::to_string(&info).map_err(|error| napi::Error::from_reason(error.to_string()))
}
