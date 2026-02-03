use crate::rtsp::depacketizer::AccessUnit;
use crate::rtsp::stream::{CmafInit, CmafStream};
use bytes::Bytes;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::debug;

#[derive(Debug)]
pub struct CmafSegmenter {
    output_dir: PathBuf,
    target_duration: f64,
    window: usize,
    sequence: u64,
    segments: VecDeque<SegmentInfo>,
    current: Option<SegmentBuffer>,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    last_init_sps: Option<Vec<u8>>,
    last_init_pps: Option<Vec<u8>>,
    part_duration: f64,
    last_sample_duration: Option<u32>,
    fragment_sequence: u32,
    stream: Option<CmafStream>,
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
    frames: u64,
    filename: String,
    file: fs::File,
    bytes_written: u64,
    parts: Vec<PartInfo>,
    part_index: u32,
    part_start_pts: u64,
    part_start_byte: u64,
    part_samples: Vec<Sample>,
    part_independent: bool,
}

#[derive(Debug, Clone)]
struct Sample {
    pts90k: u64,
    is_idr: bool,
    nals: Vec<Vec<u8>>,
}

impl CmafSegmenter {
    pub async fn new(
        output_dir: PathBuf,
        target_duration: f64,
        window: usize,
        part_duration: f64,
        stream: Option<CmafStream>,
    ) -> anyhow::Result<Self> {
        fs::create_dir_all(&output_dir).await?;
        let mut resolved_part_duration = part_duration;
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
            last_init_sps: None,
            last_init_pps: None,
            part_duration: resolved_part_duration,
            last_sample_duration: None,
            fragment_sequence: 1,
            stream,
        })
    }

    pub fn set_parameter_sets(&mut self, sps: Vec<u8>, pps: Vec<u8>) {
        self.sps = Some(sps);
        self.pps = Some(pps);
    }

    pub async fn ensure_init(&mut self) -> anyhow::Result<()> {
        self.write_init_if_needed().await
    }

    pub async fn push_access_unit(
        &mut self,
        access_unit: AccessUnit,
        pts90k: u64,
    ) -> anyhow::Result<()> {
        self.write_init_if_needed().await?;

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

        if current.part_samples.is_empty() {
            current.part_start_pts = pts90k;
            current.part_start_byte = current.bytes_written;
            current.part_independent = access_unit.is_idr;
        }

        let part_elapsed =
            (pts90k.saturating_sub(current.part_start_pts)) as f64 / 90_000.0;
        if current.part_samples.len() > 0 && part_elapsed >= self.part_duration {
            self.flush_part(&mut current).await?;
            current.part_start_pts = pts90k;
            current.part_start_byte = current.bytes_written;
            current.part_independent = access_unit.is_idr;
            self.write_playlist(Some(&current)).await?;
        }

        current.last_pts = pts90k;
        current.frames = current.frames.saturating_add(1);
        current.part_samples.push(Sample {
            pts90k,
            is_idr: access_unit.is_idr,
            nals: access_unit.nals,
        });

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
        let filename = format!("seg{:06}.m4s", seq);
        let path = self.output_dir.join(&filename);
        let file = fs::File::create(&path).await?;
        self.current = Some(SegmentBuffer {
            seq,
            start_pts: pts90k,
            last_pts: pts90k,
            frames: 0,
            filename,
            file,
            bytes_written: 0,
            parts: Vec::new(),
            part_index: 0,
            part_start_pts: pts90k,
            part_start_byte: 0,
            part_samples: Vec::new(),
            part_independent: true,
        });
        Ok(())
    }

    async fn flush_part(&mut self, current: &mut SegmentBuffer) -> anyhow::Result<()> {
        if current.part_samples.is_empty() {
            return Ok(());
        }

        let samples = std::mem::take(&mut current.part_samples);
        let part_start_pts = current.part_start_pts;
        let (durations, total_duration_90k) =
            self.compute_sample_durations(&samples);

        let mut sample_datas = Vec::with_capacity(samples.len());
        let mut sample_sizes = Vec::with_capacity(samples.len());
        let mut sample_flags = Vec::with_capacity(samples.len());
        for (idx, sample) in samples.iter().enumerate() {
            let data = build_avc_sample(&sample.nals);
            sample_sizes.push(data.len() as u32);
            sample_datas.push(data);
            sample_flags.push(if sample.is_idr {
                SAMPLE_FLAG_SYNC
            } else {
                SAMPLE_FLAG_NON_SYNC
            });
            if idx == samples.len() - 1 {
                self.last_sample_duration = durations.last().copied();
            }
        }

        let sequence = self.fragment_sequence;
        self.fragment_sequence = self.fragment_sequence.wrapping_add(1);
        let moof = build_moof(
            sequence,
            part_start_pts,
            &durations,
            &sample_sizes,
            &sample_flags,
        );
        let styp = build_styp();
        let mdat = build_mdat(&sample_datas);
        let mut part_bytes = Vec::with_capacity(styp.len() + moof.len() + mdat.len());
        part_bytes.extend_from_slice(&styp);
        part_bytes.extend_from_slice(&moof);
        part_bytes.extend_from_slice(&mdat);
        let part_bytes = Bytes::from(part_bytes);

        if let Some(stream) = &self.stream {
            stream.send_fragment(part_bytes.clone());
        }

        current.file.write_all(part_bytes.as_ref()).await?;
        current.file.flush().await?;

        let byte_start = current.part_start_byte;
        let byte_length = part_bytes.len() as u64;
        current.bytes_written = current.bytes_written.saturating_add(byte_length);

        let duration = (total_duration_90k as f64) / 90_000.0;
        let part_index = current.part_index;
        current.parts.push(PartInfo {
            index: part_index,
            duration: duration.max(0.001),
            byte_start,
            byte_length,
            independent: current.part_independent,
        });
        current.part_index = current.part_index.saturating_add(1);
        current.part_start_byte = current.bytes_written;
        current.part_independent = false;

        debug!(
            part = part_index,
            bytes = byte_length,
            duration = duration,
            "cmaf part written"
        );

        Ok(())
    }

    fn compute_sample_durations(&self, samples: &[Sample]) -> (Vec<u32>, u64) {
        let mut durations = Vec::with_capacity(samples.len());
        let mut total = 0u64;
        for i in 0..samples.len() {
            let duration = if i + 1 < samples.len() {
                let current = samples[i].pts90k;
                let next = samples[i + 1].pts90k;
                if next > current {
                    (next - current) as u32
                } else {
                    self.last_sample_duration.unwrap_or(3000)
                }
            } else {
                self.last_sample_duration.unwrap_or_else(|| {
                    if samples.len() > 1 {
                        durations.last().copied().unwrap_or(3000)
                    } else {
                        (self.part_duration * 90_000.0) as u32
                    }
                })
            };
            durations.push(duration.max(1));
            total += duration as u64;
        }
        (durations, total)
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
        debug!(segment = %filename, duration = %duration, "cmaf segment written");

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
        let playlist = self.render_playlist(current);
        let tmp_path = self.output_dir.join("stream.m3u8.tmp");
        let final_path = self.output_dir.join("stream.m3u8");
        fs::write(&tmp_path, playlist).await?;
        fs::rename(tmp_path, final_path).await?;
        Ok(())
    }

    fn render_playlist(&self, current: Option<&SegmentBuffer>) -> String {
        let max_segment = self
            .segments
            .iter()
            .map(|seg| seg.duration)
            .fold(0.0_f64, f64::max);
        let target_duration = self.target_duration.max(max_segment).ceil() as u64;
        let mut max_part = self.part_duration;
        for seg in &self.segments {
            for part in &seg.parts {
                if part.duration > max_part {
                    max_part = part.duration;
                }
            }
        }
        if let Some(current) = current {
            for part in &current.parts {
                if part.duration > max_part {
                    max_part = part.duration;
                }
            }
        }
        let media_sequence = self
            .segments
            .front()
            .map(|seg| seg.seq)
            .or_else(|| current.map(|seg| seg.seq))
            .unwrap_or(0);
        let part_hold_back = (max_part * 3.0).max(max_part + 0.1);
        let hold_back = (target_duration as f64 * 3.0).max(part_hold_back * 2.0);

        let mut lines = Vec::new();
        lines.push("#EXTM3U".to_string());
        lines.push("#EXT-X-VERSION:9".to_string());
        lines.push("#EXT-X-INDEPENDENT-SEGMENTS".to_string());
        lines.push(format!("#EXT-X-TARGETDURATION:{}", target_duration));
        lines.push(format!(
            "#EXT-X-PART-INF:PART-TARGET={:.3}",
            max_part
        ));
        lines.push(format!(
            "#EXT-X-SERVER-CONTROL:CAN-BLOCK-RELOAD=YES,PART-HOLD-BACK={:.3},HOLD-BACK={:.3}",
            part_hold_back, hold_back
        ));
        lines.push("#EXT-X-MAP:URI=\"init.mp4\"".to_string());
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

    async fn write_init_if_needed(&mut self) -> anyhow::Result<()> {
        let (sps, pps) = match (self.sps.clone(), self.pps.clone()) {
            (Some(sps), Some(pps)) => (sps, pps),
            _ => return Ok(()),
        };

        if self.last_init_sps.as_ref() == Some(&sps) && self.last_init_pps.as_ref() == Some(&pps) {
            return Ok(());
        }

        let (width, height) = parse_sps_dimensions(&sps).unwrap_or((1280, 720));
        let init = build_init_mp4(&sps, &pps, width, height);
        let codec = codec_string_from_sps(&sps);
        let init_bytes = Bytes::from(init);
        let path = self.output_dir.join("init.mp4");
        fs::write(&path, init_bytes.as_ref()).await?;
        if let Some(stream) = &self.stream {
            stream.update_init(CmafInit {
                bytes: init_bytes.clone(),
                codec,
            });
        }
        self.last_init_sps = Some(sps);
        self.last_init_pps = Some(pps);
        Ok(())
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }
}

