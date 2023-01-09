//! First step of block ingestion.
use std::sync::Arc;

use apibara_node::db::libmdbx::EnvironmentKind;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::{
    core::{pb::starknet::v1alpha2::BlockStatus, GlobalBlockId},
    db::{DatabaseStorage, StorageReader, StorageWriter},
    ingestion::finalized::FinalizedBlockIngestion,
    provider::{BlockId, Provider},
};

use super::{
    accepted::AcceptedBlockIngestion, config::BlockIngestionConfig, downloader::Downloader,
    error::BlockIngestionError, subscription::IngestionStreamPublisher,
};

pub struct StartedBlockIngestion<G: Provider + Send, E: EnvironmentKind> {
    config: BlockIngestionConfig,
    provider: Arc<G>,
    downloader: Downloader<G>,
    storage: DatabaseStorage<E>,
    publisher: IngestionStreamPublisher,
}

impl<G, E> StartedBlockIngestion<G, E>
where
    G: Provider + Send,
    E: EnvironmentKind,
{
    pub fn new(
        provider: Arc<G>,
        storage: DatabaseStorage<E>,
        config: BlockIngestionConfig,
        publisher: IngestionStreamPublisher,
    ) -> Self {
        let downloader = Downloader::new(provider.clone(), config.rpc_concurrency);
        StartedBlockIngestion {
            config,
            provider,
            storage,
            downloader,
            publisher,
        }
    }

    pub async fn start(self, ct: CancellationToken) -> Result<(), BlockIngestionError> {
        loop {
            let latest_indexed = match self.storage.highest_accepted_block()? {
                Some(block) => block,
                None => self.ingest_genesis_block().await?,
            };

            info!(
                id = %latest_indexed,
                "latest indexed block"
            );

            // check if should jump to accepted ingestion directly based
            // on the status of the latest indexed block.
            let status = self.block_status(&latest_indexed).await?;
            if status.is_rejected() {
                // remove block from canonical chain (but not storage) and
                // try again.
                debug!(
                    id = %latest_indexed,
                    "block was rejected while offline"
                );
                let mut txn = self.storage.begin_txn()?;
                txn.shrink_canonical_chain(&latest_indexed)?;
                txn.commit()?;
            } else if status.is_accepted() {
                return self
                    .into_accepted_block_ingestion()
                    .start(latest_indexed, ct)
                    .await;
            } else {
                return self
                    .into_finalized_block_ingestion()
                    .start(latest_indexed, ct)
                    .await;
            }
        }
    }

    fn into_accepted_block_ingestion(self) -> AcceptedBlockIngestion<G, E> {
        AcceptedBlockIngestion::new(self.provider, self.storage, self.config, self.publisher)
    }

    fn into_finalized_block_ingestion(self) -> FinalizedBlockIngestion<G, E> {
        FinalizedBlockIngestion::new(self.provider, self.storage, self.config, self.publisher)
    }

    async fn block_status(
        &self,
        global_id: &GlobalBlockId,
    ) -> Result<BlockStatus, BlockIngestionError> {
        let block_id = BlockId::Hash(*global_id.hash());
        let (status, _header, _transactions) = self
            .provider
            .get_block(&block_id)
            .await
            .map_err(BlockIngestionError::provider)?;

        Ok(status)
    }

    #[tracing::instrument(skip(self))]
    async fn ingest_genesis_block(&self) -> Result<GlobalBlockId, BlockIngestionError> {
        info!("ingest genesis block");
        let block_id = BlockId::Number(0);
        let (status, header, body) = self
            .provider
            .get_block(&block_id)
            .await
            .map_err(BlockIngestionError::provider)?;

        let global_id = GlobalBlockId::from_block_header(&header)?;
        info!(id = %global_id, "genesis block");

        let mut txn = self.storage.begin_txn()?;
        self.downloader
            .finish_ingesting_block(&global_id, status, header, body, &mut txn)
            .await?;
        txn.extend_canonical_chain(&global_id)?;
        txn.commit()?;
        Ok(global_id)
    }
}
