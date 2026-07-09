//! `weather.*` — the keyless Open-Meteo current-conditions read verb.

mod current;
mod tool;

pub use current::{weather_current, WeatherCurrent, OPEN_METEO_BASE_ENV};
pub use tool::call_weather_tool;
