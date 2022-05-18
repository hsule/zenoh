//
// Copyright (c) 2022 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

//! Query primitives.

use crate::prelude::*;
use crate::Session;
use crate::API_REPLY_RECEPTION_CHANNEL_SIZE;
use std::collections::HashMap;
use std::fmt;
use zenoh_core::zresult::ZResult;
use zenoh_core::{AsyncResolve, Resolvable, SyncResolve};

/// The [`Queryable`](crate::queryable::Queryable)s that should be target of a [`get`](Session::get).
pub use zenoh_protocol_core::QueryTarget;

/// The kind of consolidation.
pub use zenoh_protocol_core::ConsolidationMode;

/// The kind of consolidation that should be applied on replies to a [`get`](Session::get)
/// at different stages of the reply process.
pub use zenoh_protocol_core::ConsolidationStrategy;

/// The replies consolidation strategy to apply on replies to a [`get`](Session::get).
#[derive(Clone, Debug)]
pub enum QueryConsolidation {
    Auto,
    Manual(ConsolidationStrategy),
}

impl QueryConsolidation {
    /// Automatic query consolidation strategy selection.
    ///
    /// A query consolidation strategy will automatically be selected depending
    /// the query selector. If the selector contains time range properties,
    /// no consolidation is performed. Otherwise the [`reception`](QueryConsolidation::reception) strategy is used.
    #[inline]
    pub fn auto() -> Self {
        QueryConsolidation::Auto
    }

    /// No consolidation performed.
    ///
    /// This is useful when querying timeseries data bases or
    /// when using quorums.
    #[inline]
    pub fn none() -> Self {
        QueryConsolidation::Manual(ConsolidationStrategy::none())
    }

    /// Lazy consolidation performed at all stages.
    ///
    /// This strategy offers the best latency. Replies are directly
    /// transmitted to the application when received without needing
    /// to wait for all replies.
    ///
    /// This mode does not guarantee that there will be no duplicates.
    #[inline]
    pub fn lazy() -> Self {
        QueryConsolidation::Manual(ConsolidationStrategy::lazy())
    }

    /// Full consolidation performed at reception.
    ///
    /// This is the default strategy. It offers the best latency while
    /// guarantying that there will be no duplicates.
    #[inline]
    pub fn reception() -> Self {
        QueryConsolidation::Manual(ConsolidationStrategy::reception())
    }

    /// Full consolidation performed on last router and at reception.
    ///
    /// This mode offers a good latency while optimizing bandwidth on
    /// the last transport link between the router and the application.
    #[inline]
    pub fn last_router() -> Self {
        QueryConsolidation::Manual(ConsolidationStrategy::last_router())
    }

    /// Full consolidation performed everywhere.
    ///
    /// This mode optimizes bandwidth on all links in the system
    /// but will provide a very poor latency.
    #[inline]
    pub fn full() -> Self {
        QueryConsolidation::Manual(ConsolidationStrategy::full())
    }
}

impl Default for QueryConsolidation {
    fn default() -> Self {
        QueryConsolidation::Auto
    }
}

/// Structs returned by a [`get`](Session::get).
#[derive(Clone, Debug)]
pub struct Reply {
    /// The result of this Reply.
    pub sample: Result<Sample, Value>,
    /// The id of the zenoh instance that answered this Reply.
    pub replier_id: ZenohId,
}

pub(crate) struct QueryState {
    pub(crate) nb_final: usize,
    pub(crate) reception_mode: ConsolidationMode,
    pub(crate) replies: Option<HashMap<String, Reply>>,
    pub(crate) callback: Callback<Reply>,
}

