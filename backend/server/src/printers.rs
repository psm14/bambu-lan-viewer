use crate::commands::CommandRequest;
use crate::config::{AppConfig, PrinterConfig};
use crate::mqtt;
use crate::rtsp;
use crate::rtsp::CmafStream;
use crate::state::PrinterState;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, watch, RwLock};
use tokio::task::AbortHandle;

#[derive(Debug)]
pub struct PrinterRuntime {
    pub state: Arc<RwLock<PrinterState>>,
    pub status_tx: watch::Sender<PrinterState>,
    pub command_tx: mpsc::Sender<CommandRequest>,
    pub cmaf_dir: PathBuf,
    pub cmaf_stream: CmafStream,
    mqtt_abort: AbortHandle,
    rtsp_abort: AbortHandle,
}

impl PrinterRuntime {
    pub fn spawn(config: PrinterConfig, settings: &AppConfig) -> Arc<Self> {
        let state = Arc::new(RwLock::new(PrinterState::default()));
        let (status_tx, _status_rx) = watch::channel(PrinterState::default());
        let (command_tx, command_rx) = mpsc::channel(32);
        let cmaf_dir = PathBuf::from(&settings.cmaf_output_dir).join(config.id.to_string());
        let part_duration = if settings.cmaf_part_duration_secs > 0.0 {
            settings.cmaf_part_duration_secs
        } else if settings.cmaf_target_duration_secs > 0.0 {
            settings.cmaf_target_duration_secs
        } else {
            0.25
        };
        let backlog_capacity =
            ((settings.cmaf_ws_backlog_secs / part_duration).ceil() as usize).clamp(1, 240);
        let cmaf_stream = CmafStream::new(backlog_capacity);

        let mqtt_state = Arc::clone(&state);
        let mqtt_settings = settings.clone();
        let mqtt_config = config.clone();
        let mqtt_status_tx = status_tx.clone();
        let mqtt_handle = tokio::spawn(async move {
            mqtt::run(
                mqtt_settings,
                mqtt_config,
                mqtt_state,
                command_rx,
                mqtt_status_tx,
            )
            .await;
        });

        let video_settings = settings.clone();
        let video_config = config.clone();
        let video_state = Arc::clone(&state);
        let video_cmaf_dir = cmaf_dir.clone();
        let video_stream = cmaf_stream.clone();
        let rtsp_handle = tokio::spawn(async move {
            rtsp::run_rtsp_hls(
                video_settings,
                video_config,
                video_state,
                video_cmaf_dir,
                video_stream,
            )
            .await;
        });

        Arc::new(Self {
            state,
            status_tx,
            command_tx,
            cmaf_dir,
            cmaf_stream,
            mqtt_abort: mqtt_handle.abort_handle(),
            rtsp_abort: rtsp_handle.abort_handle(),
        })
    }

    pub fn shutdown(&self) {
        self.mqtt_abort.abort();
        self.rtsp_abort.abort();
    }
}
