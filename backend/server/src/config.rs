use serde::{Deserialize, Serialize};
use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database_url: String,
    pub mqtt_port: u16,
    pub mqtt_tls: bool,
    pub mqtt_tls_insecure: bool,
    pub mqtt_ca_cert: Option<String>,
    pub mqtt_max_incoming_packet_size: usize,
    pub mqtt_max_outgoing_packet_size: usize,
    pub mqtt_client_id: String,
    pub mqtt_keep_alive_secs: u64,
    pub mqtt_user_id: String,
    pub rtsp_tls_insecure: bool,
    pub hls_output_dir: String,
    pub hls_target_duration_secs: f64,
    pub hls_window_segments: usize,
    pub hls_low_latency: bool,
    pub hls_part_duration_secs: f64,
    pub http_bind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterConfig {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub serial: String,
    pub access_code: String,
    pub rtsp_url: Option<String>,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL")
            .or_else(|_| env::var("DB_PATH"))
            .unwrap_or_else(|_| "data/printers.db".to_string());
        let database_url = normalize_db_url(&database_url);
        let mqtt_tls = env_bool("MQTT_TLS", true);
        let mqtt_port = env_u16("MQTT_PORT").unwrap_or_else(|| if mqtt_tls { 8883 } else { 1883 });
        let mqtt_ca_cert = env::var("MQTT_CA_CERT").ok();
        let mqtt_tls_insecure = env_bool("MQTT_TLS_INSECURE", mqtt_ca_cert.is_none());
        let mqtt_max_incoming_packet_size =
            env_usize("MQTT_MAX_INCOMING_PACKET_SIZE").unwrap_or(256 * 1024);
        let mqtt_max_outgoing_packet_size =
            env_usize("MQTT_MAX_OUTGOING_PACKET_SIZE").unwrap_or(64 * 1024);
        let mqtt_client_id =
            env::var("MQTT_CLIENT_ID").unwrap_or_else(|_| "bambu-lan-viewer".to_string());
        let mqtt_keep_alive_secs = env_u64("MQTT_KEEP_ALIVE_SECS").unwrap_or(30);
        let mqtt_user_id = env::var("MQTT_USER_ID").unwrap_or_else(|_| "1".to_string());
        let rtsp_tls_insecure = env_bool("RTSP_TLS_INSECURE", true);
        let hls_output_dir = env::var("HLS_OUTPUT_DIR").unwrap_or_else(|_| "hls".to_string());
        let hls_target_duration_secs = env_f64("HLS_TARGET_DURATION_SECS").unwrap_or(2.0);
        let hls_window_segments = env_usize("HLS_WINDOW_SEGMENTS").unwrap_or(6);
        let hls_low_latency = env_bool("HLS_LOW_LATENCY", true);
        let hls_part_duration_secs = env_f64("HLS_PART_DURATION_SECS").unwrap_or(0.333);
        let http_bind = env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        Ok(Self {
            database_url,
            mqtt_port,
            mqtt_tls,
            mqtt_tls_insecure,
            mqtt_ca_cert,
            mqtt_max_incoming_packet_size,
            mqtt_max_outgoing_packet_size,
            mqtt_client_id,
            mqtt_keep_alive_secs,
            mqtt_user_id,
            rtsp_tls_insecure,
            hls_output_dir,
            hls_target_duration_secs,
            hls_window_segments,
            hls_low_latency,
            hls_part_duration_secs,
            http_bind,
        })
    }
}

fn normalize_db_url(value: &str) -> String {
    if value.starts_with("sqlite:") {
        value.to_string()
    } else {
        format!("sqlite://{value}")
    }
}

fn env_bool(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => default,
    }
}

fn env_u16(name: &str) -> Option<u16> {
    env::var(name).ok().and_then(|value| value.parse().ok())
}

fn env_u64(name: &str) -> Option<u64> {
    env::var(name).ok().and_then(|value| value.parse().ok())
}

fn env_usize(name: &str) -> Option<usize> {
    env::var(name).ok().and_then(|value| value.parse().ok())
}

fn env_f64(name: &str) -> Option<f64> {
    env::var(name).ok().and_then(|value| value.parse().ok())
}
