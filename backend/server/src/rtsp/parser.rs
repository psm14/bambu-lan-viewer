use std::collections::HashMap;

#[derive(Debug)]
pub enum RtspEvent {
    Response(RtspResponse),
    Interleaved { channel: u8, payload: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct RtspResponse {
    pub status_code: u16,
    pub reason_phrase: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

impl RtspResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(&name.to_ascii_lowercase())
            .map(|value| value.as_str())
    }

    pub fn cseq(&self) -> Option<u32> {
        self.header("cseq").and_then(|value| value.parse().ok())
    }
}

pub struct RtspStreamParser {
    buffer: Vec<u8>,
}

impl RtspStreamParser {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn append(&mut self, data: &[u8]) -> Vec<RtspEvent> {
        self.buffer.extend_from_slice(data);
        let mut events = Vec::new();

        loop {
            if self.buffer.is_empty() {
                break;
            }

            if self.buffer[0] == 0x24 {
                if let Some(event) = self.extract_interleaved() {
                    events.push(event);
                    continue;
                }
                break;
            }

            if let Some(event) = self.extract_response() {
                events.push(RtspEvent::Response(event));
                continue;
            }

            break;
        }

        events
    }

    fn extract_interleaved(&mut self) -> Option<RtspEvent> {
        if self.buffer.len() < 4 || self.buffer[0] != 0x24 {
            return None;
        }

        let channel = self.buffer[1];
        let length = ((self.buffer[2] as usize) << 8) | (self.buffer[3] as usize);
        let total = 4 + length;
        if self.buffer.len() < total {
            return None;
        }

        let payload = self.buffer[4..total].to_vec();
        self.buffer.drain(0..total);
        Some(RtspEvent::Interleaved { channel, payload })
    }

    fn extract_response(&mut self) -> Option<RtspResponse> {
        let header_end = find_double_crlf(&self.buffer)?;
        let header_bytes = &self.buffer[..header_end];
        let header_text = String::from_utf8_lossy(header_bytes);
        let mut lines = header_text.split("\r\n").filter(|line| !line.is_empty());
        let status_line = lines.next()?;
        let mut status_parts = status_line.splitn(3, ' ');
        let _proto = status_parts.next()?;
        let status_code = status_parts.next()?.parse().ok()?;
        let reason_phrase = status_parts.next().unwrap_or("").to_string();

        let mut headers = HashMap::new();
        for line in lines {
            let mut parts = line.splitn(2, ':');
            let key = parts.next()?.trim().to_ascii_lowercase();
            let value = parts.next().unwrap_or("").trim().to_string();
            headers.insert(key, value);
        }

        let content_length = headers
            .get("content-length")
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(0);

        let total_length = header_end + 4 + content_length;
        if self.buffer.len() < total_length {
            return None;
        }

        let body_start = header_end + 4;
        let body = self.buffer[body_start..total_length].to_vec();
        self.buffer.drain(0..total_length);

        Some(RtspResponse {
            status_code,
            reason_phrase,
            headers,
            body,
        })
    }
}

fn find_double_crlf(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}
