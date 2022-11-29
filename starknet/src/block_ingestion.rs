//! Ingest blocks from the node.

use std::{sync::Arc, time::Duration};

use apibara_core::stream::{RawMessageData, Sequence, StreamMessage};
use apibara_node::{
    chain_tracker::{ChainChange, ChainTracker, ChainTrackerError},
    db::libmdbx::EnvironmentKind,
    message_stream::{self, BackfilledMessageStream},
    o11y::{self, ObservableCounter, ObservableGauge},
};
use chrono::{DateTime, Utc};
use futures::{Stream, TryStreamExt};
use prost::Message;
use starknet::providers::SequencerGatewayProvider;
use tokio::sync::broadcast::{self, error::SendError};
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::{
    block_builder::{BlockBuilder, BlockBuilderError},
    core::Block,
    pb::BlockHash,
};

pub type BlockStreamMessage = StreamMessage<Block>;

pub struct BlockIngestor<E: EnvironmentKind> {
    chain: Arc<ChainTracker<Block, E>>,
    block_builder: BlockBuilder,
    block_tx: broadcast::Sender<BlockStreamMessage>,
    _block_rx: broadcast::Receiver<BlockStreamMessage>,
    metrics: Metrics,
}

#[derive(Debug, thiserror::Error)]
pub enum BlockIngestorError {
    #[error("error tracking chain state")]
    ChainTracker(#[from] ChainTrackerError),
    #[error("error fetching or parsing block")]
    BlockBuilder(#[from] BlockBuilderError),
    #[error("error broadcasting chain event")]
    Broadcast(#[from] SendError<BlockStreamMessage>),
    #[error("chain not started syncing")]
    EmptyChain,
    #[error("chain is missing a block")]
    MissingBlock { block_number: u64 },
}

pub type Result<T> = std::result::Result<T, BlockIngestorError>;

const MESSAGE_CHANNEL_SIZE: usize = 128;

lazy_static::lazy_static! {
    static ref FAR_HEAD_REFRESH_INTERVAL: chrono::Duration = chrono::Duration::from_std(Duration::from_secs(60)).expect("far head refresh interval");
    static ref CLOSE_HEAD_REFRESH_INTERVAL: chrono::Duration = chrono::Duration::from_std(Duration::from_secs(10)).expect("close head refresh interval");
}

pub struct Metrics {
    ingested_blocks: ObservableCounter<u64>,
    latest_block: ObservableGauge<u64>,
}

/// Tracks ingestor state.
#[derive(Debug)]
struct LoopState {
    head_refreshed_at: DateTime<Utc>,
    current_block_number: u64,
    sync_sleep_interval: Duration,
}

impl<E> BlockIngestor<E>
where
    E: EnvironmentKind,
{
    pub fn new(
        chain: Arc<ChainTracker<Block, E>>,
        client: Arc<SequencerGatewayProvider>,
    ) -> Result<Self> {
        let block_builder = BlockBuilder::new(client);
        let (block_tx, block_rx) = broadcast::channel(MESSAGE_CHANNEL_SIZE);
        let metrics = Metrics::new();

        Ok(BlockIngestor {
            chain,
            block_builder,
            block_tx,
            _block_rx: block_rx,
            metrics,
        })
    }

    /// Creates a new stream of live blockchain blocks and reorgs.
    pub fn live_stream(
        &self,
    ) -> impl Stream<Item = std::result::Result<BlockStreamMessage, Box<dyn std::error::Error>>>
    {
        let receiver = self.block_tx.subscribe();
        BroadcastStream::new(receiver).map_err(|err| Box::new(err) as Box<dyn std::error::Error>)
    }

    pub fn stream_from_sequence(
        &self,
        starting_sequence: u64,
        pending_interval: Option<Duration>,
        ct: CancellationToken,
    ) -> Result<impl Stream<Item = message_stream::Result<StreamMessage<Block>>>> {
        info!(start = %starting_sequence, "start stream");
        let indexed = self
            .chain
            .latest_indexed_block()?
            .ok_or(BlockIngestorError::EmptyChain)?
            .block_number;

        let current = Sequence::from_u64(starting_sequence);
        let latest = Sequence::from_u64(indexed);
        let live = self.live_stream();
        Ok(BackfilledMessageStream::new(
            current,
            latest,
            self.chain.clone(),
            live,
            pending_interval,
            ct,
        ))
    }

    pub async fn start(&self, ct: CancellationToken, poll_interval: Duration) -> Result<()> {
        let current_head = self
            .block_builder
            .latest_block_with_backoff(ct.clone())
            .await?;
        self.chain.update_head(&current_head)?;
        info!(
            hash = %current_head.block_hash.unwrap_or_default(),
            number = %current_head.block_number,
            "updated head"
        );

        let mut starting_block_number = 0;
        if let Some(latest_block) = self.chain.latest_indexed_block()? {
            info!("check shrunk reorg while offline");

            if current_head.block_number < latest_block.block_number {
                info!(
                    head = %current_head.block_number,
                    latest = %latest_block.block_number,
                    "chain shrunk. invalidate"
                );
                self.chain.invalidate(current_head.block_number)?;
            }
        }

        if let Some(latest_block) = self.chain.latest_indexed_block()? {
            info!("check reorg while offline");

            let block = self
                .block_builder
                .block_by_number_with_backoff(latest_block.block_number, ct.clone())
                .await?;

            if block.block_hash != latest_block.block_hash {
                let stored_block_hash = latest_block.block_hash.unwrap_or_default();
                let stored_block_height = latest_block.block_number;

                let chain_block_hash = block.block_hash.unwrap_or_default();
                let chain_block_height = block.block_number;

                warn!(
                    stored_block_hash = %stored_block_hash,
                    stored_block_height = %stored_block_height,
                    chain_block_hash = %chain_block_hash,
                    chain_block_height = %chain_block_height,
                    "reorg while offline. start recovery"
                );

                let mut stored_block_number = latest_block.block_number;
                loop {
                    if stored_block_number == 0 {
                        unreachable!("reached block 0 while checking for offline reorg");
                    }

                    let stored_block = self.chain.block_by_number(stored_block_number - 1)?.ok_or(
                        BlockIngestorError::MissingBlock {
                            block_number: stored_block_number - 1,
                        },
                    )?;
                    let chain_block = self
                        .block_builder
                        .block_by_number_with_backoff(stored_block.block_number, ct.clone())
                        .await?;

                    if stored_block.block_hash == chain_block.block_hash {
                        let block_hash = stored_block.block_hash.unwrap_or_default();

                        info!(
                            block_number = %stored_block.block_number,
                            block_hash = %block_hash,
                            "found common ancestor. invalidating data"
                        );

                        self.chain.invalidate(stored_block.block_number + 1)?;

                        starting_block_number = stored_block.block_number + 1;
                        break;
                    }

                    let stored_block_hash = stored_block.block_hash.unwrap_or_default();
                    let chain_block_hash = chain_block.block_hash.unwrap_or_default();
                    info!(
                        block_number = %stored_block.block_number,
                        stored_block_hash = %stored_block_hash,
                        chain_block_hash = %chain_block_hash,
                        "blocks did not match"
                    );

                    stored_block_number = stored_block.block_number;
                }
            } else {
                starting_block_number = latest_block.block_number + 1;
            }
        }

        info!(block_number = %starting_block_number, "starting block ingestion");

        let mut loop_state = LoopState::new(starting_block_number, poll_interval);

        loop {
            if ct.is_cancelled() {
                break;
            }

            match self.chain.gap()? {
                None => {
                    self.fetch_initial_block(&mut loop_state, &ct).await?;
                }
                Some(0) => {
                    self.fetch_block_at_head(&mut loop_state, &ct).await?;
                }
                Some(gap) => {
                    self.fetch_lagging_block(&mut loop_state, gap, &ct).await?;
                }
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip(self, ct))]
    async fn fetch_initial_block(
        &self,
        state: &mut LoopState,
        ct: &CancellationToken,
    ) -> Result<()> {
        debug!(block_number = %state.current_block_number, "gap is none");
        let (current_block_number, _) = self
            .fetch_and_broadcast_block(state.current_block_number, ct)
            .await?;
        state.set_current_block_number(current_block_number);
        Ok(())
    }

    #[tracing::instrument(skip(self, ct))]
    async fn fetch_block_at_head(
        &self,
        state: &mut LoopState,
        ct: &CancellationToken,
    ) -> Result<()> {
        let head_height = self
            .chain
            .head_height()?
            .ok_or(BlockIngestorError::EmptyChain)?;
        debug!(block_number = %head_height, "gap is 0");

        let (next_block_number, current_block_hash) =
            self.fetch_and_broadcast_latest_block(ct).await?;

        state.set_current_block_number(next_block_number);
        let pending_block = self.block_builder.fetch_pending_block().await?;

        if let Some(mut pending_block) = pending_block {
            if current_block_hash.is_some() && pending_block.parent_block_hash == current_block_hash
            {
                info!("pending block");
                // change pending block number to the next block's height.
                pending_block.block_number = head_height + 1;
                let sequence = Sequence::from_u64(pending_block.block_number);
                let raw_block = RawMessageData::from_vec(pending_block.encode_to_vec());
                let message = BlockStreamMessage::new_pending(sequence, raw_block);
                self.block_tx.send(message)?;
            }
        }
        if state.current_block_number == head_height + 1 {
            tokio::time::sleep(state.sync_sleep_interval).await;
        }
        Ok(())
    }

    #[tracing::instrument(skip(self, ct))]
    async fn fetch_lagging_block(
        &self,
        state: &mut LoopState,
        gap: u64,
        ct: &CancellationToken,
    ) -> Result<()> {
        let head_refresh_elapsed = chrono::offset::Utc::now() - state.head_refreshed_at;

        let should_refresh_head = (gap > 50 && head_refresh_elapsed > *FAR_HEAD_REFRESH_INTERVAL)
            || (gap > 10 && head_refresh_elapsed > *CLOSE_HEAD_REFRESH_INTERVAL);

        if should_refresh_head {
            debug!("refresh head");
            let current_head = self
                .block_builder
                .latest_block_with_backoff(ct.clone())
                .await?;
            self.chain.update_head(&current_head)?;
            state.set_head_refreshed_at_now();
        }

        debug!(block_number = %state.current_block_number, gap = %gap, "has gap");
        let (current_block_number, _) = self
            .fetch_and_broadcast_block(state.current_block_number, ct)
            .await?;
        state.set_current_block_number(current_block_number);

        Ok(())
    }

    #[tracing::instrument(skip(self, ct))]
    async fn fetch_and_broadcast_latest_block(
        &self,
        ct: &CancellationToken,
    ) -> Result<(u64, Option<BlockHash>)> {
        let block = tokio::select! {
            block = self.block_builder.latest_block_with_backoff(ct.clone()) => {
                block?
            }
            _ = ct.cancelled() => {
                return Ok((0, None))
            }
        };

        self.apply_block(block)
    }

    #[tracing::instrument(skip(self, ct))]
    async fn fetch_and_broadcast_block(
        &self,
        block_number: u64,
        ct: &CancellationToken,
    ) -> Result<(u64, Option<BlockHash>)> {
        let block = tokio::select! {
            block = self.block_builder.block_by_number_with_backoff(block_number, ct.clone()) => {
                block?
            }
            _ = ct.cancelled() => {
                return Ok((0, None))
            }
        };

        self.apply_block(block)
    }

    #[tracing::instrument(skip(self, block))]
    fn apply_block(&self, block: Block) -> Result<(u64, Option<BlockHash>)> {
        info!(block_number = %block.block_number, "got block");
        let block_number = block.block_number;
        self.metrics.observe_ingested_block();

        let mut current_block_hash = block.block_hash.clone();
        match self.chain.update_indexed_block(block)? {
            ChainChange::Advance(blocks) => {
                info!("chain advanced by {} blocks", blocks.len());
                let mut next_block_number = block_number + 1;
                for block in blocks {
                    next_block_number = block.block_number + 1;
                    let sequence = Sequence::from_u64(block.block_number);
                    current_block_hash = block.block_hash.clone();
                    let raw_block = RawMessageData::from_vec(block.encode_to_vec());
                    let message = BlockStreamMessage::new_data(sequence, raw_block);
                    self.block_tx.send(message)?;
                }
                self.metrics.observe_latest_block(next_block_number - 1);
                Ok((next_block_number, current_block_hash))
            }
            ChainChange::Reorg(blocks) => {
                info!("chain reorged by {} blocks", blocks.len());
                todo!()
            }
            ChainChange::MissingBlock(block_number, block_hash) => {
                info!("block is missing: {}/{}", block_number, block_hash);
                todo!()
            }
            ChainChange::AlreadySeen => {
                info!("block already seen");
                Ok((block_number + 1, current_block_hash))
            }
        }
    }
}

impl LoopState {
    pub fn new(starting_block_number: u64, sync_sleep_interval: Duration) -> LoopState {
        let head_refreshed_at = chrono::offset::Utc::now();
        LoopState {
            head_refreshed_at,
            current_block_number: starting_block_number,
            sync_sleep_interval,
        }
    }

    pub fn set_current_block_number(&mut self, block: u64) {
        self.current_block_number = block;
    }

    pub fn set_head_refreshed_at_now(&mut self) {
        self.head_refreshed_at = chrono::offset::Utc::now();
    }
}

impl Metrics {
    pub fn new() -> Metrics {
        let meter = o11y::meter("apibara.com/starknet");
        let ingested_blocks = meter
            .u64_observable_counter("ingested_blocks")
            .with_description("The number of ingested blocks")
            .init();
        let latest_block = meter
            .u64_observable_gauge("latest_block")
            .with_description("The sequence number of the latest ingested block")
            .init();
        Metrics {
            ingested_blocks,
            latest_block,
        }
    }

    pub fn observe_ingested_block(&self) {
        let cx = o11y::Context::current();
        self.ingested_blocks.observe(&cx, 1, &[]);
    }

    pub fn observe_latest_block(&self, block: u64) {
        let cx = o11y::Context::current();
        self.latest_block.observe(&cx, block, &[]);
    }
}
