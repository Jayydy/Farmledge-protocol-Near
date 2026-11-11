use near_sdk::env;
use near_sdk::serde::Serialize;
use serde_json::json;

pub const NEP297_STANDARD: &str = "farmledge_receipt";
pub const NEP297_VERSION: &str = "3.0.0";

pub fn emit_event<T: Serialize>(event: &str, data: &T) {
    let payload = json!({
        "standard": NEP297_STANDARD,
        "version": NEP297_VERSION,
        "event": event,
        "data": [data]
    });
    env::log_str(&format!("EVENT_JSON:{}", payload));
}
