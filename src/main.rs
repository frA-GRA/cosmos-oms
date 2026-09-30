mod bridge;
mod codec;

use std::sync::Arc;

use slog::error;

use rcal::cal;
use rcal::service::{AbstractService, AbstractServiceImpl};

#[rcal_macros::rcal_main]
async fn main() {
    let port: u16 = std::env::var("OMS_BRIDGE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7779);

    let config = Arc::new(rcal_config);

    let (tx, _) = tokio::sync::broadcast::channel::<Arc<Vec<u8>>>(256);

    if let Err(e) = bridge::start(port, tx.clone(), root_logger.clone()).await {
        error!(root_logger, "TCP bind failed"; "port" => port, "error" => %e);
        std::process::exit(1);
    }

    let mut cal = match cal::get_cal(
        "oms_ingest",
        None::<String>,
        Arc::clone(&config),
        root_logger.clone(),
    )
    .await
    {
        Ok(c) => c,
        Err(e) => {
            error!(root_logger, "CAL init failed"; "error" => %e);
            std::process::exit(1);
        }
    };

    let peer_uri = std::env::var("OMS_ZMQ_PEER").ok();
    if let Some(ref uri) = peer_uri {
        cal.add_receive_peer(uri);
    }

    let mut svc = AbstractServiceImpl::new(
        "oms_ingest",
        config.system.id.clone(),
        vec![],
        cal,
        Arc::clone(&config),
        root_logger.clone(),
    );

    if let Err(e) = svc.activate() {
        error!(root_logger, "service activate failed"; "error" => %e);
        std::process::exit(1);
    }

    include!(concat!(env!("OUT_DIR"), "/subscribers.rs"));

    tokio::signal::ctrl_c().await.ok();

    if let Err(e) = svc.deactivate() {
        error!(root_logger, "deactivate failed"; "error" => %e);
    }
}