const SAMPLE_FLAG_SYNC: u32 = 0x02000000;
const SAMPLE_FLAG_NON_SYNC: u32 = 0x01010000;

fn build_avc_sample(nals: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    for nal in nals {
        let len = nal.len() as u32;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(nal);
    }
    out
}

fn build_moof(
    sequence: u32,
    base_decode_time: u64,
    sample_durations: &[u32],
    sample_sizes: &[u32],
    sample_flags: &[u32],
) -> Vec<u8> {
    let sample_count = sample_durations.len() as u32;
    let trun_size = 20 + (sample_count as usize * 12);
    let traf_size = 8 + 16 + 20 + trun_size;
    let moof_size = 8 + 16 + traf_size;
    let data_offset = (moof_size + 8) as i32;

    let mut trun = Vec::with_capacity(trun_size);
    write_u32(&mut trun, 0x000001 | 0x000100 | 0x000200 | 0x000400);
    write_u32(&mut trun, sample_count);
    write_i32(&mut trun, data_offset);
    for i in 0..sample_count as usize {
        write_u32(&mut trun, sample_durations[i]);
        write_u32(&mut trun, sample_sizes[i]);
        write_u32(&mut trun, sample_flags[i]);
    }
    let trun_box = make_box(*b"trun", trun);

    let mut tfhd = Vec::with_capacity(8);
    write_u32(&mut tfhd, 0x020000);
    write_u32(&mut tfhd, 1);
    let tfhd_box = make_box(*b"tfhd", tfhd);

    let mut tfdt = Vec::with_capacity(12);
    write_u32(&mut tfdt, 0x01000000);
    write_u64(&mut tfdt, base_decode_time);
    let tfdt_box = make_box(*b"tfdt", tfdt);

    let mut traf_payload = Vec::new();
    traf_payload.extend_from_slice(&tfhd_box);
    traf_payload.extend_from_slice(&tfdt_box);
    traf_payload.extend_from_slice(&trun_box);
    let traf_box = make_box(*b"traf", traf_payload);

    let mut mfhd = Vec::with_capacity(8);
    write_u32(&mut mfhd, 0);
    write_u32(&mut mfhd, sequence);
    let mfhd_box = make_box(*b"mfhd", mfhd);

    let mut moof_payload = Vec::new();
    moof_payload.extend_from_slice(&mfhd_box);
    moof_payload.extend_from_slice(&traf_box);
    make_box(*b"moof", moof_payload)
}

