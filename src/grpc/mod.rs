mod controller;
mod handler;
mod sequencer;
mod user_cache;

use crate::grpc::handler::Handler;
use crate::state::GlobalState;
use orchestra::rpc::rollup::rollup_state_server::RollupStateServer;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

pub fn run_grpc_server(addr: SocketAddr, state: Arc<RwLock<GlobalState>>) -> anyhow::Result<()> {
    let rt: tokio::runtime::Runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Build runtime");

    rt.block_on(async {
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            log::info!("Ctrl-C received, shutting down");
            tx.send(()).ok();
        });

        let handler = Handler::new(state).await;

        tonic::transport::Server::builder()
            .add_service(RollupStateServer::new(handler))
            .serve_with_shutdown(addr, async {
                rx.await.ok();
            })
            .await?;

        Ok(())
    })
}
