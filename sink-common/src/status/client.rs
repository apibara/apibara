use apibara_core::node;
use color_eyre::eyre::{Context, Result};
use tokio::sync::mpsc;

/// Message between the connector and the status service.
#[derive(Debug)]
pub enum StatusMessage {
    /// Set the starting cursor.
    SetStartingCursor(Option<node::v1alpha2::Cursor>),
    /// Update the most recently indexed cursor.
    UpdateCursor(Option<node::v1alpha2::Cursor>),
    /// Send a heartbeat to the status service.
    Heartbeat,
}

#[derive(Clone)]
pub struct StatusServerClient {
    tx: mpsc::Sender<StatusMessage>,
}

impl StatusServerClient {
    pub fn new(tx: mpsc::Sender<StatusMessage>) -> Self {
        StatusServerClient { tx }
    }

    /// Send heartbeat message to status server.
    pub async fn heartbeat(&self) -> Result<()> {
        self.tx
            .send(StatusMessage::Heartbeat)
            .await
            .context("failed to send heartbeat message to status server")?;
        Ok(())
    }

    /// Update the most recently processed cursor.
    pub async fn set_starting_cursor(&self, cursor: Option<node::v1alpha2::Cursor>) -> Result<()> {
        self.tx
            .send(StatusMessage::SetStartingCursor(cursor))
            .await
            .context("failed to send starting cursor to status server")?;
        Ok(())
    }

    /// Update the most recently processed cursor.
    pub async fn update_cursor(&self, cursor: Option<node::v1alpha2::Cursor>) -> Result<()> {
        self.tx
            .send(StatusMessage::UpdateCursor(cursor))
            .await
            .context("failed to send update cursor message to status server")?;
        Ok(())
    }
}
