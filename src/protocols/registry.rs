use aex::tcp::router::Router as TcpRouter;
use aex::tcp::types::Command;
use futures::future::BoxFuture;
use std::sync::Arc;
use tokio::sync::Mutex;

use aex::connection::context::Context;

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::{
        ack::onlineack_handler, message::message_handler, offline::offline_handler,
        online::online_handler,
    },
    frame::P2PFrame,
};

type P2PDoer = Box<
    dyn Fn(Arc<Mutex<Context>>, P2PFrame, P2PCommand) -> BoxFuture<'static, anyhow::Result<bool>>
        + Send
        + Sync
        + 'static,
>;

fn extract_p2p_cmd_id(cmd: &P2PCommand) -> u32 {
    P2PCommand::to_u32(cmd.entity, cmd.action)
}

pub fn register(mut router: TcpRouter<P2PFrame, P2PCommand>) -> TcpRouter<P2PFrame, P2PCommand> {
    router = router.extractor(extract_p2p_cmd_id);

    router.on(
        P2PCommand::to_u32(Entity::Node, Action::OnLine),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                online_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    router.on(
        P2PCommand::to_u32(Entity::Node, Action::OffLine),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                offline_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    router.on(
        P2PCommand::to_u32(Entity::Node, Action::OnLineAck),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                onlineack_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    router.on(
        P2PCommand::to_u32(Entity::Message, Action::SendText),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                message_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    tracing::info!("Registered handlers: {:?}", router.handlers);
    router
}
