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
    pub rtsp_packet_timeout_secs: u64,
    pub cmaf_output_dir: String,
    pub cmaf_target_duration_secs: f64,
    pub cmaf_window_segments: usize,
    pub cmaf_part_duration_secs: f64,
    pub cmaf_ws_backlog_secs: f64,
    pub cmaf_write_files: bool,
    pub cmaf_fallback_fps: f64,
    pub http_bind: String,
    pub cf_access_enabled: bool,
    pub cf_access_jwks_url: Option<String>,
    pub cf_access_audience: Option<String>,
    pub cf_access_issuer: Option<String>,
    pub cf_access_jwks_cache_ttl_secs: u64,
    pub cf_access_dev_user_email: String,
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
        let mqtt_port = env_u16("MQTT_PORT").unwrap_or(if mqtt_tls { 8883 } else { 1883 });
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
        let rtsp_packet_timeout_secs = env_u64("RTSP_PACKET_TIMEOUT_SECS").unwrap_or(10);
        let cmaf_output_dir = env::var("CMAF_OUTPUT_DIR").unwrap_or_else(|_| "cmaf".to_string());
        let cmaf_target_duration_secs = env_f64("CMAF_TARGET_DURATION_SECS").unwrap_or(2.0);
        let cmaf_window_segments = env_usize("CMAF_WINDOW_SEGMENTS").unwrap_or(6);
        let cmaf_part_duration_secs = env_f64("CMAF_PART_DURATION_SECS").unwrap_or(0.333);
        let cmaf_ws_backlog_secs = env_f64("CMAF_WS_BACKLOG_SECS").unwrap_or(3.0);
        let cmaf_write_files = env_bool("CMAF_WRITE_FILES", false);
        let cmaf_fallback_fps = env_f64("CMAF_FALLBACK_FPS").unwrap_or(15.0);
        let http_bind = env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let cf_access_enabled = env_bool("CF_ACCESS_ENABLED", false);
        let cf_access_team_domain = env::var("CF_ACCESS_TEAM_DOMAIN").ok();
        let cf_access_jwks_url = env::var("CF_ACCESS_JWKS_URL").ok().or_else(|| {
            cf_access_team_domain
                .as_ref()
                .map(|domain| format!("https://{domain}/cdn-cgi/access/certs"))
        });
        let cf_access_audience = env::var("CF_ACCESS_AUD").ok();
        let cf_access_issuer = env::var("CF_ACCESS_ISSUER").ok().or_else(|| {
            cf_access_team_domain
                .as_ref()
                .map(|domain| format!("https://{domain}"))
        });
        let cf_access_jwks_cache_ttl_secs =
            env_u64("CF_ACCESS_JWKS_CACHE_TTL_SECS").unwrap_or(60 * 60);
        let cf_access_dev_user_email =
            env::var("CF_ACCESS_DEV_USER_EMAIL").unwrap_or_else(|_| "admin@local".to_string());

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
            rtsp_packet_timeout_secs,
            cmaf_output_dir,
            cmaf_target_duration_secs,
            cmaf_window_segments,
            cmaf_part_duration_secs,
            cmaf_ws_backlog_secs,
            cmaf_write_files,
            cmaf_fallback_fps,
            http_bind,
            cf_access_enabled,
            cf_access_jwks_url,
            cf_access_audience,
            cf_access_issuer,
            cf_access_jwks_cache_ttl_secs,
            cf_access_dev_user_email,
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
