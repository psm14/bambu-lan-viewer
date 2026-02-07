use crate::rtsp::auth::{RtspAuthenticator, RtspCredentials};
use crate::rtsp::parser::{RtspEvent, RtspResponse, RtspStreamParser};
use crate::rtsp::sdp::{parse_sdp, SdpInfo};
use crate::tls;
use anyhow::Context;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::sleep;
use tokio_rustls::TlsConnector;
use tracing::info;
use url::Url;

#[derive(Debug)]
pub struct InterleavedPacket {
    pub channel: u8,
    pub payload: Vec<u8>,
}

pub struct RtspSession {
    pub sdp: SdpInfo,
    pub rtp_channel: u8,
    pub interleaved_rx: mpsc::Receiver<InterleavedPacket>,
    _connection: Arc<RtspConnection>,
}

pub struct RtspClient {
    url: Url,
    credentials: Option<RtspCredentials>,
    tls_insecure: bool,
}

impl RtspClient {
    pub fn new(url: Url, credentials: Option<RtspCredentials>, tls_insecure: bool) -> Self {
        Self {
            url,
            credentials,
            tls_insecure,
        }
    }

    pub async fn start(self) -> anyhow::Result<RtspSession> {
        let (connection, interleaved_rx) =
            RtspConnection::connect(&self.url, self.credentials, self.tls_insecure).await?;

        let describe = connection
            .send_request_with_retry(
                "DESCRIBE",
                self.url.as_str(),
                [("Accept".to_string(), "application/sdp".to_string())]
                    .into_iter()
                    .collect(),
            )
            .await?;
        if describe.status_code != 200 {
            anyhow::bail!(
                "RTSP DESCRIBE failed: {} {}",
                describe.status_code,
                describe.reason_phrase
            );
        }

        let sdp = parse_sdp(&describe.body).ok_or_else(|| anyhow::anyhow!("invalid SDP"))?;
        let base_url = describe
            .header("content-base")
            .or_else(|| describe.header("content-location"))
            .and_then(|value| normalize_base_url(value, &self.url))
            .unwrap_or_else(|| self.url.clone());
        let setup_uri = sdp.resolved_video_control_url(&base_url);
        let setup = connection
            .send_request_with_retry(
                "SETUP",
                &setup_uri,
                [(
                    "Transport".to_string(),
                    "RTP/AVP/TCP;unicast;interleaved=0-1".to_string(),
                )]
                .into_iter()
                .collect(),
            )
            .await?;
        if setup.status_code != 200 {
            anyhow::bail!(
                "RTSP SETUP failed: {} {}",
                setup.status_code,
                setup.reason_phrase
            );
        }
        let (rtp_channel, _rtcp_channel) = parse_interleaved_channels(&setup).unwrap_or((0, 1));

        let play_uri = sdp.resolved_play_url(&base_url);
        info!(
            rtsp_base = %base_url,
            video_control = ?sdp.video_control,
            session_control = ?sdp.session_control,
            setup_uri = %setup_uri,
            play_uri = %play_uri,
            "rtsp control urls"
        );
        let play = connection
            .send_request_with_retry(
                "PLAY",
                &play_uri,
                [("Range".to_string(), "npt=0-".to_string())]
                    .into_iter()
                    .collect(),
            )
            .await?;
        if play.status_code != 200 {
            anyhow::bail!(
                "RTSP PLAY failed: {} {}",
                play.status_code,
                play.reason_phrase
            );
        }

        connection.start_keepalive(play_uri).await;

        Ok(RtspSession {
            sdp,
            rtp_channel,
            interleaved_rx,
            _connection: connection,
        })
    }
}

struct RtspConnection {
    writer: Mutex<WriteHalf<BoxedStream>>,
    pending: Mutex<HashMap<u32, oneshot::Sender<RtspResponse>>>,
    authenticator: Mutex<Option<RtspAuthenticator>>,
    session_id: Mutex<Option<String>>,
    session_timeout: Mutex<Option<Duration>>,
    cseq: Mutex<u32>,
}

