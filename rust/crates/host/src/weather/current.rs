//! `weather.current` — live conditions for one location from the free, keyless Open-Meteo feed.

use lb_mcp::ToolError;
use serde::Serialize;
use serde_json::Value;

/// Overridable in tests so a real local HTTP stub stands in for Open-Meteo (rule 9 — no mocks, a
/// true external behind one seam). Unset in production: the real Open-Meteo base is used.
pub const OPEN_METEO_BASE_ENV: &str = "LB_WEATHER_OPEN_METEO_BASE";
const DEFAULT_BASE: &str = "https://api.open-meteo.com";

#[derive(Debug, Clone, Serialize)]
pub struct WeatherCurrent {
    pub location: String,
    pub temp_c: f64,
    pub wind_kph: f64,
    pub code: u32,
    /// Observation time as a UTC epoch in SECONDS (we ask Open-Meteo for `timeformat=unixtime`, so this
    /// is an unambiguous instant, not a naive local string). The UI renders it in the VIEWER's browser
    /// timezone — a naive `2026-07-09T10:45` string can't be converted correctly, an epoch can.
    pub observed_ts: i64,
}

pub async fn weather_current(input: &Value) -> Result<WeatherCurrent, ToolError> {
    let lat = parse_coord(input, "lat")?;
    let lon = parse_coord(input, "lon")?;

    let base = std::env::var(OPEN_METEO_BASE_ENV).unwrap_or_else(|_| DEFAULT_BASE.to_string());
    // `timeformat=unixtime` → `current.time` is a UTC epoch (seconds), not a naive tz-less string, so the
    // UI can render it in the viewer's browser timezone.
    let url = format!(
        "{base}/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,wind_speed_10m,weather_code&timeformat=unixtime"
    );

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| ToolError::Extension(format!("weather fetch failed: {e}")))?
        .error_for_status()
        .map_err(|e| ToolError::Extension(format!("weather fetch failed: {e}")))?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| ToolError::Extension(format!("weather response: {e}")))?;

    let current = body
        .get("current")
        .ok_or_else(|| ToolError::Extension("weather response: missing `current`".into()))?;
    let temp_c = current
        .get("temperature_2m")
        .and_then(Value::as_f64)
        .ok_or_else(|| ToolError::Extension("weather response: missing temperature_2m".into()))?;
    let wind_kph = current
        .get("wind_speed_10m")
        .and_then(Value::as_f64)
        .ok_or_else(|| ToolError::Extension("weather response: missing wind_speed_10m".into()))?;
    let code = current
        .get("weather_code")
        .and_then(Value::as_u64)
        .ok_or_else(|| ToolError::Extension("weather response: missing weather_code".into()))?
        as u32;
    let observed_ts = current
        .get("time")
        .and_then(Value::as_i64)
        .ok_or_else(|| ToolError::Extension("weather response: missing time".into()))?;

    Ok(WeatherCurrent {
        location: format!("{lat},{lon}"),
        temp_c,
        wind_kph,
        code,
        observed_ts,
    })
}

fn parse_coord(input: &Value, key: &str) -> Result<f64, ToolError> {
    input
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}
