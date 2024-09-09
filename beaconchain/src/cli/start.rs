use apibara_dna_common::{
    cli::{EtcdArgs, ObjectStoreArgs},
    ingestion::{ingestion_service_loop, IngestionServiceOptions},
    server::{server_loop, ServerArgs},
};
use clap::Args;
use error_stack::{Result, ResultExt};
use futures::TryFutureExt;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{cli::rpc::RpcArgs, error::BeaconChainError, ingestion::BeaconChainBlockIngestion};

#[derive(Args, Debug)]
pub struct StartCommand {
    #[clap(flatten)]
    rpc: RpcArgs,
    #[clap(flatten)]
    object_store: ObjectStoreArgs,
    #[clap(flatten)]
    etcd: EtcdArgs,

    // TODO: make this configurable.
    #[arg(long = "role.ingestion", default_value = "true")]
    role_ingestion: bool,

    #[clap(flatten)]
    server: ServerArgs,
}

impl StartCommand {
    pub async fn run(self, ct: CancellationToken) -> Result<(), BeaconChainError> {
        info!("Starting Beaconchain DNA server");
        let provider = self.rpc.to_beacon_api_provider()?;
        let object_store = self.object_store.into_object_store_client().await;
        let mut etcd_client = self
            .etcd
            .into_etcd_client()
            .await
            .change_context(BeaconChainError)?;

        let status_response = etcd_client
            .status()
            .await
            .change_context(BeaconChainError)?;

        info!(
            version = status_response.version(),
            "connected to etcd cluster"
        );

        let ingestion_options = IngestionServiceOptions {
            chain_segment_size: 1000,
            ..Default::default()
        };

        let ingestion_handle = if self.role_ingestion {
            let ingestion = BeaconChainBlockIngestion::new(provider);
            tokio::spawn(
                ingestion_service_loop(
                    ingestion,
                    etcd_client,
                    object_store,
                    ingestion_options,
                    ct.clone(),
                )
                .or_else(|err| async { Err(err).change_context(BeaconChainError) }),
            )
        } else {
            tokio::spawn({
                let ct = ct.clone();
                async move {
                    ct.cancelled().await;
                    Ok(())
                }
            })
        };

        let server_handle = if self.server.server_enabled {
            let options = self
                .server
                .to_server_options()
                .change_context(BeaconChainError)?;
            tokio::spawn(
                server_loop(options, ct)
                    .or_else(|err| async { Err(err).change_context(BeaconChainError) }),
            )
        } else {
            tokio::spawn({
                let ct = ct.clone();
                async move {
                    ct.cancelled().await;
                    Ok(())
                }
            })
        };

        match tokio::try_join!(ingestion_handle, server_handle).change_context(BeaconChainError)? {
            (Err(err), _) => return Err(err).change_context(BeaconChainError),
            (_, Err(err)) => return Err(err).change_context(BeaconChainError),
            _ => {}
        }

        Ok(())
    }
}