trait RtspStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T> RtspStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

type BoxedStream = Box<dyn RtspStream>;

impl RtspConnection {
    async fn connect(
        url: &Url,
        credentials: Option<RtspCredentials>,
        tls_insecure: bool,
    ) -> anyhow::Result<(Arc<Self>, mpsc::Receiver<InterleavedPacket>)> {
        let host = url.host_str().unwrap_or("");
        let port = url.port().unwrap_or(322);
        let stream = TcpStream::connect((host, port))
            .await
            .context("rtsp connect")?;

        let stream: BoxedStream = if url.scheme().eq_ignore_ascii_case("rtsps") {
            let tls_config = if tls_insecure {
                tls::insecure_client_config()
            } else {
                rustls::ClientConfig::builder()
                    .with_safe_defaults()
                    .with_root_certificates(rustls::RootCertStore::empty())
                    .with_no_client_auth()
            };
            let connector = TlsConnector::from(Arc::new(tls_config));
            let server_name = rustls::ServerName::try_from(host)
                .map_err(|_| anyhow::anyhow!("invalid server name"))?;
            let tls_stream = connector.connect(server_name, stream).await?;
            Box::new(tls_stream)
        } else {
            Box::new(stream)
        };

        let (reader, writer) = tokio::io::split(stream);
        let (interleaved_tx, interleaved_rx) = mpsc::channel(64);

        let connection = Arc::new(Self {
            writer: Mutex::new(writer),
            pending: Mutex::new(HashMap::new()),
            authenticator: Mutex::new(credentials.map(RtspAuthenticator::new)),
            session_id: Mutex::new(None),
            session_timeout: Mutex::new(None),
            cseq: Mutex::new(1),
        });

        let connection_clone = Arc::clone(&connection);
        tokio::spawn(async move {
            if let Err(error) = reader_loop(reader, connection_clone, interleaved_tx).await {
                tracing::warn!(?error, "rtsp reader loop ended");
            }
        });

        Ok((connection, interleaved_rx))
    }

    async fn send_request_with_retry(
        &self,
        method: &str,
        uri: &str,
        headers: HashMap<String, String>,
    ) -> anyhow::Result<RtspResponse> {
        let mut attempts = 0;
        let mut last_response = None;
        while attempts < 2 {
            let response = self.send_request(method, uri, headers.clone()).await?;
            if response.status_code == 401 {
                if let Some(header) = response.header("www-authenticate") {
                    if let Some(auth) = self.authenticator.lock().await.as_mut() {
                        if auth.update_challenge(header) {
                            attempts += 1;
                            last_response = Some(response);
                            continue;
                        }
                    }
                }
            }
            return Ok(response);
        }
        last_response.ok_or_else(|| anyhow::anyhow!("invalid rtsp response"))
    }

    async fn send_request(
        &self,
        method: &str,
        uri: &str,
        mut headers: HashMap<String, String>,
    ) -> anyhow::Result<RtspResponse> {
        let cseq = {
            let mut guard = self.cseq.lock().await;
            let value = *guard;
            *guard = guard.saturating_add(1);
            value
        };

        if !headers.contains_key("Session") && method != "DESCRIBE" {
            if let Some(session_id) = self.session_id.lock().await.clone() {
                headers.insert("Session".to_string(), session_id);
            }
        }

        let auth_header = self
            .authenticator
            .lock()
            .await
            .as_mut()
            .map(|auth| auth.authorization_header(method, uri));

        let request = build_request(method, uri, cseq, &headers, auth_header.as_deref());
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(cseq, tx);

        {
            let mut writer = self.writer.lock().await;
            writer.write_all(request.as_bytes()).await?;
            writer.flush().await?;
        }

        rx.await
            .map_err(|_| anyhow::anyhow!("rtsp response channel closed"))
    }

