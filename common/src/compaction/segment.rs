use apibara_observability::RecordRequest;
use error_stack::{Result, ResultExt};
use futures::{FutureExt, StreamExt};
use futures_buffered::FuturesOrderedBounded;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::{
    block_store::{BlockStoreWriter, UncachedBlockStoreReader},
    chain_view::{ChainView, NextCursor},
    ingestion::IngestionStateClient,
    Cursor,
};

use super::{metrics::CompactionMetrics, segment_builder::SegmentBuilder, CompactionError};

const MAX_BUFFERED_BLOCKS: usize = 128;

pub struct SegmentService {
    segment_size: usize,
    chain_view: ChainView,
    block_store_reader: UncachedBlockStoreReader,
    block_store_writer: BlockStoreWriter,
    state_client: IngestionStateClient,
    metrics: CompactionMetrics,
}

impl SegmentService {
    pub fn new(
        segment_size: usize,
        chain_view: ChainView,
        block_store_reader: UncachedBlockStoreReader,
        block_store_writer: BlockStoreWriter,
        state_client: IngestionStateClient,
        metrics: CompactionMetrics,
    ) -> Self {
        Self {
            segment_size,
            chain_view,
            block_store_reader,
            block_store_writer,
            state_client,
            metrics,
        }
    }

    pub async fn start(mut self, ct: CancellationToken) -> Result<(), CompactionError> {
        loop {
            if ct.is_cancelled() {
                return Ok(());
            }

            let first_block_in_segment = if let Some(cursor) = self
                .chain_view
                .get_segmented_cursor()
                .await
                .change_context(CompactionError)?
            {
                let NextCursor::Continue { cursor, .. } = self
                    .chain_view
                    .get_next_cursor(&Some(cursor.clone()))
                    .await
                    .change_context(CompactionError)?
                else {
                    return Err(CompactionError)
                        .attach_printable("chain view returned invalid cursor")
                        .attach_printable_lazy(|| format!("cursor: {cursor}"));
                };
                cursor
            } else {
                self.chain_view
                    .get_starting_cursor()
                    .await
                    .change_context(CompactionError)?
            };

            let head = self
                .chain_view
                .get_head()
                .await
                .change_context(CompactionError)?;
            let finalized = self
                .chain_view
                .get_finalized_cursor()
                .await
                .change_context(CompactionError)?;

            info!(
                next_cursor = %first_block_in_segment,
                head = %head,
                finalized = %finalized,
                "compaction: segment tick"
            );

            let latest_available = u64::min(finalized.number, head.number);

            if first_block_in_segment.number + self.segment_size as u64 <= latest_available {
                let creation_metrics = self.metrics.segment_creation.clone();
                self.compact_segment(first_block_in_segment)
                    .record_request(creation_metrics)
                    .await
                    .change_context(CompactionError)?;
            } else {
                let state_change = if finalized.number < head.number {
                    info!("compaction waiting for finalized change");
                    self.chain_view.finalized_changed().boxed()
                } else {
                    info!("compaction waiting for head change");
                    self.chain_view.head_changed().boxed()
                };

                tokio::pin!(state_change);

                let Some(_) = ct.run_until_cancelled(state_change).await else {
                    return Ok(());
                };
            }
        }
    }

