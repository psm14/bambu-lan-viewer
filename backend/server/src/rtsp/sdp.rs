use base64::{engine::general_purpose, Engine as _};
use url::Url;

#[derive(Debug, Clone)]
pub struct SdpInfo {
    pub video_control: Option<String>,
    pub session_control: Option<String>,
    pub payload_type: Option<u8>,
    pub sps: Option<Vec<u8>>,
    pub pps: Option<Vec<u8>>,
}

impl SdpInfo {
    pub fn resolved_video_control_url(&self, base_url: &Url) -> String {
        if let Some(control) = self.video_control.as_ref() {
            return resolve_control(control, base_url);
        }
        base_url.to_string()
    }

    pub fn resolved_play_url(&self, base_url: &Url) -> String {
        if let Some(control) = self.session_control.as_ref() {
            if control != "*" {
                return resolve_control(control, base_url);
            }
        }
        base_url.to_string()
    }
}

pub fn parse_sdp(body: &[u8]) -> Option<SdpInfo> {
    let text = String::from_utf8_lossy(body);
    let mut session_control = None;
    let mut video_control = None;
    let mut payload_type = None;
    let mut sps = None;
    let mut pps = None;
    let mut in_video = false;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("m=") {
            in_video = line.to_ascii_lowercase().starts_with("m=video");
            if in_video {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    payload_type = parts[3].parse::<u8>().ok();
                }
            }
            continue;
        }

        if line.starts_with("a=control:") {
            let value = line.trim_start_matches("a=control:").trim().to_string();
            if in_video {
                video_control = Some(value);
            } else {
                session_control = Some(value);
            }
            continue;
        }

        if in_video && line.starts_with("a=rtpmap:") {
            let value = line.trim_start_matches("a=rtpmap:");
            let mut parts = value.split_whitespace();
            if let (Some(pt), Some(codec)) = (parts.next(), parts.next()) {
                if codec.to_ascii_uppercase().starts_with("H264") {
                    payload_type = pt.parse::<u8>().ok();
                }
            }
            continue;
        }

        if in_video && line.starts_with("a=fmtp:") {
            let value = line.trim_start_matches("a=fmtp:");
            let mut parts = value.splitn(2, ' ');
            let _pt = parts.next();
            let params = match parts.next() {
                Some(params) => params,
                None => continue,
            };
            for param in params.split(';') {
                let mut kv = param.splitn(2, '=');
                let key = kv.next().unwrap_or("").trim();
                let val = kv.next().unwrap_or("").trim();
                if key == "sprop-parameter-sets" {
                    let mut sets = val.split(',');
                    if let Some(sps_b64) = sets.next() {
                        sps = general_purpose::STANDARD.decode(sps_b64).ok();
                    }
                    if let Some(pps_b64) = sets.next() {
                        pps = general_purpose::STANDARD.decode(pps_b64).ok();
                    }
                }
            }
        }
    }

    Some(SdpInfo {
        video_control,
        session_control,
        payload_type,
        sps,
        pps,
    })
}

fn resolve_control(control: &str, base_url: &Url) -> String {
    let lower = control.to_ascii_lowercase();
    if lower.starts_with("rtsp://") || lower.starts_with("rtsps://") {
        return control.to_string();
    }
    if control == "*" {
        return base_url.to_string();
    }
    base_url
        .join(control)
        .map(|url| url.to_string())
        .unwrap_or_else(|_| base_url.to_string())
}
