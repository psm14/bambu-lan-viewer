use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrinterState {
    pub connected: bool,
    pub job_state: Option<String>,
    pub percent: Option<u8>,
    pub remaining_minutes: Option<u32>,
    pub nozzle_c: Option<f64>,
    pub bed_c: Option<f64>,
    pub chamber_c: Option<f64>,
    pub light: Option<String>,
    pub last_update: Option<DateTime<Utc>>,
}

impl PrinterState {
    pub fn apply_report(&mut self, report: &Value) {
        if let Some(state) = read_str(report.pointer("/print/gcode_state")) {
            self.job_state = Some(state.to_string());
        }

        if let Some(percent) = read_u8(
            report
                .pointer("/print/mc_percent")
                .or_else(|| report.pointer("/print/percent")),
        ) {
            self.percent = Some(percent);
        }

        if let Some(remaining) = read_u32(
            report
                .pointer("/print/mc_remaining_time")
                .or_else(|| report.pointer("/print/remain_time")),
        ) {
            self.remaining_minutes = Some(remaining);
        }

        if let Some(nozzle) = read_f64(
            report
                .pointer("/print/nozzle_temper")
                .or_else(|| report.pointer("/temp/nozzle_temper"))
                .or_else(|| report.pointer("/print/device/extruder/info/0/temp")),
        ) {
            self.nozzle_c = Some(nozzle);
        }

        if let Some(bed) = read_f64(
            report
                .pointer("/print/bed_temper")
                .or_else(|| report.pointer("/temp/bed_temper"))
                .or_else(|| report.pointer("/print/device/bed/info/temp")),
        ) {
            self.bed_c = Some(bed);
        }

        if let Some(chamber) = read_f64(
            report
                .pointer("/print/chamber_temper")
                .or_else(|| report.pointer("/temp/chamber_temper"))
                .or_else(|| report.pointer("/print/device/ctc/info/temp")),
        ) {
            self.chamber_c = Some(chamber);
        }

        if let Some(light) = extract_light(report) {
            self.light = Some(light);
        }

        self.last_update = Some(Utc::now());
    }
}

fn read_str(value: Option<&Value>) -> Option<&str> {
    value.and_then(|value| value.as_str())
}

fn read_u8(value: Option<&Value>) -> Option<u8> {
    match value? {
        Value::Number(number) => number.as_u64().and_then(|value| u8::try_from(value).ok()),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn read_u32(value: Option<&Value>) -> Option<u32> {
    match value? {
        Value::Number(number) => number.as_u64().and_then(|value| u32::try_from(value).ok()),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn read_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn extract_light(report: &Value) -> Option<String> {
    let lights_value = report
        .pointer("/print/lights_report")
        .or_else(|| report.pointer("/lights_report"));

    match lights_value? {
        Value::Array(entries) => {
            for entry in entries {
                let node = entry.get("node").and_then(Value::as_str);
                if node != Some("chamber_light") {
                    continue;
                }
                if let Some(mode) = entry.get("mode").and_then(Value::as_str) {
                    return Some(normalize_light_mode(mode));
                }
            }
            None
        }
        Value::Object(map) => map.get("chamber_light").and_then(|value| match value {
            Value::Number(number) => Some(
                if number.as_i64().unwrap_or(0) == 0 {
                    "off"
                } else {
                    "on"
                }
                .to_string(),
            ),
            Value::Bool(flag) => Some(if *flag { "on" } else { "off" }.to_string()),
            Value::String(text) => Some(normalize_light_mode(text)),
            _ => None,
        }),
        _ => None,
    }
}

fn normalize_light_mode(mode: &str) -> String {
    match mode {
        "on" | "off" => mode.to_string(),
        "flashing" => "on".to_string(),
        other => other.to_string(),
    }
}
