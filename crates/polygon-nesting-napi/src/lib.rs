pub mod compat;

mod diagnostics;

use napi_derive::napi;
use polygon_nesting_protocol::EngineInfo;

#[napi]
pub fn engine_info_json() -> napi::Result<String> {
    let info: EngineInfo = polygon_nesting_core::engine_info();
    serde_json::to_string(&info).map_err(|error| napi::Error::from_reason(error.to_string()))
}

#[napi(object)]
pub struct Capability {
    pub api_version: u32,
    pub crate_version: String,
    pub target_triple: String,
    pub profiles: Vec<String>,
}

#[napi]
pub fn native_capability() -> Capability {
    Capability {
        api_version: 3,
        crate_version: env!("CARGO_PKG_VERSION").to_string(),
        target_triple: env!("IRREGULAR_NATIVE_TARGET").to_string(),
        profiles: vec!["compact".to_string(), "compact-short-side".to_string()],
    }
}

#[napi]
pub fn get_last_job_diagnostics() -> String {
    diagnostics::last_job_diagnostics_json()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_capability_has_the_desktop_compatibility_shape() {
        let capability = native_capability();
        assert_eq!(capability.api_version, 3);
        assert_eq!(capability.crate_version, env!("CARGO_PKG_VERSION"));
        assert!(!capability.target_triple.is_empty());
        assert_eq!(
            capability.profiles,
            vec!["compact".to_string(), "compact-short-side".to_string()]
        );
    }

    #[test]
    fn diagnostics_export_returns_valid_json_or_null() {
        let json = get_last_job_diagnostics();
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("valid diagnostics JSON");
        assert!(parsed.is_null() || parsed.is_object());
    }
}
