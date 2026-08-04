use polygon_nesting_protocol::EngineInfo;

pub fn engine_info() -> EngineInfo {
    EngineInfo {
        name: "polygon-nesting".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    }
}
