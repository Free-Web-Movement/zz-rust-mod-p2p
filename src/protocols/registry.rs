use aex::tcp::router::Router;
use futures::FutureExt;

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::{ack::onlineack_handler, online::online_handler},
    frame::P2PFrame,
};

pub fn register(router: &mut Router) {
    router.on::<P2PFrame, P2PCommand>(
        P2PCommand::to_u32(Entity::Node, Action::OnLine),
        Box::new(|ctx, frame, cmd| {
            Box::pin(async move {
                online_handler(ctx.clone(), frame, cmd).await;
                Ok(true)
            })
            .boxed()
        }),
        vec![],
    );

    router.on::<P2PFrame, P2PCommand>(
        P2PCommand::to_u32(Entity::Node, Action::OffLine),
        Box::new(|ctx, frame, cmd| {
            Box::pin(async move {
                online_handler(ctx.clone(), frame, cmd).await;
                Ok(true)
            })
            .boxed()
        }),
        vec![],
    );

    router.on::<P2PFrame, P2PCommand>(
        P2PCommand::to_u32(Entity::Node, Action::OnLineAck),
        Box::new(|ctx, frame, cmd| {
            Box::pin(async move {
                onlineack_handler(ctx.clone(), frame, cmd).await;
                Ok(true)
            })
            .boxed()
        }),
        vec![],
    );

        router.on::<P2PFrame, P2PCommand>(
        P2PCommand::to_u32(Entity::Message, Action::SendText),
        Box::new(|ctx, frame, cmd| {
            Box::pin(async move {
                onlineack_handler(ctx.clone(), frame, cmd).await;
                Ok(true)
            })
            .boxed()
        }),
        vec![],
    );
}
