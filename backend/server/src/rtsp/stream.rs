use bytes::Bytes;
use tokio::sync::{broadcast, watch};

#[derive(Clone, Debug)]
pub struct CmafInit {
    pub bytes: Bytes,
    pub codec: String,
}

#[derive(Clone, Debug)]
pub struct CmafStream {
    init_tx: watch::Sender<Option<CmafInit>>,
    fragment_tx: broadcast::Sender<Bytes>,
}

pub struct CmafStreamSubscription {
    pub init_rx: watch::Receiver<Option<CmafInit>>,
    pub fragment_rx: broadcast::Receiver<Bytes>,
}

impl CmafStream {
    pub fn new() -> Self {
        let (init_tx, _init_rx) = watch::channel(None);
        let (fragment_tx, _fragment_rx) = broadcast::channel(64);
        Self {
            init_tx,
            fragment_tx,
        }
    }

    pub fn subscribe(&self) -> CmafStreamSubscription {
        CmafStreamSubscription {
            init_rx: self.init_tx.subscribe(),
            fragment_rx: self.fragment_tx.subscribe(),
        }
    }

    pub fn update_init(&self, init: CmafInit) {
        let _ = self.init_tx.send(Some(init));
    }

    pub fn send_fragment(&self, fragment: Bytes) {
        let _ = self.fragment_tx.send(fragment);
    }
}
