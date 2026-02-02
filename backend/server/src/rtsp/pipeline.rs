use crate::config::{AppConfig, PrinterConfig};
use crate::rtsp::auth::RtspCredentials;
use crate::rtsp::client::RtspClient;
use crate::rtsp::depacketizer::H264RtpDepacketizer;
use crate::rtsp::hls::HlsSegmenter;
use crate::rtsp::rtp::RtpPacket;
use crate::rtsp::time::RtpTimeMapper;
use crate::state::PrinterState;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info, warn};
use url::Url;

pub async fn run_rtsp_hls(
    settings: AppConfig,
    printer: PrinterConfig,
    state: Arc<RwLock<PrinterState>>,
    output_dir: PathBuf,
) {
    let mut segmenter = match HlsSegmenter::new(
        output_dir,
        settings.hls_target_duration_secs,
        settings.hls_window_segments,
        settings.hls_low_latency,
        settings.hls_part_duration_secs,
    )
    .await
    {
        Ok(segmenter) => segmenter,
        Err(error) => {
            warn!(?error, "failed to initialize hls segmenter");
            return;
        }
    };

    let mut warned_missing = false;

    loop {
        let url = match resolve_rtsp_url(&printer, &state).await {
            Some(url) => {
                warned_missing = false;
                url
            }
            None => {
                if !warned_missing {
                    warn!("waiting for rtsp url from mqtt report");
                    warned_missing = true;
                }
                sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        if let Err(error) = run_session(&settings, &printer, &mut segmenter, url).await {
            warn!(?error, "rtsp session ended");
        }
        sleep(Duration::from_secs(2)).await;
    }
}

async fn run_session(
    settings: &AppConfig,
    printer: &PrinterConfig,
    segmenter: &mut HlsSegmenter,
    url: Url,
) -> anyhow::Result<()> {
    let credentials = Some(RtspCredentials {
        username: "bblp".to_string(),
        password: printer.access_code.clone(),
    });
    info!(%url, "starting rtsp session");
    let client = RtspClient::new(url.clone(), credentials, settings.rtsp_tls_insecure);
    let mut session = client.start().await?;

    if let (Some(sps), Some(pps)) = (session.sdp.sps.clone(), session.sdp.pps.clone()) {
        segmenter.set_parameter_sets(sps, pps);
    }

    let expected_payload = session.sdp.payload_type;
    let mut depacketizer = H264RtpDepacketizer::new();
    let mut time_mapper = RtpTimeMapper::new();

    let mut saw_interleaved = false;
    let mut saw_rtp = false;
    let mut saw_access_unit = false;

    while let Some(packet) = session.interleaved_rx.recv().await {
        if !saw_interleaved {
            saw_interleaved = true;
            debug!(
                channel = packet.channel,
                bytes = packet.payload.len(),
                "rtsp interleaved packet received"
            );
        }
        if packet.channel != session.rtp_channel {
            continue;
        }
        let rtp = match RtpPacket::parse(&packet.payload) {
            Some(packet) => packet,
            None => continue,
        };
        if !saw_rtp {
            saw_rtp = true;
            debug!(
                payload_type = rtp.payload_type,
                sequence = rtp.sequence_number,
                timestamp = rtp.timestamp,
                "rtp packet received"
            );
        }
        if let Some(expected) = expected_payload {
            if rtp.payload_type != expected {
                continue;
            }
        }

        let access_units = depacketizer.handle(&rtp);
        if !access_units.is_empty() && !saw_access_unit {
            saw_access_unit = true;
            let first = &access_units[0];
            debug!(
                nals = first.nals.len(),
                is_idr = first.is_idr,
                rtp_timestamp = first.rtp_timestamp,
                "h264 access unit assembled"
            );
        }
        if let Some((sps, pps)) = depacketizer.take_parameter_sets() {
            segmenter.set_parameter_sets(sps, pps);
        }

        for access_unit in access_units {
            let pts = time_mapper.pts90k(access_unit.rtp_timestamp);
            segmenter.push_access_unit(access_unit, pts).await?;
        }
    }

    segmenter.finalize_segment().await?;
    Ok(())
}

async fn resolve_rtsp_url(
    printer: &PrinterConfig,
    state: &Arc<RwLock<PrinterState>>,
) -> Option<Url> {
    if let Some(url) = printer.rtsp_url.as_ref() {
        return Url::parse(url).ok();
    }

    let rtsp_url = state.read().await.rtsp_url.clone()?;
    Url::parse(&rtsp_url).ok()
}
