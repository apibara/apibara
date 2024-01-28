use apibara_dna_common::{error::Result, segment::SegmentOptions, storage::StorageBackend};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::segment::{SegmentBuilder, SegmentGroupBuilder};

use super::{FinalizedBlockIngestor, IngestionEvent, RpcProvider};

pub struct Ingestor<S: StorageBackend + Send + Sync + 'static> {
    segment_options: SegmentOptions,
    provider: RpcProvider,
    storage: S,
}

impl<S> Ingestor<S>
where
    S: StorageBackend + Send + Sync + 'static,
{
    pub fn new(provider: RpcProvider, storage: S) -> Self {
        let segment_options = SegmentOptions::default();
        Self {
            segment_options,
            provider,
            storage,
        }
    }

    pub fn with_segment_options(mut self, segment_options: SegmentOptions) -> Self {
        self.segment_options = segment_options;
        self
    }

    pub async fn start(mut self, starting_block_number: u64, ct: CancellationToken) -> Result<()> {
        let mut ingestor = FinalizedBlockIngestor::new(self.provider, starting_block_number);

        let mut segment_builder = SegmentBuilder::new();
        let mut segment_group_builder = SegmentGroupBuilder::new();

        let mut segment_size = 0;
        let mut group_size = 0;

        loop {
            if ct.is_cancelled() {
                return Ok(());
            }

            let max_blocks = 1;
            match ingestor
                .ingest_next_segment(&mut segment_builder, max_blocks)
                .await?
            {
                IngestionEvent::Completed {
                    last_ingested_block,
                } => {
                    info!(block_number = last_ingested_block, "finished ingestion");
                    break;
                }
                IngestionEvent::Segment {
                    count,
                    first_block_number,
                    last_block_number,
                } => {
                    segment_size += count;
                    info!(
                        first_block_number = first_block_number,
                        last_block_number = last_block_number,
                        count = count,
                        segment_size = segment_size,
                        "ingested segments"
                    );

                    segment_group_builder.add_segment(first_block_number, count);

                    if segment_size >= self.segment_options.segment_size {
                        let segment_name =
                            self.segment_options.format_segment_name(last_block_number);
                        segment_builder
                            .write(&format!("segment/{segment_name}"), &mut self.storage)
                            .await?;
                        let index = segment_builder.take_index();
                        segment_group_builder.add_index(&index);

                        segment_size = 0;
                        segment_builder.reset();
                        group_size += 1;

                        info!(segment_name, "wrote segment");
                    }

                    if group_size >= self.segment_options.group_size {
                        let group_name = self
                            .segment_options
                            .format_segment_group_name(last_block_number);
                        group_size = 0;
                        segment_group_builder
                            .write(&group_name, &mut self.storage)
                            .await?;
                        segment_group_builder.reset();
                        info!(group_name, "wrote group index");
                    }
                }
            }
        }

        Ok(())
    }
}
