use crate::commands::CommandRequest;
use crate::config::Config;
use crate::state::PrinterState;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, TlsConfiguration, Transport};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

pub async fn run(
    config: Config,
    state: Arc<RwLock<PrinterState>>,
    mut command_rx: mpsc::Receiver<CommandRequest>,
) {
    let report_topic = format!("device/{}/report", config.printer_serial);
    let request_topic = format!("device/{}/request", config.printer_serial);
    let mut sequence_id: u64 = 1;

    loop {
        let mqtt_options = build_mqtt_options(&config);
        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

        if let Err(error) = client
            .subscribe(report_topic.clone(), QoS::AtMostOnce)
            .await
        {
            warn!(?error, "failed to subscribe to report topic");
            set_connected(&state, false).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        info!("mqtt connected, listening for reports");

        loop {
            tokio::select! {
                event = eventloop.poll() => {
                    match event {
                        Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                            set_connected(&state, true).await;
                        }
                        Ok(Event::Incoming(Incoming::Publish(publish))) => {
                            if let Ok(report) = serde_json::from_slice::<Value>(&publish.payload) {
                                let mut guard = state.write().await;
                                guard.connected = true;
                                guard.apply_report(&report);
                            } else {
                                warn!("failed to parse mqtt report payload");
                            }
                        }
                        Ok(_) => {}
                        Err(error) => {
                            warn!(?error, "mqtt connection error; reconnecting");
                            set_connected(&state, false).await;
                            break;
                        }
                    }
                }
                command = command_rx.recv() => {
                    let Some(command) = command else {
                        info!("command channel closed; shutting down mqtt task");
                        return;
                    };
                    let payload = command.to_payload(&config.mqtt_user_id, sequence_id);
                    sequence_id = sequence_id.wrapping_add(1);
                    let payload_bytes = match serde_json::to_vec(&payload) {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            warn!(?error, "failed to serialize command payload");
                            continue;
                        }
                    };

                    if let Err(error) = client
                        .publish(request_topic.clone(), QoS::AtLeastOnce, false, payload_bytes)
                        .await
                    {
                        warn!(?error, "failed to publish command");
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn build_mqtt_options(config: &Config) -> MqttOptions {
    let mut options = MqttOptions::new(
        config.mqtt_client_id.clone(),
        config.printer_host.clone(),
        config.mqtt_port,
    );
    options.set_credentials("bblp", &config.printer_access_code);
    options.set_keep_alive(Duration::from_secs(config.mqtt_keep_alive_secs));

    if config.mqtt_tls {
        let ca = config
            .mqtt_ca_cert
            .as_ref()
            .and_then(|path| std::fs::read(path).ok());
        options.set_transport(Transport::Tls(TlsConfiguration::Simple {
            ca,
            alpn: None,
            client_auth: None,
        }));
    }

    options
}

async fn set_connected(state: &Arc<RwLock<PrinterState>>, connected: bool) {
    let mut guard = state.write().await;
    guard.connected = connected;
    if !connected {
        guard.last_update = None;
    }
}
