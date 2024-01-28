use std::{path::PathBuf, process::ExitCode, time::Instant};

use apibara_dna_common::{
    error::{DnaError, ReportExt, Result},
    segment::SegmentArgs,
    storage::LocalStorageBackend,
};
use apibara_dna_evm::{
    ingestion::{Ingestor, RpcProviderService},
    segment::{
        store, BlockHeaderSegmentReader, LogSegmentReader, SegmentGroupExt, SegmentGroupReader,
    },
};
use apibara_observability::init_opentelemetry;
use clap::{Args, Parser, Subcommand};
use error_stack::ResultExt;
use roaring::RoaringBitmap;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

#[cfg(not(windows))]
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    subcommand: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    StartIngestion(StartIngestionArgs),
    Inspect(InspectArgs),
}

/// Start ingesting data from Ethereum.
///
/// If a snapshot is already present, it will be used to resume ingestion.
#[derive(Args, Debug)]
struct StartIngestionArgs {
    /// Start ingesting data from this block.
    ///
    /// Notice that if a client requests data from a block that is earlier than
    /// this block, it will error.
    #[arg(long, env, default_value = "0")]
    pub from_block: u64,
    /// Location for ingested data.
    #[arg(long, env)]
    pub data_dir: PathBuf,
    #[clap(flatten)]
    pub segment: SegmentArgs,
    #[clap(flatten)]
    pub rpc: RpcArgs,
}

#[derive(Args, Debug, Clone)]
struct RpcArgs {
    /// Ethereum RPC URL.
    #[arg(long, env)]
    pub rpc_url: String,
    /// RPC rate limit, in requests per second.
    #[arg(long, env, default_value = "1000")]
    pub rpc_rate_limit: usize,
    /// How many concurrent requests to send.
    #[arg(long, env, default_value = "100")]
    pub rpc_concurrency: usize,
}

#[derive(Args, Debug)]
struct InspectArgs {
    /// Location for ingested data.
    #[arg(long, env)]
    pub data_dir: PathBuf,
    /// Start ingesting data from this block.
    ///
    /// Needed until we add snapshots.
    #[arg(long, env, default_value = "0")]
    pub from_block: u64,
    #[arg(long, env)]
    pub to_block: u64,
    /// Needed until we add snapshots.
    #[clap(flatten)]
    pub segment: SegmentArgs,
    #[clap(flatten)]
    pub logs: InspectLogsArgs,
    #[clap(flatten)]
    pub header: InspectHeadersArgs,
}

#[derive(Args, Debug)]
struct InspectHeadersArgs {
    #[arg(long, env, default_value = "false")]
    pub header: bool,
}

#[derive(Args, Debug)]
struct InspectLogsArgs {
    /// Address to inspect.
    #[arg(long, env)]
    pub address: Option<String>,
    /// Topic to inspect.
    #[arg(long, env)]
    pub topic: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Cli::parse();
    run_with_args(args).await.to_exit_code()
}

async fn run_with_args(args: Cli) -> Result<()> {
    init_opentelemetry()
        .change_context(DnaError::Fatal)
        .attach_printable("failed to initialize opentelemetry")?;

    match args.subcommand {
        Command::StartIngestion(args) => run_ingestion(args).await,
        Command::Inspect(args) => run_inspect(args).await,
    }
}

async fn run_ingestion(args: StartIngestionArgs) -> Result<()> {
    info!(from_block = %args.from_block, "Starting EVM ingestion");
    info!(data_dir = %args.data_dir.display(), "Using data directory");
    let segment_options = args.segment.to_segment_options();
    info!(segment_options = ?segment_options, "Using segment options");

    let ct = CancellationToken::new();

    let (provider, rpc_provider_fut) = RpcProviderService::new(args.rpc.rpc_url)?
        .with_rate_limit(args.rpc.rpc_rate_limit as u32)
        .with_concurrency(args.rpc.rpc_concurrency)
        .start(ct.clone());

    let storage = LocalStorageBackend::new(args.data_dir);
    let starting_block_number = segment_options.segment_group_start(args.from_block);
    let ingestor = Ingestor::new(provider, storage).with_segment_options(segment_options);

    let rpc_provider_task = tokio::spawn(rpc_provider_fut);
    let ingestion_task = tokio::spawn({
        let ct = ct.clone();
        async move { ingestor.start(starting_block_number, ct).await }
    });

    tokio::select! {
        _ = rpc_provider_task => {
            info!("rpc provider task finished");
        }
        _ = ingestion_task => {
            info!("ingestion task finished");
        }
    }

    Ok(())
}

