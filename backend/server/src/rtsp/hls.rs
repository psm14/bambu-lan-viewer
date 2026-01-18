use crate::rtsp::depacketizer::AccessUnit;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

#[derive(Debug)]
pub struct HlsSegmenter {
    output_dir: PathBuf,
    target_duration: f64,
    window: usize,
    sequence: u64,
    segments: VecDeque<SegmentInfo>,
    current: Option<SegmentBuffer>,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct SegmentInfo {
    seq: u64,
    duration: f64,
    filename: String,
}

#[derive(Debug)]
struct SegmentBuffer {
    seq: u64,
    start_pts: u64,
    last_pts: u64,
    data: Vec<u8>,
    has_idr: bool,
    frames: u64,
    muxer: MpegTsMuxer,
}

impl HlsSegmenter {
    pub async fn new(
        output_dir: PathBuf,
        target_duration: f64,
        window: usize,
    ) -> anyhow::Result<Self> {
        fs::create_dir_all(&output_dir).await?;
        Ok(Self {
            output_dir,
            target_duration,
            window,
            sequence: 0,
            segments: VecDeque::new(),
            current: None,
            sps: None,
            pps: None,
        })
    }

    pub fn set_parameter_sets(&mut self, sps: Vec<u8>, pps: Vec<u8>) {
        self.sps = Some(sps);
        self.pps = Some(pps);
    }

    pub async fn push_access_unit(
        &mut self,
        mut access_unit: AccessUnit,
        pts90k: u64,
    ) -> anyhow::Result<()> {
        if self.current.is_none() {
            self.start_segment(pts90k);
        }

        if let Some(current) = self.current.as_ref() {
            let elapsed = (pts90k.saturating_sub(current.start_pts)) as f64 / 90_000.0;
            if elapsed >= self.target_duration && access_unit.is_idr {
                self.finalize_segment().await?;
                self.start_segment(pts90k);
            }
        }

        if let Some(current) = self.current.as_mut() {
            if current.data.is_empty() {
                current
                    .data
                    .extend_from_slice(&current.muxer.write_pat_pmt());
            }

            if current.frames == 0 {
                if access_unit.is_idr {
                    if let (Some(sps), Some(pps)) = (self.sps.clone(), self.pps.clone()) {
                        let mut nals = Vec::with_capacity(access_unit.nals.len() + 2);
                        nals.push(sps);
                        nals.push(pps);
                        nals.extend(access_unit.nals.drain(..));
                        access_unit.nals = nals;
                    }
                }
            }

            current.has_idr |= access_unit.is_idr;
            current.last_pts = pts90k;
            let bytes = current.muxer.write_access_unit(pts90k, &access_unit.nals);
            current.data.extend_from_slice(&bytes);
            current.frames = current.frames.saturating_add(1);
        }

        Ok(())
    }

    pub async fn finalize_segment(&mut self) -> anyhow::Result<()> {
        let current = match self.current.take() {
            Some(current) => current,
            None => return Ok(()),
        };

        let duration = if current.last_pts > current.start_pts {
            (current.last_pts - current.start_pts) as f64 / 90_000.0
        } else {
            0.1
        };

        let filename = format!("seg{:06}.ts", current.seq);
        let path = self.output_dir.join(&filename);
        fs::write(&path, current.data).await?;
        debug!(segment = %filename, duration = %duration, "hls segment written");

        self.segments.push_back(SegmentInfo {
            seq: current.seq,
            duration,
            filename,
        });

        while self.segments.len() > self.window {
            if let Some(old) = self.segments.pop_front() {
                let old_path = self.output_dir.join(&old.filename);
                let _ = fs::remove_file(old_path).await;
            }
        }

        self.write_playlist().await?;
        Ok(())
    }

    fn start_segment(&mut self, pts90k: u64) {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        self.current = Some(SegmentBuffer {
            seq,
            start_pts: pts90k,
            last_pts: pts90k,
            data: Vec::new(),
            has_idr: false,
            frames: 0,
            muxer: MpegTsMuxer::new(),
        });
    }