/// A builder for initializing a `query`.
///
/// The result of the query is provided as a [`ReplyReceiver`](ReplyReceiver) and can be
/// accessed synchronously via [`wait()`](ZFuture::wait()) or asynchronously via `.await`.
///
/// # Examples
/// ```
/// # async_std::task::block_on(async {
/// use futures::prelude::*;
/// use zenoh::prelude::*;
/// use zenoh::query::*;
/// use zenoh::queryable;
///
/// let session = zenoh::open(config::peer()).await.unwrap();
/// let mut replies = session
///     .get("/key/expression?value>1")
///     .target(QueryTarget::All)
///     .consolidation(QueryConsolidation::none())
///     .await
///     .unwrap();
/// # })
/// ```
#[derive(Debug, Clone)]
pub struct GetBuilder<'a, 'b> {
    pub(crate) session: &'a Session,
    pub(crate) selector: Selector<'b>,
    pub(crate) target: QueryTarget,
    pub(crate) consolidation: QueryConsolidation,
    pub(crate) local_routing: Option<bool>,
}

impl<'a, 'b> GetBuilder<'a, 'b> {
    /// Make the built query a [`CallbackGet`](CallbackGet).
    #[inline]
    pub fn callback<Callback>(self, callback: Callback) -> CallbackGetBuilder<'a, 'b, Callback>
    where
        Callback: Fn(Reply) + Send + Sync + 'static,
    {
        CallbackGetBuilder {
            builder: self,
            callback,
        }
    }

    /// Make the built query a [`CallbackGet`](CallbackGet).
    #[inline]
    pub fn callback_mut<CallbackMut>(
        self,
        callback: CallbackMut,
    ) -> CallbackGetBuilder<'a, 'b, impl Fn(Reply) + Send + Sync + 'static>
    where
        CallbackMut: FnMut(Reply) + Send + Sync + 'static,
    {
        self.callback(locked(callback))
    }

    /// Make the built query a [`HandlerGet`](HandlerGet).
    #[inline]
    pub fn with<IntoHandler, Receiver>(
        self,
        handler: IntoHandler,
    ) -> HandlerGetBuilder<'a, 'b, Receiver>
    where
        IntoHandler: crate::prelude::IntoHandler<Reply, Receiver>,
    {
        HandlerGetBuilder {
            builder: self,
            handler: handler.into_handler(),
        }
    }

    /// Change the target of the query.
    #[inline]
    pub fn target(mut self, target: QueryTarget) -> Self {
        self.target = target;
        self
    }

    /// Change the consolidation mode of the query.
    #[inline]
    pub fn consolidation(mut self, consolidation: QueryConsolidation) -> Self {
        self.consolidation = consolidation;
        self
    }

    /// Enable or disable local routing.
    #[inline]
    pub fn local_routing(mut self, local_routing: bool) -> Self {
        self.local_routing = Some(local_routing);
        self
    }
}

impl Resolvable for GetBuilder<'_, '_> {
    type Output = zenoh_core::Result<flume::Receiver<Reply>>;
}
impl SyncResolve for GetBuilder<'_, '_> {
    fn res_sync(self) -> Self::Output {
        self.with(flume::bounded(*API_REPLY_RECEPTION_CHANNEL_SIZE))
            .res_sync()
    }
}
impl AsyncResolve for GetBuilder<'_, '_> {
    type Future = futures::future::Ready<Self::Output>;

    fn res_async(self) -> Self::Future {
        self.with(flume::bounded(*API_REPLY_RECEPTION_CHANNEL_SIZE))
            .res_async()
    }
}

#[derive(Debug, Clone)]
#[must_use = "ZFutures do nothing unless you `.wait()`, `.await` or poll them"]
pub struct CallbackGetBuilder<'a, 'b, Callback>
where
    Callback: Fn(Reply) + Send + Sync + 'static,
{
    builder: GetBuilder<'a, 'b>,
    pub(crate) callback: Callback,
}