fn build_mdat(samples: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = Vec::new();
    for sample in samples {
        payload.extend_from_slice(sample);
    }
    make_box(*b"mdat", payload)
}

fn build_init_mp4(sps: &[u8], pps: &[u8], width: u32, height: u32) -> Vec<u8> {
    let ftyp = build_ftyp();
    let moov = build_moov(sps, pps, width, height);
    let mut out = Vec::with_capacity(ftyp.len() + moov.len());
    out.extend_from_slice(&ftyp);
    out.extend_from_slice(&moov);
    out
}

fn build_ftyp() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"isom");
    write_u32(&mut payload, 0x200);
    payload.extend_from_slice(b"isom");
    payload.extend_from_slice(b"iso6");
    payload.extend_from_slice(b"avc1");
    payload.extend_from_slice(b"cmfc");
    make_box(*b"ftyp", payload)
}

fn build_styp() -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"msdh");
    write_u32(&mut payload, 0);
    payload.extend_from_slice(b"msdh");
    payload.extend_from_slice(b"msix");
    payload.extend_from_slice(b"iso6");
    payload.extend_from_slice(b"avc1");
    payload.extend_from_slice(b"cmfc");
    make_box(*b"styp", payload)
}

fn build_moov(sps: &[u8], pps: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mvhd = build_mvhd();
    let trak = build_trak(sps, pps, width, height);
    let mvex = build_mvex();
    let mut payload = Vec::new();
    payload.extend_from_slice(&mvhd);
    payload.extend_from_slice(&trak);
    payload.extend_from_slice(&mvex);
    make_box(*b"moov", payload)
}

