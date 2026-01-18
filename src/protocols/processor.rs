use std::sync::Arc;

use futures::{ FutureExt, future::BoxFuture };

use crate::{
    context::Context,
    protocols::{ codec::Codec, command::{ Action, Entity }, frame::Frame },
};

pub struct CommandProcessor<C: Send + Sync + 'static> {
    pub entity: Entity,
    pub action: Action,

    pub handler: Arc<dyn (Fn(Frame, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>) + Send + Sync>,

    pub sender: Arc<
        dyn (Fn(Vec<u8>, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>) + Send + Sync
    >,
}

impl<C: Send + Sync + 'static> CommandProcessor<C> {
    pub fn new<T>(
        entity: Entity,
        action: Action,
        handler: fn(T, Frame, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>,
        sender: fn(T, Arc<Context>, Arc<C>) -> BoxFuture<'static, ()>
    ) -> Self
        where T: Codec + bincode::Decode<()> + 'static
    {
        let handler = Arc::new(
            move |frame: Frame, ctx: Arc<Context>, client: Arc<C>| -> BoxFuture<'static, ()> {
                (
                    async move {
                        let cmd = match T::from_bytes(&frame.body.data) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::error!("decode failed: {:?}", e);
                                return;
                            }
                        };

                        handler(cmd, frame, ctx, client).await;
                    }
                ).boxed()
            }
        );

        let sender = Arc::new(
            move |data: Vec<u8>, ctx: Arc<Context>, client: Arc<C>| -> BoxFuture<'static, ()> {
                (
                    async move {
                        let cmd = match T::from_bytes(&data) {
                            Ok(c) => c,
                            Err(e) => {
                                tracing::error!("decode failed: {:?}", e);
                                return;
                            }
                        };

                        sender(cmd, ctx, client).await;
                    }
                ).boxed()
            }
        );

        Self {
            entity,
            action,
            handler,
            sender,
        }
    }
}
