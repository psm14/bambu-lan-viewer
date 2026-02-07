use crate::rtsp::rtp::RtpPacket;

#[derive(Debug, Clone)]
pub struct AccessUnit {
    pub nals: Vec<Vec<u8>>,
    pub rtp_timestamp: u32,
    pub is_idr: bool,
}

pub struct H264RtpDepacketizer {
    current_access_unit: Vec<Vec<u8>>,
    current_timestamp: Option<u32>,
    current_access_unit_bytes: usize,
    fu_buffer: Option<Vec<u8>>,
    fu_sequence: Option<u16>,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
    parameter_sets_dirty: bool,
}

impl H264RtpDepacketizer {
    pub fn new() -> Self {
        Self {
            current_access_unit: Vec::new(),
            current_timestamp: None,
            current_access_unit_bytes: 0,
            fu_buffer: None,
            fu_sequence: None,
            sps: None,
            pps: None,
            parameter_sets_dirty: false,
        }
    }

    pub fn take_parameter_sets(&mut self) -> Option<(Vec<u8>, Vec<u8>)> {
        if self.parameter_sets_dirty {
            self.parameter_sets_dirty = false;
            if let (Some(sps), Some(pps)) = (self.sps.clone(), self.pps.clone()) {
                return Some((sps, pps));
            }
        }
        None
    }

    pub fn handle(&mut self, packet: &RtpPacket) -> Vec<AccessUnit> {
        let mut output = Vec::new();

        if let Some(current_ts) = self.current_timestamp {
            if current_ts != packet.timestamp && !self.current_access_unit.is_empty() {
                output.push(self.build_access_unit(current_ts));
            }
        }

        let nals = self.extract_nals(packet);
        for nal in nals {
            self.append_nal(nal, packet.timestamp);
            if self.current_access_unit_bytes >= MAX_ACCESS_UNIT_BYTES {
                if let Some(ts) = self.current_timestamp {
                    tracing::warn!(
                        bytes = self.current_access_unit_bytes,
                        "rtp access unit exceeded size limit; forcing flush"
                    );
                    output.push(self.build_access_unit(ts));
                }
            }
        }

        if packet.marker && self.current_timestamp.is_some() && !self.current_access_unit.is_empty()
        {
            let ts = self.current_timestamp.unwrap_or(packet.timestamp);
            output.push(self.build_access_unit(ts));
        }

        output
    }

    fn build_access_unit(&mut self, timestamp: u32) -> AccessUnit {
        let nals = std::mem::take(&mut self.current_access_unit);
        self.current_timestamp = None;
        self.current_access_unit_bytes = 0;
        let is_idr = nals
            .iter()
            .any(|nal| nal.first().map(|b| b & 0x1F) == Some(5));
        AccessUnit {
            nals,
            rtp_timestamp: timestamp,
            is_idr,
        }
    }

    fn append_nal(&mut self, nal: Vec<u8>, timestamp: u32) {
        if self.current_timestamp.is_none() {
            self.current_timestamp = Some(timestamp);
        }

        if let Some(nal_type) = nal.first().map(|b| b & 0x1F) {
            if nal_type == 7 {
                self.sps = Some(nal.clone());
                self.parameter_sets_dirty = self.pps.is_some();
            } else if nal_type == 8 {
                self.pps = Some(nal.clone());
                self.parameter_sets_dirty = self.sps.is_some();
            }
        }

        self.current_access_unit_bytes = self
            .current_access_unit_bytes
            .saturating_add(nal.len());
        self.current_access_unit.push(nal);
    }

    fn extract_nals(&mut self, packet: &RtpPacket) -> Vec<Vec<u8>> {
        let payload = &packet.payload;
        if payload.is_empty() {
            return Vec::new();
        }
        let nal_type = payload[0] & 0x1F;
        match nal_type {
            1..=23 => vec![payload.clone()],
            24 => self.extract_stap_a(payload),
            28 => self.extract_fu_a(payload, packet.sequence_number),
            _ => Vec::new(),
        }
    }

    fn extract_stap_a(&self, payload: &[u8]) -> Vec<Vec<u8>> {
        if payload.len() <= 1 {
            return Vec::new();
        }
        let mut index = 1;
        let mut nals = Vec::new();
        while index + 2 <= payload.len() {
            let size = u16::from_be_bytes([payload[index], payload[index + 1]]) as usize;
            index += 2;
            if index + size > payload.len() {
                break;
            }
            nals.push(payload[index..index + size].to_vec());
            index += size;
        }
        nals
    }

    fn extract_fu_a(&mut self, payload: &[u8], sequence: u16) -> Vec<Vec<u8>> {
        if payload.len() <= 2 {
            return Vec::new();
        }
        let fu_indicator = payload[0];
        let fu_header = payload[1];
        let start = (fu_header & 0x80) != 0;
        let end = (fu_header & 0x40) != 0;
        let nal_type = fu_header & 0x1F;
        let nal_header = (fu_indicator & 0xE0) | nal_type;

        if start {
            let mut buffer = Vec::with_capacity(payload.len());
            buffer.push(nal_header);
            buffer.extend_from_slice(&payload[2..]);
            self.fu_buffer = Some(buffer);
            self.fu_sequence = Some(sequence);
            return Vec::new();
        }

        let expected_sequence = self.fu_sequence.map(|seq| seq.wrapping_add(1));
        if let Some(expected) = expected_sequence {
            if sequence != expected {
                self.fu_buffer = None;
                self.fu_sequence = None;
                return Vec::new();
            }
        }

        if let Some(buffer) = self.fu_buffer.as_mut() {
            buffer.extend_from_slice(&payload[2..]);
            if buffer.len() > MAX_FU_BUFFER_BYTES {
                tracing::warn!(
                    bytes = buffer.len(),
                    "rtp fu-a buffer exceeded size limit; dropping"
                );
                self.fu_buffer = None;
                self.fu_sequence = None;
                return Vec::new();
            }
        } else {
            return Vec::new();
        }
        self.fu_sequence = Some(sequence);

        if end {
            self.fu_sequence = None;
            return self
                .fu_buffer
                .take()
                .map(|data| vec![data])
                .unwrap_or_default();
        }

        Vec::new()
    }
}

const MAX_ACCESS_UNIT_BYTES: usize = 8 * 1024 * 1024;
const MAX_FU_BUFFER_BYTES: usize = 4 * 1024 * 1024;