fn build_mvhd() -> Vec<u8> {
    let mut payload = Vec::with_capacity(100);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 90_000);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0x00010000);
    write_u16(&mut payload, 0x0100);
    write_u16(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_matrix(&mut payload);
    for _ in 0..6 {
        write_u32(&mut payload, 0);
    }
    write_u32(&mut payload, 2);
    make_box(*b"mvhd", payload)
}

fn build_trak(sps: &[u8], pps: &[u8], width: u32, height: u32) -> Vec<u8> {
    let tkhd = build_tkhd(width, height);
    let mdia = build_mdia(sps, pps, width, height);
    let mut payload = Vec::new();
    payload.extend_from_slice(&tkhd);
    payload.extend_from_slice(&mdia);
    make_box(*b"trak", payload)
}

fn build_tkhd(width: u32, height: u32) -> Vec<u8> {
    let mut payload = Vec::with_capacity(84);
    write_u32(&mut payload, 0x00000007);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 1);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u16(&mut payload, 0);
    write_u16(&mut payload, 0);
    write_u16(&mut payload, 0);
    write_u16(&mut payload, 0);
    write_matrix(&mut payload);
    write_u32(&mut payload, width << 16);
    write_u32(&mut payload, height << 16);
    make_box(*b"tkhd", payload)
}

