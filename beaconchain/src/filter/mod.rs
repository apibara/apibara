mod blob;
mod helpers;
mod transaction;
mod validator;

use apibara_dna_common::{
    data_stream::BlockFilterFactory,
    query::{BlockFilter, HeaderFilter},
};
use apibara_dna_protocol::beaconchain;
use prost::Message;

use self::helpers::{BlockFilterExt, FragmentFilterExt};

pub struct BeaconChainFilterFactory;

impl BlockFilterFactory for BeaconChainFilterFactory {
    fn create_block_filter(
        &self,
        filters: &[Vec<u8>],
    ) -> tonic::Result<Vec<BlockFilter>, tonic::Status> {
        let proto_filters = filters
            .iter()
            .map(|bytes| beaconchain::Filter::decode(bytes.as_slice()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| tonic::Status::invalid_argument("failed to decode filter"))?;

        if proto_filters.is_empty() {
            return Err(tonic::Status::invalid_argument("no filters provided"));
        }

        if proto_filters.len() > 5 {
            return Err(tonic::Status::invalid_argument(format!(
                "too many filters ({} > 5)",
                proto_filters.len(),
            )));
        }

        let filters = proto_filters
            .iter()
            .map(BlockFilterExt::compile_to_block_filter)
            .collect::<tonic::Result<Vec<_>>>()?;

        if filters.iter().any(|f| f.can_produce_data()) {
            Ok(filters)
        } else {
            Err(tonic::Status::invalid_argument(
                "at least one filter must be non-empty",
            ))
        }
    }
}

impl BlockFilterExt for beaconchain::Filter {
    fn compile_to_block_filter(&self) -> tonic::Result<BlockFilter, tonic::Status> {
        let mut block_filter = BlockFilter::default();

        let header_filter = match beaconchain::HeaderFilter::try_from(self.header) {
            Ok(beaconchain::HeaderFilter::Always) => Some(HeaderFilter::Always),
            Ok(beaconchain::HeaderFilter::OnData) => Some(HeaderFilter::OnData),
            Ok(beaconchain::HeaderFilter::OnDataOrOnNewBlock) => {
                Some(HeaderFilter::OnDataOrOnNewBlock)
            }
            _ => None,
        }
        .unwrap_or_default();

        block_filter.set_header_filter(header_filter);

        for filter in self.transactions.iter() {
            let filter = filter.compile_to_filter()?;
            block_filter.add_filter(filter);
        }

        for filter in self.blobs.iter() {
            let filter = filter.compile_to_filter()?;
            block_filter.add_filter(filter);
        }

        for filter in self.validators.iter() {
            let filter = filter.compile_to_filter()?;
            block_filter.add_filter(filter);
        }

        Ok(block_filter)
    }
}