    async fn start_keepalive(self: &Arc<Self>, uri: String) {
        let timeout = *self.session_timeout.lock().await;
        let interval = if let Some(timeout) = timeout {
            let secs = timeout.as_secs_f64();
            Duration::from_secs_f64((secs * 0.5).clamp(1.0, secs.max(1.0) - 1.0))
        } else {
            Duration::from_secs(5)
        };
        let connection = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                sleep(interval).await;
                let headers = HashMap::new();
                let result = connection
                    .send_request_with_retry("OPTIONS", &uri, headers)
                    .await;
                if let Err(error) = result {
                    tracing::warn!(?error, "rtsp keepalive failed");
                    break;
                }
            }
        });
    }
}

async fn reader_loop(
    mut reader: ReadHalf<BoxedStream>,
    connection: Arc<RtspConnection>,
    interleaved_tx: mpsc::Sender<InterleavedPacket>,
) -> anyhow::Result<()> {
    let mut parser = RtspStreamParser::new();
    let mut buffer = [0u8; 16 * 1024];

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let events = parser.append(&buffer[..read]);
        for event in events {
            match event {
                RtspEvent::Interleaved { channel, payload } => {
                    if interleaved_tx
                        .send(InterleavedPacket { channel, payload })
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                RtspEvent::Response(response) => {
                    if let Some(session_info) = parse_session_info(&response) {
                        *connection.session_id.lock().await = Some(session_info.0);
                        *connection.session_timeout.lock().await = session_info.1;
                    }
                    if let Some(cseq) = response.cseq() {
                        if let Some(tx) = connection.pending.lock().await.remove(&cseq) {
                            let _ = tx.send(response);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

fn build_request(
    method: &str,
    uri: &str,
    cseq: u32,
    headers: &HashMap<String, String>,
    auth_header: Option<&str>,
) -> String {
    let mut lines = Vec::new();
    lines.push(format!("{} {} RTSP/1.0", method, uri));
    lines.push(format!("CSeq: {}", cseq));
    lines.push("User-Agent: BambuLANViewer/1.0".to_string());
    for (key, value) in headers {
        lines.push(format!("{}: {}", key, value));
    }
    if let Some(auth) = auth_header {
        lines.push(auth.to_string());
    }
    lines.push(String::new());
    lines.push(String::new());
    lines.join("\r\n")
}

fn parse_interleaved_channels(response: &RtspResponse) -> Option<(u8, u8)> {
    let transport = response.header("transport")?;
    for part in transport.split(';') {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix("interleaved=") {
            let mut parts = value.split('-');
            let rtp = parts.next()?.parse::<u8>().ok()?;
            let rtcp = parts.next()?.parse::<u8>().ok()?;
            return Some((rtp, rtcp));
        }
    }
    None
}

fn parse_session_info(response: &RtspResponse) -> Option<(String, Option<Duration>)> {
    let session = response.header("session")?;
    let mut parts = session.split(';');
    let session_id = parts.next()?.trim();
    if session_id.is_empty() {
        return None;
    }
    let mut timeout = None;
    for part in parts {
        let trimmed = part.trim();
        if let Some(value) = trimmed.strip_prefix("timeout=") {
            if let Ok(seconds) = value.parse::<u64>() {
                timeout = Some(Duration::from_secs(seconds));
            }
        }
    }
    Some((session_id.to_string(), timeout))
}

fn normalize_base_url(value: &str, fallback: &Url) -> Option<Url> {
    let mut url = Url::parse(value).ok()?;
    if url.port().is_none() {
        let _ = url.set_port(fallback.port());
    }
    if url.scheme() != fallback.scheme() {
        let _ = url.set_scheme(fallback.scheme());
    }
    if url.host_str().is_none() {
        let _ = url.set_host(fallback.host_str());
    }
    Some(url)
}