fn build_mdia(sps: &[u8], pps: &[u8], width: u32, height: u32) -> Vec<u8> {
    let mdhd = build_mdhd();
    let hdlr = build_hdlr();
    let minf = build_minf(sps, pps, width, height);
    let mut payload = Vec::new();
    payload.extend_from_slice(&mdhd);
    payload.extend_from_slice(&hdlr);
    payload.extend_from_slice(&minf);
    make_box(*b"mdia", payload)
}

fn build_mdhd() -> Vec<u8> {
    let mut payload = Vec::with_capacity(32);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 90_000);
    write_u32(&mut payload, 0);
    write_u16(&mut payload, 0x55c4);
    write_u16(&mut payload, 0);
    make_box(*b"mdhd", payload)
}

fn build_hdlr() -> Vec<u8> {
    let mut payload = Vec::new();
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    payload.extend_from_slice(b"vide");
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    payload.extend_from_slice(b"VideoHandler");
    payload.push(0);
    make_box(*b"hdlr", payload)
}

fn build_minf(sps: &[u8], pps: &[u8], width: u32, height: u32) -> Vec<u8> {
    let vmhd = build_vmhd();
    let dinf = build_dinf();
    let stbl = build_stbl(sps, pps, width, height);
    let mut payload = Vec::new();
    payload.extend_from_slice(&vmhd);
    payload.extend_from_slice(&dinf);
    payload.extend_from_slice(&stbl);
    make_box(*b"minf", payload)
}

fn build_vmhd() -> Vec<u8> {
    let mut payload = Vec::new();
    write_u32(&mut payload, 0x00000001);
    write_u16(&mut payload, 0);
    write_u16(&mut payload, 0);
    write_u16(&mut payload, 0);
    write_u16(&mut payload, 0);
    make_box(*b"vmhd", payload)
}

fn build_dinf() -> Vec<u8> {
    let mut url = Vec::new();
    write_u32(&mut url, 0x00000001);
    let url_box = make_box(*b"url ", url);

    let mut dref = Vec::new();
    write_u32(&mut dref, 0);
    write_u32(&mut dref, 1);
    dref.extend_from_slice(&url_box);
    let dref_box = make_box(*b"dref", dref);

    let mut payload = Vec::new();
    payload.extend_from_slice(&dref_box);
    make_box(*b"dinf", payload)
}

fn build_stbl(sps: &[u8], pps: &[u8], width: u32, height: u32) -> Vec<u8> {
    let stsd = build_stsd(sps, pps, width, height);
    let stts = make_box(*b"stts", vec![0, 0, 0, 0, 0, 0, 0, 0]);
    let stsc = make_box(*b"stsc", vec![0, 0, 0, 0, 0, 0, 0, 0]);
    let stsz = make_box(*b"stsz", vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    let stco = make_box(*b"stco", vec![0, 0, 0, 0, 0, 0, 0, 0]);

    let mut payload = Vec::new();
    payload.extend_from_slice(&stsd);
    payload.extend_from_slice(&stts);
    payload.extend_from_slice(&stsc);
    payload.extend_from_slice(&stsz);
    payload.extend_from_slice(&stco);
    make_box(*b"stbl", payload)
}

fn build_stsd(sps: &[u8], pps: &[u8], width: u32, height: u32) -> Vec<u8> {
    let avc1 = build_avc1(sps, pps, width, height);
    let mut payload = Vec::new();
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 1);
    payload.extend_from_slice(&avc1);
    make_box(*b"stsd", payload)
}

