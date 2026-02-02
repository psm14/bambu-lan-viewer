use crate::commands::CommandRequest;
use crate::config::{AppConfig, PrinterConfig};
use crate::mqtt;
use crate::rtsp;
use crate::state::PrinterState;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, RwLock};
use tokio::task::AbortHandle;

#[derive(Debug)]
pub struct PrinterRuntime {
    pub config: PrinterConfig,
    pub state: Arc<RwLock<PrinterState>>,
    pub status_tx: watch::Sender<PrinterState>,
    pub command_tx: mpsc::Sender<CommandRequest>,
    pub hls_dir: PathBuf,
    mqtt_abort: AbortHandle,
    rtsp_abort: AbortHandle,
}

impl PrinterRuntime {
    pub fn spawn(config: PrinterConfig, settings: &AppConfig) -> Arc<Self> {
        let state = Arc::new(RwLock::new(PrinterState::default()));
        let (status_tx, _status_rx) = watch::channel(PrinterState::default());
        let (command_tx, command_rx) = mpsc::channel(32);
        let hls_dir = PathBuf::from(&settings.hls_output_dir).join(config.id.to_string());

        let mqtt_state = Arc::clone(&state);
        let mqtt_settings = settings.clone();
        let mqtt_config = config.clone();
        let mqtt_status_tx = status_tx.clone();
        let mqtt_handle = tokio::spawn(async move {
            mqtt::run(mqtt_settings, mqtt_config, mqtt_state, command_rx, mqtt_status_tx).await;
        });

        let video_settings = settings.clone();
        let video_config = config.clone();
        let video_state = Arc::clone(&state);
        let video_hls_dir = hls_dir.clone();
        let rtsp_handle = tokio::spawn(async move {
            rtsp::run_rtsp_hls(video_settings, video_config, video_state, video_hls_dir).await;
        });

        Arc::new(Self {
            config,
            state,
            status_tx,
            command_tx,
            hls_dir,
            mqtt_abort: mqtt_handle.abort_handle(),
            rtsp_abort: rtsp_handle.abort_handle(),
        })
    }

    pub fn shutdown(&self) {
        self.mqtt_abort.abort();
        self.rtsp_abort.abort();
    }
}
