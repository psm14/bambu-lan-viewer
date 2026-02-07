use serde::Deserialize;
use serde_json::{json, Value};

const MAX_MOVE_MM: f64 = 50.0;
const MAX_EXTRUDE_MM: f64 = 50.0;
const MIN_FEED_RATE: u32 = 60;
const MAX_FEED_RATE: u32 = 12000;
const NOZZLE_TEMP_MIN_C: f64 = 0.0;
const NOZZLE_TEMP_MAX_C: f64 = 320.0;
const BED_TEMP_MIN_C: f64 = 0.0;
const BED_TEMP_MAX_C: f64 = 120.0;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MotionAxis {
    X,
    Y,
    Z,
}

impl MotionAxis {
    fn letter(self) -> char {
        match self {
            MotionAxis::X => 'X',
            MotionAxis::Y => 'Y',
            MotionAxis::Z => 'Z',
        }
    }

    fn default_feed_rate(self) -> u32 {
        match self {
            MotionAxis::X | MotionAxis::Y => 3000,
            MotionAxis::Z => 600,
        }
    }
}

#[derive(Clone, Debug)]
pub enum CommandRequest {
    Pause,
    Resume,
    Stop,
    Light {
        on: bool,
    },
    Home,
    Move {
        axis: MotionAxis,
        distance: f64,
        feed_rate: Option<u32>,
    },
    SetNozzleTemp {
        target_c: f64,
    },
    SetBedTemp {
        target_c: f64,
    },
    Extrude {
        amount_mm: f64,
        feed_rate: Option<u32>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandPayload {
    Pause,
    Resume,
    Stop,
    Light {
        on: bool,
    },
    Home,
    Move {
        axis: MotionAxis,
        distance: f64,
        feed_rate: Option<u32>,
    },
    SetNozzleTemp {
        target_c: f64,
    },
    SetBedTemp {
        target_c: f64,
    },
    Extrude {
        amount_mm: f64,
        feed_rate: Option<u32>,
    },
}

impl From<CommandPayload> for CommandRequest {
    fn from(payload: CommandPayload) -> Self {
        match payload {
            CommandPayload::Pause => CommandRequest::Pause,
            CommandPayload::Resume => CommandRequest::Resume,
            CommandPayload::Stop => CommandRequest::Stop,
            CommandPayload::Light { on } => CommandRequest::Light { on },
            CommandPayload::Home => CommandRequest::Home,
            CommandPayload::Move {
                axis,
                distance,
                feed_rate,
            } => CommandRequest::Move {
                axis,
                distance,
                feed_rate,
            },
            CommandPayload::SetNozzleTemp { target_c } => {
                CommandRequest::SetNozzleTemp { target_c }
            }
            CommandPayload::SetBedTemp { target_c } => CommandRequest::SetBedTemp { target_c },
            CommandPayload::Extrude {
                amount_mm,
                feed_rate,
            } => CommandRequest::Extrude {
                amount_mm,
                feed_rate,
            },
        }
    }
}

impl CommandRequest {
    pub fn to_payload(&self, user_id: &str, sequence_id: u64) -> Value {
        let sequence_id = sequence_id.to_string();
        match self {
            CommandRequest::Pause => json!({
                "user_id": user_id,
                "print": {
                    "sequence_id": sequence_id,
                    "command": "pause"
                }
            }),
            CommandRequest::Resume => json!({
                "user_id": user_id,
                "print": {
                    "sequence_id": sequence_id,
                    "command": "resume"
                }
            }),
            CommandRequest::Stop => json!({
                "user_id": user_id,
                "print": {
                    "sequence_id": sequence_id,
                    "command": "stop"
                }
            }),
            CommandRequest::Light { on } => json!({
                "user_id": user_id,
                "system": {
                    "sequence_id": sequence_id,
                    "command": "ledctrl",
                    "led_node": "chamber_light",
                    "led_mode": if *on { "on" } else { "off" },
                    "led_on_time": 500,
                    "led_off_time": 500,
                    "loop_times": 0,
                    "interval_time": 0
                }
            }),
            CommandRequest::Home => json!({
                "user_id": user_id,
                "print": {
                    "sequence_id": sequence_id,
                    "command": "gcode_line",
                    "param": "G28 \n"
                }
            }),
            CommandRequest::Move {
                axis,
                distance,
                feed_rate,
            } => {
                let distance = sanitize_distance(*distance);
                let feed_rate = sanitize_feed_rate(feed_rate.unwrap_or(axis.default_feed_rate()));
                let gcode = motion_gcode(*axis, distance, feed_rate);
                json!({
                    "user_id": user_id,
                    "print": {
                        "sequence_id": sequence_id,
                        "command": "gcode_line",
                        "param": gcode
                    }
                })
            }
            CommandRequest::SetNozzleTemp { target_c } => {
                let sanitized =
                    sanitize_temperature(*target_c, NOZZLE_TEMP_MIN_C, NOZZLE_TEMP_MAX_C);
                let gcode = format!("M104 S{}\n", format_gcode_number(sanitized));
                json!({
                    "user_id": user_id,
                    "print": {
                        "sequence_id": sequence_id,
                        "command": "gcode_line",
                        "param": gcode
                    }
                })
            }
            CommandRequest::SetBedTemp { target_c } => {
                let sanitized = sanitize_temperature(*target_c, BED_TEMP_MIN_C, BED_TEMP_MAX_C);
                let gcode = format!("M140 S{}\n", format_gcode_number(sanitized));
                json!({
                    "user_id": user_id,
                    "print": {
                        "sequence_id": sequence_id,
                        "command": "gcode_line",
                        "param": gcode
                    }
                })
            }
            CommandRequest::Extrude {
                amount_mm,
                feed_rate,
            } => {
                let amount_mm = sanitize_extrude_amount(*amount_mm);
                let feed_rate = sanitize_feed_rate(feed_rate.unwrap_or(180));
                let gcode = extrude_gcode(amount_mm, feed_rate);
                json!({
                    "user_id": user_id,
                    "print": {
                        "sequence_id": sequence_id,
                        "command": "gcode_line",
                        "param": gcode
                    }
                })
            }
        }
    }
}

fn motion_gcode(axis: MotionAxis, distance: f64, feed_rate: u32) -> String {
    format!(
        "M211 X0 Y0 Z0 \nM211 S\nM1002 push_ref_mode\nG91\nG1 {}{} F{}\nM1002 pop_ref_mode\n",
        axis.letter(),
        format_gcode_number(distance),
        feed_rate
    )
}

fn sanitize_distance(distance: f64) -> f64 {
    if !distance.is_finite() {
        return 0.0;
    }
    distance.clamp(-MAX_MOVE_MM, MAX_MOVE_MM)
}

fn sanitize_feed_rate(feed_rate: u32) -> u32 {
    feed_rate.clamp(MIN_FEED_RATE, MAX_FEED_RATE)
}

fn sanitize_temperature(target_c: f64, min_c: f64, max_c: f64) -> f64 {
    if !target_c.is_finite() {
        return min_c;
    }
    target_c.clamp(min_c, max_c).round()
}

fn sanitize_extrude_amount(amount_mm: f64) -> f64 {
    if !amount_mm.is_finite() {
        return 0.0;
    }
    amount_mm.clamp(-MAX_EXTRUDE_MM, MAX_EXTRUDE_MM)
}

fn extrude_gcode(amount_mm: f64, feed_rate: u32) -> String {
    format!(
        "M83\nG1 E{} F{}\n",
        format_gcode_number(amount_mm),
        feed_rate
    )
}

fn format_gcode_number(value: f64) -> String {
    let mut rendered = format!("{value:.3}");
    while rendered.ends_with('0') {
        rendered.pop();
    }
    if rendered.ends_with('.') {
        rendered.pop();
    }
    if rendered == "-0" {
        "0".to_string()
    } else {
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_payload_uses_g28_gcode_line() {
        let payload = CommandRequest::Home.to_payload("1", 7);
        assert_eq!(payload["user_id"], "1");
        assert_eq!(payload["print"]["sequence_id"], "7");
        assert_eq!(payload["print"]["command"], "gcode_line");
        assert_eq!(payload["print"]["param"], "G28 \n");
    }

    #[test]
    fn move_payload_wraps_relative_axis_move() {
        let payload = CommandRequest::Move {
            axis: MotionAxis::X,
            distance: 5.0,
            feed_rate: Some(3000),
        }
        .to_payload("1", 9);
        let gcode = payload["print"]["param"].as_str().unwrap_or("");

        assert_eq!(payload["print"]["command"], "gcode_line");
        assert!(gcode.contains("M1002 push_ref_mode"));
        assert!(gcode.contains("G91"));
        assert!(gcode.contains("G1 X5 F3000"));
        assert!(gcode.contains("M1002 pop_ref_mode"));
    }

    #[test]
    fn move_payload_clamps_out_of_range_values() {
        let payload = CommandRequest::Move {
            axis: MotionAxis::Z,
            distance: 1000.0,
            feed_rate: Some(1),
        }
        .to_payload("1", 9);
        let gcode = payload["print"]["param"].as_str().unwrap_or("");

        assert!(gcode.contains("G1 Z50 F60"));
    }

    #[test]
    fn set_nozzle_temp_uses_m104_with_clamping() {
        let payload = CommandRequest::SetNozzleTemp { target_c: 999.0 }.to_payload("1", 10);
        let gcode = payload["print"]["param"].as_str().unwrap_or("");

        assert_eq!(payload["print"]["command"], "gcode_line");
        assert_eq!(gcode, "M104 S320\n");
    }

    #[test]
    fn set_bed_temp_uses_m140_with_clamping() {
        let payload = CommandRequest::SetBedTemp { target_c: -5.0 }.to_payload("1", 11);
        let gcode = payload["print"]["param"].as_str().unwrap_or("");

        assert_eq!(payload["print"]["command"], "gcode_line");
        assert_eq!(gcode, "M140 S0\n");
    }

    #[test]
    fn extrude_uses_relative_extrusion_gcode() {
        let payload = CommandRequest::Extrude {
            amount_mm: 5.0,
            feed_rate: Some(240),
        }
        .to_payload("1", 12);
        let gcode = payload["print"]["param"].as_str().unwrap_or("");

        assert_eq!(payload["print"]["command"], "gcode_line");
        assert_eq!(gcode, "M83\nG1 E5 F240\n");
    }
}
