use bytes::Bytes;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, watch};

#[derive(Clone, Debug)]
pub struct CmafInit {
    pub bytes: Bytes,
    pub codec: String,
}

#[derive(Clone, Debug)]
pub struct CmafFragment {
    pub seq: u64,
    pub bytes: Bytes,
}

#[derive(Clone, Debug)]
pub struct CmafStream {
    init_tx: watch::Sender<Option<CmafInit>>,
    fragment_tx: broadcast::Sender<CmafFragment>,
    backlog: Arc<Mutex<VecDeque<CmafFragment>>>,
    next_seq: Arc<AtomicU64>,
    backlog_capacity: usize,
}

pub struct CmafStreamSubscription {
    pub init_rx: watch::Receiver<Option<CmafInit>>,
    pub fragment_rx: broadcast::Receiver<CmafFragment>,
}

impl CmafStream {
    pub fn new(backlog_capacity: usize) -> Self {
        let (init_tx, _init_rx) = watch::channel(None);
        let (fragment_tx, _fragment_rx) = broadcast::channel(64);
        let capacity = backlog_capacity.max(1);
        Self {
            init_tx,
            fragment_tx,
            backlog: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            next_seq: Arc::new(AtomicU64::new(1)),
            backlog_capacity: capacity,
        }
    }

    pub fn subscribe(&self) -> CmafStreamSubscription {
        CmafStreamSubscription {
            init_rx: self.init_tx.subscribe(),
            fragment_rx: self.fragment_tx.subscribe(),
        }
    }

    pub fn update_init(&self, init: CmafInit) {
        self.init_tx.send_replace(Some(init));
    }

    pub fn send_fragment(&self, fragment: Bytes) {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed);
        let entry = CmafFragment {
            seq,
            bytes: fragment,
        };
        if let Ok(mut backlog) = self.backlog.lock() {
            backlog.push_back(entry.clone());
            while backlog.len() > self.backlog_capacity {
                backlog.pop_front();
            }
        }
        let _ = self.fragment_tx.send(entry);
    }

    pub fn backlog_snapshot(&self) -> Vec<CmafFragment> {
        self.backlog
            .lock()
            .map(|backlog| backlog.iter().cloned().collect())
            .unwrap_or_default()
    }
}