async fn run_inspect(args: InspectArgs) -> Result<()> {
    info!(data_dir = %args.data_dir.display(), "Using data directory");

    let storage = LocalStorageBackend::new(args.data_dir);
    let segment_options = args.segment.to_segment_options();
    let starting_block_number = segment_options.segment_group_start(args.from_block);

    let mut segment_group_reader =
        SegmentGroupReader::new(storage.clone(), segment_options.clone(), 1024 * 1024 * 1024);
    let mut header_segment_reader =
        BlockHeaderSegmentReader::new(storage.clone(), segment_options.clone(), 1024 * 1024 * 1024);
    let mut log_segment_reader =
        LogSegmentReader::new(storage.clone(), segment_options.clone(), 1024 * 1024 * 1024);

    let address_filter = if let Some(address) = args.logs.address {
        info!(address, "Filter by log address");
        let address = store::Address::from_hex(&address)
            .change_context(DnaError::Fatal)
            .attach_printable("failed to parse address")?;
        Some(address)
    } else {
        None
    };

    /*
    let topic_filter = if let Some(topic) = args.logs.topic {
        todo!()
    } else {
        None
    };
    */

    let mut current_block_number = starting_block_number;

    let start_time = Instant::now();

    let mut block_bitmap = RoaringBitmap::new();
    let mut event_count = 0;
    let mut segment_read_count = 0;
    while current_block_number < args.to_block {
        let current_segment_group_start = segment_options.segment_group_start(current_block_number);
        debug!(current_segment_group_start, "reading new segment group");
        let segment_group = segment_group_reader
            .read(current_segment_group_start)
            .await?;

        assert_eq!(
            segment_group.first_block_number(),
            current_segment_group_start
        );

        let segment_group_blocks = segment_options.segment_group_blocks();
        let segment_group_end = current_segment_group_start + segment_group_blocks;

        block_bitmap.clear();
        if args.header.header {
            block_bitmap.insert_range(current_segment_group_start as u32..segment_group_end as u32);
        } else {
            if let Some(address) = &address_filter {
                let address_bitmap = segment_group
                    .get_log_by_address(address)
                    .unwrap_or_default();
                debug!(address = %address, address_bitmap = ?address_bitmap, "read address bitmap");
                block_bitmap |= address_bitmap;
            }
        }

        // Skip as many segments in the group as possible.
        if let Some(starting_block) = block_bitmap.min() {
            current_block_number = starting_block as u64;
        } else {
            debug!(segment_group_end, "no blocks to read. skip ahead");
            current_block_number = segment_group_end;
            continue;
        }

        let mut current_segment_start = segment_options.segment_start(current_block_number);
        debug!(current_segment_start, "reading starting segment");

        let mut header_segment = if args.header.header {
            Some(header_segment_reader.read(current_segment_start).await?)
        } else {
            None
        };

        let mut log_segment = if address_filter.is_some() {
            Some(log_segment_reader.read(current_segment_start).await?)
        } else {
            None
        };
        for block_number in block_bitmap.iter() {
            if current_segment_start < segment_options.segment_start(block_number as u64) {
                current_segment_start = segment_options.segment_start(block_number as u64);
                segment_read_count += 1;
                debug!(current_segment_start, "reading new segment");
                if header_segment.is_some() {
                    header_segment = Some(header_segment_reader.read(current_segment_start).await?)
                };

                if log_segment.is_some() {
                    log_segment = Some(log_segment_reader.read(current_segment_start).await?)
                };
            }

            debug!(block_number, "inspect block");

            if let Some(log_segment) = log_segment.as_ref() {
                let target_address = address_filter.as_ref().unwrap();

                let index = block_number - log_segment.first_block_number() as u32;
                let block_logs = log_segment.blocks().unwrap_or_default().get(index as usize);

                for log in block_logs.logs().unwrap_or_default() {
                    let address = log.address().expect("address is missing");
                    if address != target_address {
                        continue;
                    }

                    let _topics = log.topics().unwrap_or_default();
                    let _data = log.data().unwrap_or_default();

                    let log_index = log.log_index();
                    let transaction_index = log.transaction_index();
                    let transaction_hash = log.transaction_hash().unwrap();

                    info!(
                        block_number,
                        transaction_index,
                        log_index,
                        transaction_hash = %transaction_hash.as_hex(),
                        "    log"
                    );

                    event_count += 1;
                }
            }

            if let Some(header_segment) = header_segment.as_ref() {
                let index = block_number - header_segment.first_block_number() as u32;
                let header = header_segment
                    .headers()
                    .unwrap_or_default()
                    .get(index as usize);

                let miner = header.miner().expect("miner is missing");

                info!(
                    number = header.number(),
                    miner = %miner.as_hex(),
                    "block"
                );

                for withdrawal in header.withdrawals().unwrap_or_default() {
                    let amount = withdrawal.amount().unwrap().format_units("wei")?;
                    info!(
                        index = withdrawal.index(),
                        validator_index = withdrawal.validator_index(),
                        address = %withdrawal.address().unwrap().as_hex(),
                        amount = format!("{amount} gwei"),
                        "    withdrawal"
                    );
                }
            }
        }

        current_block_number = segment_group_end;
    }

    let elapsed = start_time.elapsed();

    let block_count = current_block_number - starting_block_number;
    let block_sec = block_count as f64 / elapsed.as_secs_f64();

    info!(
        elapsed = ?elapsed,
        block_count,
        block_sec = format!("{block_sec:.0}"),
        "block count"
    );

    let event_sec = event_count as f64 / elapsed.as_secs_f64();

    info!(
        elapsed = ?elapsed,
        event_count,
        event_sec = format!("{event_sec:.0}"),
        "event count"
    );

    let segment_read_sec = segment_read_count as f64 / elapsed.as_secs_f64();

    info!(
        elapsed = ?elapsed,
        segment_read_count,
        segment_read_sec = format!("{segment_read_sec:.0}"),
        "segment read count"
    );

    Ok(())
}