fn build_avc1(sps: &[u8], pps: &[u8], width: u32, height: u32) -> Vec<u8> {
    let avcc = build_avcc(sps, pps);
    let mut payload = Vec::new();
    payload.extend_from_slice(&[0; 6]);
    write_u16(&mut payload, 1);
    write_u16(&mut payload, 0);
    write_u16(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u32(&mut payload, 0);
    write_u16(&mut payload, width as u16);
    write_u16(&mut payload, height as u16);
    write_u32(&mut payload, 0x00480000);
    write_u32(&mut payload, 0x00480000);
    write_u32(&mut payload, 0);
    write_u16(&mut payload, 1);
    payload.extend_from_slice(&[0; 32]);
    write_u16(&mut payload, 0x0018);
    write_u16(&mut payload, 0xffff);
    payload.extend_from_slice(&avcc);
    make_box(*b"avc1", payload)
}

fn build_avcc(sps: &[u8], pps: &[u8]) -> Vec<u8> {
    let profile_idc = sps.get(1).copied().unwrap_or(0);
    let profile_compat = sps.get(2).copied().unwrap_or(0);
    let level_idc = sps.get(3).copied().unwrap_or(0);
    let mut payload = Vec::new();
    payload.push(1);
    payload.push(profile_idc);
    payload.push(profile_compat);
    payload.push(level_idc);
    payload.push(0xFF);
    payload.push(0xE1);
    write_u16(&mut payload, sps.len() as u16);
    payload.extend_from_slice(sps);
    payload.push(1);
    write_u16(&mut payload, pps.len() as u16);
    payload.extend_from_slice(pps);
    make_box(*b"avcC", payload)
}

fn codec_string_from_sps(sps: &[u8]) -> String {
    let profile_idc = sps.get(1).copied().unwrap_or(0);
    let profile_compat = sps.get(2).copied().unwrap_or(0);
    let level_idc = sps.get(3).copied().unwrap_or(0);
    format!(
        "avc1.{:02X}{:02X}{:02X}",
        profile_idc, profile_compat, level_idc
    )
}

fn build_mvex() -> Vec<u8> {
    let mut trex = Vec::new();
    write_u32(&mut trex, 0);
    write_u32(&mut trex, 1);
    write_u32(&mut trex, 1);
    write_u32(&mut trex, 0);
    write_u32(&mut trex, 0);
    write_u32(&mut trex, 0x01010000);
    let trex_box = make_box(*b"trex", trex);
    make_box(*b"mvex", trex_box)
}

fn make_box(tag: [u8; 4], payload: Vec<u8>) -> Vec<u8> {
    let size = (payload.len() + 8) as u32;
    let mut out = Vec::with_capacity(payload.len() + 8);
    write_u32(&mut out, size);
    out.extend_from_slice(&tag);
    out.extend_from_slice(&payload);
    out
}

fn write_matrix(out: &mut Vec<u8>) {
    write_u32(out, 0x00010000);
    write_u32(out, 0);
    write_u32(out, 0);
    write_u32(out, 0);
    write_u32(out, 0x00010000);
    write_u32(out, 0);
    write_u32(out, 0);
    write_u32(out, 0);
    write_u32(out, 0x40000000);
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn parse_sps_dimensions(sps: &[u8]) -> Option<(u32, u32)> {
    if sps.len() < 2 {
        return None;
    }
    let rbsp = nal_to_rbsp(&sps[1..]);
    let mut br = BitReader::new(&rbsp);
    let profile_idc = br.read_bits(8)?;
    br.read_bits(8)?;
    br.read_bits(8)?;
    br.read_ue()?;

    let mut chroma_format_idc = 1u32;
    let mut _separate_colour_plane_flag = false;
    if matches!(
        profile_idc,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135 | 144
    ) {
        chroma_format_idc = br.read_ue()? as u32;
        if chroma_format_idc == 3 {
            _separate_colour_plane_flag = br.read_bit()?;
        }
        br.read_ue()?;
        br.read_ue()?;
        br.read_bit()?;
        if br.read_bit()? {
            let count = if chroma_format_idc == 3 { 12 } else { 8 };
            for i in 0..count {
                if br.read_bit()? {
                    skip_scaling_list(&mut br, if i < 6 { 16 } else { 64 })?;
                }
            }
        }
    }

    br.read_ue()?;
    let pic_order_cnt_type = br.read_ue()?;
    if pic_order_cnt_type == 0 {
        br.read_ue()?;
    } else if pic_order_cnt_type == 1 {
        br.read_bit()?;
        br.read_se()?;
        br.read_se()?;
        let cycle = br.read_ue()?;
        for _ in 0..cycle {
            br.read_se()?;
        }
    }
    br.read_ue()?;
    br.read_bit()?;
    let pic_width_in_mbs_minus1 = br.read_ue()? as u32;
    let pic_height_in_map_units_minus1 = br.read_ue()? as u32;
    let frame_mbs_only_flag = br.read_bit()?;
    if !frame_mbs_only_flag {
        br.read_bit()?;
    }
    br.read_bit()?;
    let frame_cropping_flag = br.read_bit()?;
    let (crop_left, crop_right, crop_top, crop_bottom) = if frame_cropping_flag {
        (
            br.read_ue()? as u32,
            br.read_ue()? as u32,
            br.read_ue()? as u32,
            br.read_ue()? as u32,
        )
    } else {
        (0, 0, 0, 0)
    };

    let width = (pic_width_in_mbs_minus1 + 1) * 16;
    let height = (pic_height_in_map_units_minus1 + 1) * 16 * if frame_mbs_only_flag { 1 } else { 2 };

    let (crop_unit_x, crop_unit_y) = if chroma_format_idc == 0 {
        (1, 2 - if frame_mbs_only_flag { 1 } else { 0 })
    } else if chroma_format_idc == 1 {
        (2, 2 * (2 - if frame_mbs_only_flag { 1 } else { 0 }))
    } else if chroma_format_idc == 2 {
        (2, 2 - if frame_mbs_only_flag { 1 } else { 0 })
    } else {
        (1, 2 - if frame_mbs_only_flag { 1 } else { 0 })
    };

    let width = width.saturating_sub((crop_left + crop_right) * crop_unit_x);
    let height = height.saturating_sub((crop_top + crop_bottom) * crop_unit_y);

    Some((width, height))
}

fn nal_to_rbsp(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    let mut zeros = 0u8;
    for &b in nal {
        if zeros >= 2 && b == 0x03 {
            zeros = 0;
            continue;
        }
        out.push(b);
        if b == 0 {
            zeros = zeros.saturating_add(1);
        } else {
            zeros = 0;
        }
    }
    out
}

struct BitReader<'a> {
    data: &'a [u8],
    byte: usize,
    bit: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, byte: 0, bit: 0 }
    }

    fn read_bit(&mut self) -> Option<bool> {
        if self.byte >= self.data.len() {
            return None;
        }
        let value = (self.data[self.byte] >> (7 - self.bit)) & 0x01;
        self.bit += 1;
        if self.bit >= 8 {
            self.bit = 0;
            self.byte += 1;
        }
        Some(value != 0)
    }

    fn read_bits(&mut self, count: u8) -> Option<u8> {
        let mut value = 0u8;
        for _ in 0..count {
            value <<= 1;
            value |= self.read_bit()? as u8;
        }
        Some(value)
    }

    fn read_ue(&mut self) -> Option<u32> {
        let mut zeros = 0u32;
        while let Some(bit) = self.read_bit() {
            if bit {
                break;
            }
            zeros += 1;
        }
        let mut value = 1u32;
        for _ in 0..zeros {
            value = (value << 1) | (self.read_bit()? as u32);
        }
        Some(value - 1)
    }

    fn read_se(&mut self) -> Option<i32> {
        let ue = self.read_ue()? as i32;
        let value = if ue % 2 == 0 {
            -(ue / 2)
        } else {
            (ue + 1) / 2
        };
        Some(value)
    }
}

fn skip_scaling_list(br: &mut BitReader<'_>, size: usize) -> Option<()> {
    let mut last = 8i32;
    let mut next = 8i32;
    for _ in 0..size {
        if next != 0 {
            let delta = br.read_se()? as i32;
            next = (last + delta + 256) % 256;
        }
        last = if next == 0 { last } else { next };
    }
    Some(())
}