    async fn write_playlist(&self) -> anyhow::Result<()> {
        let max_segment = self
            .segments
            .iter()
            .map(|seg| seg.duration)
            .fold(0.0_f64, f64::max);
        let target_duration = self.target_duration.max(max_segment).ceil() as u64;
        let media_sequence = self.segments.front().map(|seg| seg.seq).unwrap_or(0);
        let mut lines = Vec::new();
        lines.push("#EXTM3U".to_string());
        lines.push("#EXT-X-VERSION:3".to_string());
        lines.push("#EXT-X-INDEPENDENT-SEGMENTS".to_string());
        lines.push(format!("#EXT-X-TARGETDURATION:{}", target_duration));
        lines.push(format!("#EXT-X-MEDIA-SEQUENCE:{}", media_sequence));

        for seg in &self.segments {
            lines.push(format!("#EXTINF:{:.3},", seg.duration));
            lines.push(seg.filename.clone());
        }

        let playlist = lines.join("\n") + "\n";
        let tmp_path = self.output_dir.join("stream.m3u8.tmp");
        let final_path = self.output_dir.join("stream.m3u8");
        fs::write(&tmp_path, playlist).await?;
        fs::rename(tmp_path, final_path).await?;
        Ok(())
    }

    pub fn playlist_path(&self) -> PathBuf {
        self.output_dir.join("stream.m3u8")
    }

    pub fn segment_path(&self, name: &str) -> PathBuf {
        self.output_dir.join(name)
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }
}

#[derive(Debug, Clone)]
struct MpegTsMuxer {
    pat_cc: u8,
    pmt_cc: u8,
    video_cc: u8,
}

impl MpegTsMuxer {
    fn new() -> Self {
        Self {
            pat_cc: 0,
            pmt_cc: 0,
            video_cc: 0,
        }
    }

    fn write_pat_pmt(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.write_pat());
        out.extend_from_slice(&self.write_pmt());
        out
    }

    fn write_access_unit(&mut self, pts90k: u64, nals: &[Vec<u8>]) -> Vec<u8> {
        let pes = build_pes(pts90k, nals);
        packetize(0x101, &pes, &mut self.video_cc, Some(pts90k))
    }

    fn write_pat(&mut self) -> Vec<u8> {
        let section = build_pat_section();
        packetize_with_pointer(0x0000, &section, &mut self.pat_cc)
    }

    fn write_pmt(&mut self) -> Vec<u8> {
        let section = build_pmt_section();
        packetize_with_pointer(0x0100, &section, &mut self.pmt_cc)
    }
}

fn build_pes(pts90k: u64, nals: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = Vec::new();
    for nal in nals {
        payload.extend_from_slice(&[0x00, 0x00, 0x00, 0x01]);
        payload.extend_from_slice(nal);
    }

    let pts = encode_pts(pts90k);
    let mut pes = Vec::with_capacity(payload.len() + 19);
    pes.extend_from_slice(&[0x00, 0x00, 0x01, 0xE0]);
    pes.extend_from_slice(&[0x00, 0x00]);
    pes.push(0x80);
    pes.push(0x80);
    pes.push(0x05);
    pes.extend_from_slice(&pts);
    pes.extend_from_slice(&payload);
    pes
}

fn encode_pts(pts90k: u64) -> [u8; 5] {
    let pts = pts90k & 0x1FFFFFFFF;
    let b0 = 0x20 | (((pts >> 30) & 0x07) as u8) << 1 | 1;
    let b1 = ((pts >> 22) & 0xFF) as u8;
    let b2 = (((pts >> 15) & 0x7F) as u8) << 1 | 1;
    let b3 = ((pts >> 7) & 0xFF) as u8;
    let b4 = (((pts >> 0) & 0x7F) as u8) << 1 | 1;
    [b0, b1, b2, b3, b4]
}

fn build_pat_section() -> Vec<u8> {
    let mut section = Vec::new();
    section.push(0x00);
    section.extend_from_slice(&[0xB0, 0x0D]);
    section.extend_from_slice(&[0x00, 0x01]);
    section.push(0xC1);
    section.push(0x00);
    section.push(0x00);
    section.extend_from_slice(&[0x00, 0x01]);
    section.extend_from_slice(&[0xE1, 0x00]);
    let crc = mpeg_crc32(&section);
    section.extend_from_slice(&crc.to_be_bytes());
    section
}

