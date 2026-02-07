use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AmsUnitState {
    pub id: Option<u8>,
    pub humidity_raw: Option<u8>,
    #[serde(default)]
    pub trays: Vec<AmsTrayState>,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AmsTrayState {
    pub id: Option<u8>,
    pub filament_type: Option<String>,
    pub color: Option<String>,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PrinterState {
    pub connected: bool,
    pub job_state: Option<String>,
    pub percent: Option<u8>,
    pub layer_num: Option<u32>,
    pub total_layer_num: Option<u32>,
    pub remaining_minutes: Option<u32>,
    pub nozzle_c: Option<f64>,
    pub nozzle_target_c: Option<f64>,
    pub bed_c: Option<f64>,
    pub bed_target_c: Option<f64>,
    pub chamber_c: Option<f64>,
    pub light: Option<String>,
    pub rtsp_url: Option<String>,
    #[serde(default)]
    pub ams: Vec<AmsUnitState>,
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

        if let Some(layer_num) = read_u32(report.pointer("/print/layer_num")) {
            self.layer_num = Some(layer_num);
        }

        if let Some(total_layer_num) = read_u32(report.pointer("/print/total_layer_num")) {
            self.total_layer_num = Some(total_layer_num);
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

        if let Some(nozzle_target) = read_f64(
            report
                .pointer("/print/nozzle_target_temper")
                .or_else(|| report.pointer("/temp/nozzle_target_temper"))
                .or_else(|| report.pointer("/print/device/extruder/info/0/htar")),
        ) {
            self.nozzle_target_c = Some(nozzle_target);
        }

        if let Some(bed) = read_f64(
            report
                .pointer("/print/bed_temper")
                .or_else(|| report.pointer("/temp/bed_temper"))
                .or_else(|| report.pointer("/print/device/bed/info/temp")),
        ) {
            self.bed_c = Some(bed);
        }

        if let Some(bed_target) = read_f64(
            report
                .pointer("/print/bed_target_temper")
                .or_else(|| report.pointer("/temp/bed_target_temper")),
        ) {
            self.bed_target_c = Some(bed_target);
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

        if let Some(rtsp_url) = read_str(
            report
                .pointer("/print/ipcam/rtsp_url")
                .or_else(|| report.pointer("/ipcam/rtsp_url")),
        ) {
            if !rtsp_url.is_empty() {
                self.rtsp_url = Some(rtsp_url.to_string());
            }
        }

        if let Some(ams) = extract_ams(report) {
            self.ams = ams;
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

fn extract_ams(report: &Value) -> Option<Vec<AmsUnitState>> {
    let ams_value = report
        .pointer("/print/ams/ams")
        .or_else(|| report.pointer("/ams/ams"))?;
    let units = ams_value.as_array()?;

    let parsed = units
        .iter()
        .enumerate()
        .filter_map(|(index, unit)| {
            let unit = unit.as_object()?;
            let id = read_u8(unit.get("id")).or_else(|| u8::try_from(index + 1).ok());
            let humidity_raw = read_u8(unit.get("humidity_raw"));
            let trays = extract_ams_trays(unit.get("tray"));

            Some(AmsUnitState {
                id,
                humidity_raw,
                trays,
            })
        })
        .collect();

    Some(parsed)
}

fn extract_ams_trays(value: Option<&Value>) -> Vec<AmsTrayState> {
    let Some(trays) = value.and_then(Value::as_array) else {
        return Vec::new();
    };

    trays
        .iter()
        .enumerate()
        .filter_map(|(index, tray)| {
            let tray = tray.as_object()?;
            let id = read_u8(tray.get("id")).or_else(|| u8::try_from(index).ok());
            let filament_type = read_str(tray.get("tray_type")).and_then(non_empty_text);
            let color = extract_tray_color(tray);

            if id.is_none() && filament_type.is_none() && color.is_none() {
                return None;
            }

            Some(AmsTrayState {
                id,
                filament_type,
                color,
            })
        })
        .collect()
}

fn extract_tray_color(tray: &serde_json::Map<String, Value>) -> Option<String> {
    if let Some(color) = read_str(tray.get("tray_color")) {
        return non_empty_text(color);
    }

    let color = tray
        .get("cols")
        .and_then(Value::as_array)
        .and_then(|colors| colors.first())
        .and_then(Value::as_str)?;
    non_empty_text(color)
}

fn non_empty_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn apply_report_parses_ams_humidity_and_filament_data() {
        let report = json!({
            "print": {
                "ams": {
                    "ams": [
                        {
                            "id": "1",
                            "humidity_raw": "25",
                            "tray": [
                                {
                                    "id": "0",
                                    "tray_type": "PLA",
                                    "tray_color": "FFFFFFFF"
                                },
                                {
                                    "id": "1",
                                    "tray_type": "PETG",
                                    "cols": ["161616FF"]
                                },
                                {
                                    "id": "2",
                                    "state": 0
                                }
                            ]
                        }
                    ]
                }
            }
        });

        let mut state = PrinterState::default();
        state.apply_report(&report);

        assert_eq!(state.ams.len(), 1);
        assert_eq!(state.ams[0].id, Some(1));
        assert_eq!(state.ams[0].humidity_raw, Some(25));
        assert_eq!(state.ams[0].trays.len(), 3);
        assert_eq!(state.ams[0].trays[0].filament_type.as_deref(), Some("PLA"));
        assert_eq!(state.ams[0].trays[0].color.as_deref(), Some("FFFFFFFF"));
        assert_eq!(state.ams[0].trays[1].filament_type.as_deref(), Some("PETG"));
        assert_eq!(state.ams[0].trays[1].color.as_deref(), Some("161616FF"));
        assert_eq!(state.ams[0].trays[2].filament_type, None);
        assert_eq!(state.ams[0].trays[2].color, None);
    }

    #[test]
    fn apply_report_parses_root_ams_payload() {
        let report = json!({
            "ams": {
                "ams": [
                    {
                        "humidity_raw": "42",
                        "tray": [
                            {
                                "tray_type": "ABS",
                                "tray_color": "ABCDEF12"
                            }
                        ]
                    }
                ]
            }
        });

        let mut state = PrinterState::default();
        state.apply_report(&report);

        assert_eq!(state.ams.len(), 1);
        assert_eq!(state.ams[0].id, Some(1));
        assert_eq!(state.ams[0].humidity_raw, Some(42));
        assert_eq!(state.ams[0].trays.len(), 1);
        assert_eq!(state.ams[0].trays[0].id, Some(0));
        assert_eq!(state.ams[0].trays[0].filament_type.as_deref(), Some("ABS"));
        assert_eq!(state.ams[0].trays[0].color.as_deref(), Some("ABCDEF12"));
    }

    #[test]
    fn apply_report_parses_target_temperatures() {
        let report = json!({
            "print": {
                "nozzle_target_temper": 220.0,
                "bed_target_temper": "65"
            }
        });

        let mut state = PrinterState::default();
        state.apply_report(&report);

        assert_eq!(state.nozzle_target_c, Some(220.0));
        assert_eq!(state.bed_target_c, Some(65.0));
    }
}
