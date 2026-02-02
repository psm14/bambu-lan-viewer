use crate::rtsp::depacketizer::AccessUnit;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
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
    ll_enabled: bool,
    part_duration: f64,
}

#[derive(Debug, Clone)]
struct SegmentInfo {
    seq: u64,
    duration: f64,
    filename: String,
    parts: Vec<PartInfo>,
}

#[derive(Debug, Clone)]
struct PartInfo {
    index: u32,
    duration: f64,
    byte_start: u64,
    byte_length: u64,
    independent: bool,
}

#[derive(Debug)]
struct SegmentBuffer {
    seq: u64,
    start_pts: u64,
    last_pts: u64,
    has_idr: bool,
    frames: u64,
    muxer: MpegTsMuxer,
    filename: String,
    file: fs::File,
    bytes_written: u64,
    parts: Vec<PartInfo>,
    part_index: u32,
    part_start_pts: u64,
    part_start_byte: u64,
    part_bytes: Vec<u8>,
    part_frames: u64,
    part_independent: bool,
}

impl HlsSegmenter {
    pub async fn new(
        output_dir: PathBuf,
        target_duration: f64,
        window: usize,
        ll_enabled: bool,
        part_duration: f64,
    ) -> anyhow::Result<Self> {
        fs::create_dir_all(&output_dir).await?;
        let mut resolved_part_duration = if ll_enabled {
            part_duration
        } else {
            target_duration
        };
        if !resolved_part_duration.is_finite() || resolved_part_duration <= 0.0 {
            resolved_part_duration = target_duration;
        }
        let min_part = 0.1;
        let max_part = target_duration.max(min_part);
        resolved_part_duration = resolved_part_duration.max(min_part).min(max_part);
        Ok(Self {
            output_dir,
            target_duration,
            window,
            sequence: 0,
            segments: VecDeque::new(),
            current: None,
            sps: None,
            pps: None,
            ll_enabled,
            part_duration: resolved_part_duration,
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
            if !access_unit.is_idr {
                return Ok(());
            }
            self.start_segment(pts90k).await?;
        }

        let mut current = match self.current.take() {
            Some(current) => current,
            None => return Ok(()),
        };

        let elapsed = (pts90k.saturating_sub(current.start_pts)) as f64 / 90_000.0;
        if elapsed >= self.target_duration && access_unit.is_idr {
            self.flush_part(&mut current).await?;
            self.finalize_segment_buffer(current).await?;
            self.start_segment(pts90k).await?;
            current = match self.current.take() {
                Some(current) => current,
                None => return Ok(()),
            };
        }

        if self.ll_enabled {
            let part_elapsed =
                (pts90k.saturating_sub(current.part_start_pts)) as f64 / 90_000.0;
            if current.part_frames > 0 && part_elapsed >= self.part_duration {
                self.flush_part(&mut current).await?;
                self.reset_part(&mut current, pts90k, access_unit.is_idr);
                self.write_playlist(Some(&current)).await?;
            }
        }

        if current.bytes_written == 0 && current.part_bytes.is_empty() {
            current
                .part_bytes
                .extend_from_slice(&current.muxer.write_pat_pmt());
        }

        let mut prepend: Vec<Vec<u8>> = Vec::new();
        let has_aud = access_unit
            .nals
            .iter()
            .any(|nal| nal.first().map(|b| b & 0x1F) == Some(9));
        if !has_aud {
            prepend.push(vec![0x09, 0xF0]);
        }

        if current.frames == 0 && access_unit.is_idr {
            if let (Some(sps), Some(pps)) = (self.sps.clone(), self.pps.clone()) {
                prepend.push(sps);
                prepend.push(pps);
            }
        }

        if !prepend.is_empty() {
            let mut nals = Vec::with_capacity(access_unit.nals.len() + prepend.len());
            nals.extend(prepend);
            nals.extend(access_unit.nals.drain(..));
            access_unit.nals = nals;
        }

        current.has_idr |= access_unit.is_idr;
        current.last_pts = pts90k;
        let bytes = current
            .muxer
            .write_access_unit(pts90k, &access_unit.nals, access_unit.is_idr);
        current.part_bytes.extend_from_slice(&bytes);
        current.frames = current.frames.saturating_add(1);
        current.part_frames = current.part_frames.saturating_add(1);

        self.current = Some(current);

        Ok(())
    }

    pub async fn finalize_segment(&mut self) -> anyhow::Result<()> {
        let current = match self.current.take() {
            Some(current) => current,
            None => return Ok(()),
        };
        self.finalize_segment_buffer(current).await
    }

    async fn start_segment(&mut self, pts90k: u64) -> anyhow::Result<()> {
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        let filename = format!("seg{:06}.ts", seq);
        let path = self.output_dir.join(&filename);
        let file = fs::File::create(&path).await?;
        self.current = Some(SegmentBuffer {
            seq,
            start_pts: pts90k,
            last_pts: pts90k,
            has_idr: false,
            frames: 0,
            muxer: MpegTsMuxer::new(),
            filename,
            file,
            bytes_written: 0,
            parts: Vec::new(),
            part_index: 0,
            part_start_pts: pts90k,
            part_start_byte: 0,
            part_bytes: Vec::new(),
            part_frames: 0,
            part_independent: true,
        });
        Ok(())
    }

    async fn flush_part(&self, current: &mut SegmentBuffer) -> anyhow::Result<()> {
        if current.part_frames == 0 {
            return Ok(());
        }
        let data = std::mem::take(&mut current.part_bytes);
        if data.is_empty() {
            current.part_frames = 0;
            return Ok(());
        }
        current.file.write_all(&data).await?;
        current.file.flush().await?;
        let byte_start = current.part_start_byte;
        let byte_length = data.len() as u64;
        current.bytes_written = current.bytes_written.saturating_add(byte_length);
        let duration = if current.last_pts > current.part_start_pts {
            (current.last_pts - current.part_start_pts) as f64 / 90_000.0
        } else {
            self.part_duration.min(self.target_duration).max(0.1)
        };
        current.parts.push(PartInfo {
            index: current.part_index,
            duration,
            byte_start,
            byte_length,
            independent: current.part_independent,
        });
        current.part_index = current.part_index.saturating_add(1);
        current.part_start_byte = current.bytes_written;
        current.part_frames = 0;
        current.part_independent = false;
        Ok(())
    }

    fn reset_part(&self, current: &mut SegmentBuffer, start_pts: u64, independent: bool) {
        current.part_start_pts = start_pts;
        current.part_start_byte = current.bytes_written;
        current.part_frames = 0;
        current.part_independent = independent;
        current.part_bytes.clear();
    }

    async fn finalize_segment_buffer(
        &mut self,
        mut current: SegmentBuffer,
    ) -> anyhow::Result<()> {
        self.flush_part(&mut current).await?;
        let _ = current.file.flush().await;

        let duration = if current.last_pts > current.start_pts {
            (current.last_pts - current.start_pts) as f64 / 90_000.0
        } else {
            0.1
        };

        let filename = current.filename.clone();
        debug!(segment = %filename, duration = %duration, "hls segment written");

        self.segments.push_back(SegmentInfo {
            seq: current.seq,
            duration,
            filename,
            parts: current.parts,
        });

        while self.segments.len() > self.window {
            if let Some(old) = self.segments.pop_front() {
                let old_path = self.output_dir.join(&old.filename);
                let _ = fs::remove_file(old_path).await;
            }
        }

        self.write_playlist(None).await?;
        Ok(())
    }

    async fn write_playlist(&self, current: Option<&SegmentBuffer>) -> anyhow::Result<()> {
        let standard = self.render_standard_playlist();
        let tmp_path = self.output_dir.join("stream.m3u8.tmp");
        let final_path = self.output_dir.join("stream.m3u8");
        fs::write(&tmp_path, standard).await?;
        fs::rename(tmp_path, final_path).await?;

        if self.ll_enabled {
            let ll = self.render_ll_playlist(current);
            let tmp_ll = self.output_dir.join("stream_ll.m3u8.tmp");
            let final_ll = self.output_dir.join("stream_ll.m3u8");
            fs::write(&tmp_ll, ll).await?;
            fs::rename(tmp_ll, final_ll).await?;
        }
        Ok(())
    }

    fn render_standard_playlist(&self) -> String {
        let max_segment = self
            .segments
            .iter()
            .map(|seg| seg.duration)
            .fold(0.0_f64, f64::max);
        let target_duration = self.target_duration.max(max_segment).ceil() as u64;
        let media_sequence = self.segments.front().map(|seg| seg.seq).unwrap_or(0);
        let mut lines = Vec::new();
        lines.push("#EXTM3U".to_string());
        lines.push("#EXT-X-VERSION:4".to_string());
        lines.push("#EXT-X-INDEPENDENT-SEGMENTS".to_string());
        lines.push(format!("#EXT-X-TARGETDURATION:{}", target_duration));
        lines.push(format!("#EXT-X-MEDIA-SEQUENCE:{}", media_sequence));

        for seg in &self.segments {
            lines.push(format!("#EXTINF:{:.3},", seg.duration));
            lines.push(seg.filename.clone());
        }

        lines.join("\n") + "\n"
    }

    fn render_ll_playlist(&self, current: Option<&SegmentBuffer>) -> String {
        let max_segment = self
            .segments
            .iter()
            .map(|seg| seg.duration)
            .fold(0.0_f64, f64::max);
        let target_duration = self.target_duration.max(max_segment).ceil() as u64;
        let media_sequence = self
            .segments
            .front()
            .map(|seg| seg.seq)
            .or_else(|| current.map(|seg| seg.seq))
            .unwrap_or(0);
        let part_hold_back = (self.part_duration * 3.0).max(self.part_duration + 0.1);
        let hold_back = (target_duration as f64 * 3.0).max(part_hold_back * 2.0);

        let mut lines = Vec::new();
        lines.push("#EXTM3U".to_string());
        lines.push("#EXT-X-VERSION:9".to_string());
        lines.push("#EXT-X-INDEPENDENT-SEGMENTS".to_string());
        lines.push(format!("#EXT-X-TARGETDURATION:{}", target_duration));
        lines.push(format!(
            "#EXT-X-PART-INF:PART-TARGET={:.3}",
            self.part_duration
        ));
        lines.push(format!(
            "#EXT-X-SERVER-CONTROL:PART-HOLD-BACK={:.3},HOLD-BACK={:.3}",
            part_hold_back, hold_back
        ));
        lines.push(format!("#EXT-X-MEDIA-SEQUENCE:{}", media_sequence));

        for seg in &self.segments {
            Self::append_parts(&mut lines, &seg.filename, &seg.parts);
            lines.push(format!("#EXTINF:{:.3},", seg.duration));
            lines.push(seg.filename.clone());
        }

        if let Some(current) = current {
            Self::append_parts(&mut lines, &current.filename, &current.parts);
        }

        lines.join("\n") + "\n"
    }

    fn append_parts(lines: &mut Vec<String>, filename: &str, parts: &[PartInfo]) {
        for part in parts {
            let mut line = format!(
                "#EXT-X-PART:DURATION={:.3},URI=\"{}\",BYTERANGE=\"{}@{}\"",
                part.duration, filename, part.byte_length, part.byte_start
            );
            if part.independent {
                line.push_str(",INDEPENDENT=YES");
            }
            lines.push(line);
        }
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

    fn write_access_unit(&mut self, pts90k: u64, nals: &[Vec<u8>], is_idr: bool) -> Vec<u8> {
        let pes = build_pes(pts90k, nals);
        packetize(0x101, &pes, &mut self.video_cc, Some(pts90k), is_idr)
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
    let pes_len = payload.len().saturating_add(8);
    let pes_packet_length = if pes_len > u16::MAX as usize {
        0
    } else {
        pes_len as u16
    };
    pes.extend_from_slice(&pes_packet_length.to_be_bytes());
    pes.push(0x84);
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
    packetize(pid, &payload, cc, None, false)
}

fn packetize(pid: u16, payload: &[u8], cc: &mut u8, pcr: Option<u64>, is_idr: bool) -> Vec<u8> {
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
                    let mut flags = 0x10;
                    if is_idr {
                        flags |= 0x40;
                    }
                    packet.push(flags);
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
