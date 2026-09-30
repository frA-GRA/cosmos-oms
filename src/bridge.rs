use slog::{error, info, warn};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

pub type PacketTx = broadcast::Sender<Arc<Vec<u8>>>;

pub async fn start(
    port: u16,
    tx: PacketTx,
    logger: slog::Logger,
) -> std::io::Result<tokio::task::JoinHandle<()>> {
    let addr: SocketAddr = format!("0.0.0.0:{port}").parse().expect("valid addr");
    let listener = TcpListener::bind(addr).await?;
    info!(logger, "OMS bridge listening"; "port" => port);

    Ok(tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    info!(logger, "client connected"; "peer" => %peer);
                    let mut rx = tx.subscribe();
                    let client_log = logger.clone();
                    tokio::spawn(async move {
                        let (_, mut writer) = stream.into_split();
                        loop {
                            match rx.recv().await {
                                Ok(pkt) => {
                                    if let Err(e) = writer.write_all(&pkt).await {
                                        warn!(client_log, "write failed, dropping client";
                                            "peer" => %peer, "error" => %e);
                                        break;
                                    }
                                }
                                Err(broadcast::error::RecvError::Lagged(n)) => {
                                    warn!(client_log, "client lagged, dropped packets";
                                        "peer" => %peer, "dropped" => n);
                                }
                                Err(broadcast::error::RecvError::Closed) => break,
                            }
                        }
                        info!(client_log, "client disconnected"; "peer" => %peer);
                    });
                }
                Err(e) => {
                    error!(logger, "accept error"; "error" => %e);
                }
            }
        }
    }))
}
