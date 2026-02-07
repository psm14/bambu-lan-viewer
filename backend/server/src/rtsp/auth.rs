use base64::{engine::general_purpose, Engine as _};
use rand::RngCore;

#[derive(Debug, Clone)]
pub struct RtspCredentials {
    pub username: String,
    pub password: String,
}

pub struct RtspAuthenticator {
    credentials: RtspCredentials,
    digest: Option<DigestChallenge>,
    nonce_count: u32,
    cnonce: String,
}

impl RtspAuthenticator {
    pub fn new(credentials: RtspCredentials) -> Self {
        let mut auth = Self {
            credentials,
            digest: None,
            nonce_count: 0,
            cnonce: String::new(),
        };
        auth.reset_nonce();
        auth
    }

    pub fn update_challenge(&mut self, header_value: &str) -> bool {
        let challenge = DigestChallenge::from_header(header_value);
        if challenge.is_none() {
            return false;
        }
        self.digest = challenge;
        self.nonce_count = 0;
        self.reset_nonce();
        true
    }

    pub fn authorization_header(&mut self, method: &str, uri: &str) -> String {
        if let Some(challenge) = self.digest.clone() {
            return self.digest_authorization(method, uri, challenge);
        }
        self.basic_authorization()
    }

    fn basic_authorization(&self) -> String {
        let raw = format!(
            "{}:{}",
            self.credentials.username, self.credentials.password
        );
        let encoded = general_purpose::STANDARD.encode(raw.as_bytes());
        format!("Authorization: Basic {}", encoded)
    }

    fn digest_authorization(
        &mut self,
        method: &str,
        uri: &str,
        challenge: DigestChallenge,
    ) -> String {
        let realm = challenge.realm;
        let nonce = challenge.nonce;
        let ha1 = md5_hex(&format!(
            "{}:{}:{}",
            self.credentials.username, realm, self.credentials.password
        ));
        let ha2 = md5_hex(&format!("{}:{}", method, uri));

        if let Some(qop) = challenge.qop.clone() {
            self.nonce_count = self.nonce_count.saturating_add(1);
            let nc_string = format!("{:08x}", self.nonce_count);
            let response = md5_hex(&format!(
                "{}:{}:{}:{}:{}:{}",
                ha1, nonce, nc_string, self.cnonce, qop, ha2
            ));
            let mut header = format!(
                "Authorization: Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", response=\"{}\", qop={}, nc={}, cnonce=\"{}\"",
                self.credentials.username, realm, nonce, uri, response, qop, nc_string, self.cnonce
            );
            if let Some(opaque) = challenge.opaque {
                header.push_str(&format!(", opaque=\"{}\"", opaque));
            }
            if let Some(algorithm) = challenge.algorithm {
                header.push_str(&format!(", algorithm={}", algorithm));
            }
            return header;
        }

        let response = md5_hex(&format!("{}:{}:{}", ha1, nonce, ha2));
        let mut header = format!(
            "Authorization: Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", response=\"{}\"",
            self.credentials.username, realm, nonce, uri, response
        );
        if let Some(opaque) = challenge.opaque {
            header.push_str(&format!(", opaque=\"{}\"", opaque));
        }
        if let Some(algorithm) = challenge.algorithm {
            header.push_str(&format!(", algorithm={}", algorithm));
        }
        header
    }

    fn reset_nonce(&mut self) {
        let mut bytes = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut bytes);
        self.cnonce = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    }
}

#[derive(Debug, Clone)]
struct DigestChallenge {
    realm: String,
    nonce: String,
    qop: Option<String>,
    algorithm: Option<String>,
    opaque: Option<String>,
}

impl DigestChallenge {
    fn from_header(header_value: &str) -> Option<Self> {
        let lower = header_value.to_ascii_lowercase();
        if !lower.starts_with("digest") {
            return None;
        }
        let params = header_value["digest".len()..].trim();
        let parameters = parse_parameters(params);
        let realm = parameters.get("realm")?.to_string();
        let nonce = parameters.get("nonce")?.to_string();
        let qop = parameters
            .get("qop")
            .and_then(|value| value.split(',').find(|item| item.trim() == "auth"))
            .map(|value| value.trim().to_string())
            .or_else(|| parameters.get("qop").map(|value| value.to_string()));
        let algorithm = parameters.get("algorithm").map(|value| value.to_string());
        let opaque = parameters.get("opaque").map(|value| value.to_string());

        Some(Self {
            realm,
            nonce,
            qop,
            algorithm,
            opaque,
        })
    }
}

fn parse_parameters(params: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for ch in params.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
            current.push(ch);
            continue;
        }
        if ch == ',' && !in_quotes {
            consume_param(&mut map, &mut current);
            continue;
        }
        current.push(ch);
    }
    consume_param(&mut map, &mut current);
    map
}

fn consume_param(map: &mut std::collections::HashMap<String, String>, buffer: &mut String) {
    let trimmed = buffer.trim().to_string();
    buffer.clear();
    if trimmed.is_empty() {
        return;
    }
    let mut parts = trimmed.splitn(2, '=');
    let key = parts.next().unwrap_or("").trim().to_ascii_lowercase();
    let mut value = parts.next().unwrap_or("").trim().to_string();
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        value = value[1..value.len() - 1].to_string();
    }
    if !key.is_empty() {
        map.insert(key, value);
    }
}

fn md5_hex(input: &str) -> String {
    let digest = md5::compute(input.as_bytes());
    digest.0.iter().map(|b| format!("{:02x}", b)).collect()
}
