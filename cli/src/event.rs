//! Iroha is a quite dynamic system so many events can happen.
//! This module contains descriptions of such an events and
//! utility Iroha Special Instructions to work with them.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::std_instead_of_core,
    clippy::std_instead_of_alloc
)]
use futures::TryStreamExt;
use iroha_data_model::events::prelude::*;
use iroha_macro::error::ErrorTryFromEnum;
use warp::ws::WebSocket;

use crate::stream::{self, Sink, Stream};

/// Type of Stream error
pub type StreamError = stream::Error<<WebSocket as Stream<EventSubscriptionRequest>>::Err>;

/// Type of error for `Consumer`
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Error from provided stream/websocket
    #[error("Stream error: {0}")]
    Stream(Box<StreamError>),
    /// Error from converting received message to filter
    #[error("Can't retrieve subscription filter: {0}")]
    CantRetrieveSubscriptionFilter(#[from] ErrorTryFromEnum<EventSubscriptionRequest, FilterBox>),
    /// Error from provided websocket
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] warp::Error),
    /// Error that occurs than `WebSocket::next()` call returns `None`
    #[error("Can't receive message from stream")]
    CantReceiveMessage,
}

impl From<StreamError> for Error {
    fn from(error: StreamError) -> Self {
        Self::Stream(Box::new(error))
    }
}

/// Result type for `Consumer`
pub type Result<T> = core::result::Result<T, Error>;

/// Consumer for Iroha `Event`(s).
/// Passes the events over the corresponding connection `stream` if they match the `filter`.
#[derive(Debug)]
pub struct Consumer {
    stream: WebSocket,
    filter: FilterBox,
}

impl Consumer {
    /// Constructs [`Consumer`], which consumes `Event`s and forwards it through the `stream`.
    ///
    /// # Errors
    /// Can fail due to timeout or without message at websocket or during decoding request
    #[iroha_futures::telemetry_future]
    pub async fn new(mut stream: WebSocket) -> Result<Self> {
        let EventSubscriptionRequest(filter) = stream.recv().await?;
        Ok(Consumer { stream, filter })
    }

    /// Forwards the `event` over the `stream` if it matches the `filter`.
    ///
    /// # Errors
    /// Can fail due to timeout or sending event. Also receiving might fail
    #[iroha_futures::telemetry_future]
    pub async fn consume(&mut self, event: Event) -> Result<()> {
        if !self.filter.matches(&event) {
            return Ok(());
        }

        self.stream
            .send(EventMessage(event))
            .await
            .map_err(Into::into)
    }

    /// Listen for `Close` message in loop
    ///
    /// # Errors
    /// Can fail if can't receive message from stream for some reason
    pub async fn stream_closed(&mut self) -> Result<()> {
        while let Some(message) = self.stream.try_next().await? {
            if message.is_close() {
                return Ok(());
            }
            iroha_logger::warn!("Unexpected message received: {:?}", message);
        }
        Err(Error::CantReceiveMessage)
    }

    /// Close stream. See [`WebSocket::close()`]
    ///
    /// # Errors
    /// Throws up [`WebSocket::close()`] errors
    pub async fn close_stream(self) -> Result<()> {
        self.stream.close().await.map_err(Into::into)
    }
}