    async fn compact_segment(
        &mut self,
        first_block_in_segment: Cursor,
    ) -> Result<(), CompactionError> {
        let mut builder = SegmentBuilder::default();
        let chain_view = &self.chain_view;

        info!(
            starting_cursor = %first_block_in_segment,
            "creating new segment"
        );

        builder
            .start_new_segment(first_block_in_segment.clone())
            .change_context(CompactionError)?;

        let buffered_queue_size = usize::min(self.segment_size, MAX_BUFFERED_BLOCKS);
        let mut block_queue = FuturesOrderedBounded::new(buffered_queue_size);

        let mut current = first_block_in_segment.clone();
        let mut last_block_in_segment = first_block_in_segment.clone();

        debug!(
            first_block = %first_block_in_segment,
            buffered_queue_size,
            "starting segment compaction"
        );

        for _ in 0..buffered_queue_size {
            let block_cursor = current.clone();
            let block_download_metrics = self.metrics.block_download.clone();
            block_queue.push_back(
                self.block_store_reader
                    .get_block_and_cursor(block_cursor)
                    .record_request(block_download_metrics),
            );

            let NextCursor::Continue {
                cursor: next_cursor,
                ..
            } = chain_view
                .get_next_cursor(&Some(current.clone()))
                .await
                .change_context(CompactionError)?
            else {
                return Err(CompactionError)
                    .attach_printable("chain view returned invalid next cursor")
                    .attach_printable_lazy(|| format!("cursor: {current}"));
            };

            debug!(current = %current, "compaction: pushed block to queue");

            current = next_cursor;
        }

        if self.segment_size > buffered_queue_size {
            debug!("compaction: queue full, waiting for blocks");

            for _ in buffered_queue_size..self.segment_size {
                let (block_cursor, block_data) = block_queue
                    .next()
                    .await
                    .ok_or(CompactionError)
                    .attach_printable("compaction segment buffer is empty")?
                    .change_context(CompactionError)
                    .attach_printable("failed to get block")?;

                builder
                    .add_block(&block_cursor, &block_data)
                    .change_context(CompactionError)
                    .attach_printable("failed to add block to segment")
                    .attach_printable_lazy(|| format!("cursor: {current}"))?;

                debug!(cursor = %block_cursor, "compaction: added block to segment");

                last_block_in_segment = block_cursor;

                let block_cursor = current.clone();
                let block_download_metrics = self.metrics.block_download.clone();
                block_queue.push_back(
                    self.block_store_reader
                        .get_block_and_cursor(block_cursor)
                        .record_request(block_download_metrics),
                );

                let NextCursor::Continue {
                    cursor: next_cursor,
                    ..
                } = chain_view
                    .get_next_cursor(&Some(current.clone()))
                    .await
                    .change_context(CompactionError)?
                else {
                    return Err(CompactionError)
                        .attach_printable("chain view returned invalid next cursor")
                        .attach_printable_lazy(|| format!("cursor: {current}"));
                };

                debug!(current = %current, "compaction: pushed block to queue");

                current = next_cursor;
            }
        }

        debug!("compaction: pushed all blocks to queue");

        for _ in 0..buffered_queue_size {
            let (block_cursor, block_data) = block_queue
                .next()
                .await
                .ok_or(CompactionError)
                .attach_printable("compaction segment buffer is empty")?
                .change_context(CompactionError)
                .attach_printable("failed to get block")?;

            builder
                .add_block(&block_cursor, &block_data)
                .change_context(CompactionError)
                .attach_printable("failed to add block to segment")
                .attach_printable_lazy(|| format!("cursor: {current}"))?;

            debug!(cursor = %block_cursor, "compaction: added block to segment");

            if block_cursor != first_block_in_segment
                && block_cursor.number != last_block_in_segment.number + 1
            {
                return Err(CompactionError)
                    .attach_printable("block cursor number does not match expected number")
                    .attach_printable_lazy(|| {
                        format!("cursor: {block_cursor}, last_block: {last_block_in_segment}")
                    });
            }

            last_block_in_segment = block_cursor;
        }

        // Sanity checks
        if builder.block_count() != self.segment_size {
            return Err(CompactionError)
                .attach_printable("builder block count does not match segment size")
                .attach_printable_lazy(|| {
                    format!(
                        "builder: {:}, segment: {:}",
                        builder.block_count(),
                        self.segment_size
                    )
                });
        }

        let segment_data = builder.segment_data().change_context(CompactionError)?;

        info!(
            first_block = %first_block_in_segment,
            last_block = %last_block_in_segment,
             "uploading segment to object store"
        );

        for segment in segment_data {
            use apibara_observability::KeyValue;

            let segment_name = segment.name.clone();

            self.metrics.segment_size.record(
                segment.data.len() as u64,
                &[KeyValue::new("name", segment_name.clone())],
            );

            self.block_store_writer
                .put_segment(&first_block_in_segment, segment)
                .record_request_with_attributes(
                    self.metrics.segment_upload.clone(),
                    &[KeyValue::new("name", segment_name)],
                )
                .await
                .change_context(CompactionError)
                .attach_printable("failed to put segment")?;
        }

        self.state_client
            .put_segmented(last_block_in_segment.number)
            .await
            .change_context(CompactionError)
            .attach_printable("failed to put segmented block")?;

        self.metrics
            .segmented
            .record(last_block_in_segment.number, &[]);

        Ok(())
    }
}
