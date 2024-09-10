use std::{net::SocketAddr, path::PathBuf};

use clap::Args;
use error_stack::{Result, ResultExt};

use crate::server::ServerOptions;

use super::{error::ServerError, StreamServiceOptions};

#[derive(Args, Debug)]
pub struct ServerArgs {
    /// Whether to run the DNA server.
    #[clap(long = "server.enabled", env = "DNA_SERVER_ENABLED")]
    pub server_enabled: bool,
    /// The DNA server address.
    #[clap(
        long = "server.address",
        env = "DNA_SERVER_ADDRESS",
        default_value = "0.0.0.0:7007"
    )]
    pub server_address: String,
    /// Where to store cached data.
    #[clap(
        long = "server.cache-dir",
        env = "DNA_SERVER_CACHE_DIR",
        default_value = "/data"
    )]
    pub server_cache_dir: String,
    #[clap(
        long = "server.max-concurrent-streams",
        env = "DNA_SERVER_MAX_CONCURRENT_STREAMS",
        default_value = "1000"
    )]
    pub max_concurrent_streams: usize,
}

impl ServerArgs {
    pub fn to_server_options(&self) -> Result<ServerOptions, ServerError> {
        let address = self
            .server_address
            .parse::<SocketAddr>()
            .change_context(ServerError)
            .attach_printable("failed to parse server address")
            .attach_printable_lazy(|| format!("address: {}", self.server_address))?;

        let cache_dir = self
            .server_cache_dir
            .parse::<PathBuf>()
            .change_context(ServerError)
            .attach_printable("failed to parse cache dir")
            .attach_printable_lazy(|| format!("cache dir: {}", self.server_cache_dir))?;

        let stream_service_options = StreamServiceOptions {
            max_concurrent_streams: self.max_concurrent_streams,
        };

        Ok(ServerOptions {
            address,
            cache_dir,
            stream_service_options,
        })
    }
}
