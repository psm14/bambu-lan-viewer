use crate::commands::CommandRequest;
use crate::config::{AppConfig, PrinterConfig};
use crate::state::PrinterState;
use crate::tls;
use rand::distributions::Alphanumeric;
use rand::Rng;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS, TlsConfiguration, Transport};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, RwLock};
use tracing::{info, warn};

pub async fn run(
    settings: AppConfig,
    printer: PrinterConfig,
    state: Arc<RwLock<PrinterState>>,
    mut command_rx: mpsc::Receiver<CommandRequest>,
    status_tx: watch::Sender<PrinterState>,
) {
    let report_topic = format!("device/{}/report", printer.serial);
    let request_topic = format!("device/{}/request", printer.serial);
    let mut sequence_id: u64 = 1;

    loop {
        let mqtt_options = build_mqtt_options(&settings, &printer);
        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 10);

        if let Err(error) = client
            .subscribe(report_topic.clone(), QoS::AtMostOnce)
            .await
        {
            warn!(?error, "failed to subscribe to report topic");
            set_connected(&state, &status_tx, false).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }

        info!("mqtt connected, listening for reports");

        loop {
            tokio::select! {
                event = eventloop.poll() => {
                    match event {
                        Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                            set_connected(&state, &status_tx, true).await;
                        }
                        Ok(Event::Incoming(Incoming::Publish(publish))) => {
                            if let Ok(report) = serde_json::from_slice::<Value>(&publish.payload) {
                                let snapshot = {
                                    let mut guard = state.write().await;
                                    guard.connected = true;
                                    guard.apply_report(&report);
                                    guard.clone()
                                };
                                let _ = status_tx.send(snapshot);
                            } else {
                                warn!("failed to parse mqtt report payload");
                            }
                        }
                        Ok(_) => {}
                        Err(error) => {
                            warn!(?error, "mqtt connection error; reconnecting");
                            set_connected(&state, &status_tx, false).await;
                            break;
                        }
                    }
                }
                command = command_rx.recv() => {
                    let Some(command) = command else {
                        info!("command channel closed; shutting down mqtt task");
                        return;
                    };
                    let payload = command.to_payload(&settings.mqtt_user_id, sequence_id);
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

fn build_mqtt_options(config: &AppConfig, printer: &PrinterConfig) -> MqttOptions {
    let random_suffix: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect();
    let mut options = MqttOptions::new(
        format!(
            "{}-{}-{}",
            config.mqtt_client_id, printer.serial, random_suffix
        ),
        printer.host.clone(),
        config.mqtt_port,
    );
    options.set_credentials("bblp", &printer.access_code);
    options.set_keep_alive(Duration::from_secs(config.mqtt_keep_alive_secs));
    options.set_max_packet_size(
        config.mqtt_max_incoming_packet_size,
        config.mqtt_max_outgoing_packet_size,
    );

    if config.mqtt_tls {
        if config.mqtt_tls_insecure {
            warn!("mqtt tls verification disabled");
            let tls_config = tls::insecure_client_config();
            options.set_transport(Transport::Tls(TlsConfiguration::Rustls(Arc::new(
                tls_config,
            ))));
        } else if let Some(path) = config.mqtt_ca_cert.as_ref() {
            let ca = std::fs::read(path).unwrap_or_default();
            options.set_transport(Transport::Tls(TlsConfiguration::Simple {
                ca,
                alpn: None,
                client_auth: None,
            }));
        } else {
            options.set_transport(Transport::Tls(TlsConfiguration::default()));
        }
    }

    options
}

async fn set_connected(
    state: &Arc<RwLock<PrinterState>>,
    status_tx: &watch::Sender<PrinterState>,
    connected: bool,
) {
    let snapshot = {
        let mut guard = state.write().await;
        guard.connected = connected;
        if !connected {
            guard.last_update = None;
        }
        guard.clone()
    };
    let _ = status_tx.send(snapshot);
}
