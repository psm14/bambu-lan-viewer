#[derive(Debug, Clone)]
pub struct RtpPacket {
    pub payload_type: u8,
    pub marker: bool,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub payload: Vec<u8>,
}

impl RtpPacket {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 12 {
            return None;
        }
        let b0 = data[0];
        let b1 = data[1];
        let version = b0 >> 6;
        if version != 2 {
            return None;
        }

        let padding = (b0 & 0x20) != 0;
        let has_extension = (b0 & 0x10) != 0;
        let csrc_count = (b0 & 0x0F) as usize;

        let marker = (b1 & 0x80) != 0;
        let payload_type = b1 & 0x7F;

        let sequence_number = u16::from_be_bytes([data[2], data[3]]);
        let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let mut offset = 12 + csrc_count * 4;
        if has_extension {
            if data.len() < offset + 4 {
                return None;
            }
            let extension_length =
                u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            offset += 4 + extension_length * 4;
        }

        if data.len() < offset {
            return None;
        }

        let mut payload_end = data.len();
        if padding {
            if let Some(&pad) = data.last() {
                let pad_len = pad as usize;
                payload_end = payload_end.saturating_sub(pad_len);
            }
        }

        if payload_end < offset {
            return None;
        }

        let payload = data[offset..payload_end].to_vec();
        Some(Self {
            payload_type,
            marker,
            sequence_number,
            timestamp,
            ssrc,
            payload,
        })
    }
}
