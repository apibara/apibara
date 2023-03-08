//! State update data.

use apibara_core::starknet::v1alpha2;
use apibara_node::db::Table;

use crate::core::GlobalBlockId;

/// Store state updates.
#[derive(Debug, Clone, Copy, Default)]
pub struct StateUpdateTable {}

impl Table for StateUpdateTable {
    type Key = GlobalBlockId;
    type Value = v1alpha2::StateUpdate;

    fn db_name() -> &'static str {
        "StateUpdate"
    }
}
