use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub printer_host: String,
    pub printer_serial: String,
    pub printer_access_code: String,
    pub mqtt_port: u16,
    pub mqtt_tls: bool,
    pub mqtt_tls_insecure: bool,
    pub mqtt_ca_cert: Option<String>,
    pub mqtt_max_incoming_packet_size: usize,
    pub mqtt_max_outgoing_packet_size: usize,
    pub mqtt_client_id: String,
    pub mqtt_keep_alive_secs: u64,
    pub mqtt_user_id: String,
    pub rtsp_url: Option<String>,
    pub rtsp_port: u16,
    pub rtsp_path: String,
    pub rtsp_username: String,
    pub rtsp_password: String,
    pub rtsp_tls_insecure: bool,
    pub hls_output_dir: String,
    pub hls_target_duration_secs: f64,
    pub hls_window_segments: usize,
    pub http_bind: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let printer_host = required_env("PRINTER_HOST")?;
        let printer_serial = required_env("PRINTER_SERIAL")?;
        let printer_access_code = required_env("PRINTER_ACCESS_CODE")?;
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
        let rtsp_url = env::var("RTSP_URL").ok();
        let rtsp_port = env_u16("RTSP_PORT").unwrap_or(322);
        let rtsp_path = env::var("RTSP_PATH").unwrap_or_else(|_| "/streaming/live/1".to_string());
        let rtsp_username = env::var("RTSP_USERNAME").unwrap_or_else(|_| "bblp".to_string());
        let rtsp_password =
            env::var("RTSP_PASSWORD").unwrap_or_else(|_| printer_access_code.clone());
        let rtsp_tls_insecure = env_bool("RTSP_TLS_INSECURE", true);
        let hls_output_dir = env::var("HLS_OUTPUT_DIR").unwrap_or_else(|_| "hls".to_string());
        let hls_target_duration_secs = env_f64("HLS_TARGET_DURATION_SECS").unwrap_or(2.0);
        let hls_window_segments = env_usize("HLS_WINDOW_SEGMENTS").unwrap_or(6);
        let http_bind = env::var("HTTP_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        Ok(Self {
            printer_host,
            printer_serial,
            printer_access_code,
            mqtt_port,
            mqtt_tls,
            mqtt_tls_insecure,
            mqtt_ca_cert,
            mqtt_max_incoming_packet_size,
            mqtt_max_outgoing_packet_size,
            mqtt_client_id,
            mqtt_keep_alive_secs,
            mqtt_user_id,
            rtsp_url,
            rtsp_port,
            rtsp_path,
            rtsp_username,
            rtsp_password,
            rtsp_tls_insecure,
            hls_output_dir,
            hls_target_duration_secs,
            hls_window_segments,
            http_bind,
        })
    }
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).map_err(|_| anyhow::anyhow!("missing required env var: {name}"))
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