fn build_pmt_section() -> Vec<u8> {
    let mut section = Vec::new();
    section.push(0x02);
    section.extend_from_slice(&[0xB0, 0x12]);
    section.extend_from_slice(&[0x00, 0x01]);
    section.push(0xC1);
    section.push(0x00);
    section.push(0x00);
    section.extend_from_slice(&[0xE1, 0x01]);
    section.extend_from_slice(&[0xF0, 0x00]);
    section.push(0x1B);
    section.extend_from_slice(&[0xE1, 0x01]);
    section.extend_from_slice(&[0xF0, 0x00]);
    let crc = mpeg_crc32(&section);
    section.extend_from_slice(&crc.to_be_bytes());
    section
}

fn packetize_with_pointer(pid: u16, section: &[u8], cc: &mut u8) -> Vec<u8> {
    let mut payload = Vec::with_capacity(section.len() + 1);
    payload.push(0x00);
    payload.extend_from_slice(section);
    packetize(pid, &payload, cc, None)
}

fn packetize(pid: u16, payload: &[u8], cc: &mut u8, pcr: Option<u64>) -> Vec<u8> {
    let mut out = Vec::new();
    let mut offset = 0;
    let mut first = true;

    while offset < payload.len() {
        let remaining = payload.len() - offset;
        let payload_start = offset == 0;
        let mut payload_len = remaining.min(184);
        let mut packet = Vec::with_capacity(188);
        packet.push(0x47);
        let mut b1 = ((pid >> 8) & 0x1F) as u8;
        if payload_start {
            b1 |= 0x40;
        }
        packet.push(b1);
        packet.push((pid & 0xFF) as u8);

        if first {
            if let Some(pcr_value) = pcr {
                let max_payload = 176;
                payload_len = remaining.min(max_payload);
                packet.push(0x30 | (*cc & 0x0F));
                let adapt_len = 183 - payload_len;
                packet.push(adapt_len as u8);
                if adapt_len >= 7 {
                    packet.push(0x10);
                    packet.extend_from_slice(&encode_pcr(pcr_value));
                    if adapt_len > 7 {
                        packet.extend(std::iter::repeat(0xFF).take(adapt_len - 7));
                    }
                }
            } else if payload_len < 184 {
                packet.push(0x30 | (*cc & 0x0F));
                let adapt_len = 183 - payload_len;
                packet.push(adapt_len as u8);
                if adapt_len > 0 {
                    packet.push(0x00);
                    if adapt_len > 1 {
                        packet.extend(std::iter::repeat(0xFF).take(adapt_len - 1));
                    }
                }
            } else {
                packet.push(0x10 | (*cc & 0x0F));
            }
        } else if payload_len < 184 {
            packet.push(0x30 | (*cc & 0x0F));
            let adapt_len = 183 - payload_len;
            packet.push(adapt_len as u8);
            if adapt_len > 0 {
                packet.push(0x00);
                if adapt_len > 1 {
                    packet.extend(std::iter::repeat(0xFF).take(adapt_len - 1));
                }
            }
        } else {
            packet.push(0x10 | (*cc & 0x0F));
        }
        *cc = cc.wrapping_add(1) & 0x0F;

        packet.extend_from_slice(&payload[offset..offset + payload_len]);

        if packet.len() < 188 {
            packet.extend(std::iter::repeat(0xFF).take(188 - packet.len()));
        }

        out.extend_from_slice(&packet);
        offset += payload_len;
        first = false;
    }

    out
}

fn encode_pcr(base90k: u64) -> [u8; 6] {
    let base = base90k & 0x1FFFFFFFF;
    let ext: u16 = 0;
    let b0 = (base >> 25) as u8;
    let b1 = (base >> 17) as u8;
    let b2 = (base >> 9) as u8;
    let b3 = (base >> 1) as u8;
    let b4 = ((base & 0x1) as u8) << 7 | 0x7E | ((ext >> 8) as u8 & 0x01);
    let b5 = (ext & 0xFF) as u8;
    [b0, b1, b2, b3, b4, b5]
}

fn mpeg_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= (byte as u32) << 24;
        for _ in 0..8 {
            if (crc & 0x80000000) != 0 {
                crc = (crc << 1) ^ 0x04C11DB7;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}