impl<'a, 'b, Callback> CallbackGetBuilder<'a, 'b, Callback>
where
    Callback: Fn(Reply) + Send + Sync + 'static,
{
    /// Change the target of the query.
    #[inline]
    pub fn target(mut self, target: QueryTarget) -> Self {
        self.builder = self.builder.target(target);
        self
    }

    /// Change the consolidation mode of the query.
    #[inline]
    pub fn consolidation(mut self, consolidation: QueryConsolidation) -> Self {
        self.builder = self.builder.consolidation(consolidation);
        self
    }

    /// Enable or disable local routing.
    #[inline]
    pub fn local_routing(mut self, local_routing: bool) -> Self {
        self.builder = self.builder.local_routing(local_routing);
        self
    }
}

impl<Callback> Resolvable for CallbackGetBuilder<'_, '_, Callback>
where
    Callback: Fn(Reply) + Send + Sync + 'static,
{
    type Output = zenoh_core::Result<()>;
}
impl<Callback> SyncResolve for CallbackGetBuilder<'_, '_, Callback>
where
    Callback: Fn(Reply) + Send + Sync + 'static,
{
    fn res_sync(self) -> Self::Output {
        self.builder.session.query(
            &self.builder.selector,
            self.builder.target,
            self.builder.consolidation,
            self.builder.local_routing,
            Box::new(self.callback),
        )
    }
}
impl<Callback> AsyncResolve for CallbackGetBuilder<'_, '_, Callback>
where
    Callback: Fn(Reply) + Send + Sync + 'static,
{
    type Future = futures::future::Ready<Self::Output>;

    fn res_async(self) -> Self::Future {
        futures::future::ready(self.res_sync())
    }
}

pub struct HandlerGetBuilder<'a, 'b, Receiver> {
    builder: GetBuilder<'a, 'b>,
    handler: Handler<Reply, Receiver>,
}

impl<Receiver> fmt::Debug for HandlerGetBuilder<'_, '_, Receiver> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerGetBuilder")
            .field("selector", &self.builder.selector)
            .field("target", &self.builder.target)
            .field("consolidation", &self.builder.consolidation)
            .field("local_routing", &self.builder.local_routing)
            .finish()
    }
}

impl<'a, 'b, Receiver> HandlerGetBuilder<'a, 'b, Receiver>
where
    Receiver: Send + Sync,
{
    /// Change the target of the query.
    #[inline]
    pub fn target(mut self, target: QueryTarget) -> Self {
        self.builder = self.builder.target(target);
        self
    }

    /// Change the consolidation mode of the query.
    #[inline]
    pub fn consolidation(mut self, consolidation: QueryConsolidation) -> Self {
        self.builder = self.builder.consolidation(consolidation);
        self
    }

    /// Enable or disable local routing.
    #[inline]
    pub fn local_routing(mut self, local_routing: bool) -> Self {
        self.builder = self.builder.local_routing(local_routing);
        self
    }
}

impl<Receiver: Send> Resolvable for HandlerGetBuilder<'_, '_, Receiver> {
    type Output = ZResult<Receiver>;
}
impl<Receiver: Send> SyncResolve for HandlerGetBuilder<'_, '_, Receiver> {
    fn res_sync(self) -> Self::Output {
        let (callback, receiver) = self.handler;
        self.builder
            .session
            .query(
                &self.builder.selector,
                self.builder.target,
                self.builder.consolidation,
                self.builder.local_routing,
                callback,
            )
            .map(|_| receiver)
    }
}
impl<Receiver: Send> AsyncResolve for HandlerGetBuilder<'_, '_, Receiver> {
    type Future = futures::future::Ready<Self::Output>;

    fn res_async(self) -> Self::Future {
        futures::future::ready(self.res_sync())
    }
}

impl IntoHandler<Reply, flume::Receiver<Reply>> for (flume::Sender<Reply>, flume::Receiver<Reply>) {
    fn into_handler(self) -> Handler<Reply, flume::Receiver<Reply>> {
        let (sender, receiver) = self;
        (
            Box::new(move |s| {
                if let Err(e) = sender.send(s) {
                    log::warn!("Error sending reply into flume channel: {}", e)
                }
            }),
            receiver,
        )
    }
}
