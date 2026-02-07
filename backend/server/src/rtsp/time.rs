#[derive(Debug, Default)]
pub struct RtpTimeMapper {
    first_timestamp: Option<u32>,
}

impl RtpTimeMapper {
    pub fn new() -> Self {
        Self {
            first_timestamp: None,
        }
    }

    pub fn pts90k(&mut self, rtp_timestamp: u32) -> u64 {
        if self.first_timestamp.is_none() {
            self.first_timestamp = Some(rtp_timestamp);
        }
        let base = self.first_timestamp.unwrap_or(rtp_timestamp);
        let delta = rtp_timestamp.wrapping_sub(base);
        delta as u64
    }
}
