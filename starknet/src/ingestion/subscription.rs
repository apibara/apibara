use std::sync::Arc;

use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tracing::debug;

use crate::core::{GlobalBlockId, IngestionMessage};

use super::error::BlockIngestionError;

pub type IngestionStream = BroadcastStream<IngestionMessage>;

#[derive(Clone)]
pub struct IngestionStreamPublisher {
    tx: Arc<broadcast::Sender<IngestionMessage>>,
    _rx: Arc<broadcast::Receiver<IngestionMessage>>,
}

pub struct IngestionStreamClient {
    tx: Arc<broadcast::Sender<IngestionMessage>>,
}

impl IngestionStreamPublisher {
    pub fn new() -> (IngestionStreamClient, IngestionStreamPublisher) {
        let (tx, rx) = broadcast::channel(128);
        let tx = Arc::new(tx);
        let rx = Arc::new(rx);

        let manager = IngestionStreamPublisher {
            tx: tx.clone(),
            _rx: rx,
        };
        let client = IngestionStreamClient { tx };
        (client, manager)
    }

    pub fn publish_finalized(&self, id: GlobalBlockId) -> Result<(), BlockIngestionError> {
        self.publish(IngestionMessage::Finalized(id))
    }

    pub fn publish_accepted(&self, id: GlobalBlockId) -> Result<(), BlockIngestionError> {
        self.publish(IngestionMessage::Accepted(id))
    }

    pub fn publish_pending(&self, id: GlobalBlockId) -> Result<(), BlockIngestionError> {
        self.publish(IngestionMessage::Pending(id))
    }

    pub fn publish_invalidate(&self, id: GlobalBlockId) -> Result<(), BlockIngestionError> {
        self.publish(IngestionMessage::Invalidate(id))
    }

    fn publish(&self, message: IngestionMessage) -> Result<(), BlockIngestionError> {
        self.tx
            .send(message)
            .map_err(|_| BlockIngestionError::IngestionStreamPublish)?;
        Ok(())
    }
}

impl IngestionStreamClient {
    pub async fn subscribe(&self) -> IngestionStream {
        debug!("subscribing to ingestion stream");
        BroadcastStream::new(self.tx.subscribe())
    }
}
