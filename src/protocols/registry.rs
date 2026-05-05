use aex::tcp::router::Router as TcpRouter;
use futures::future::BoxFuture;
use std::sync::Arc;
use tokio::sync::Mutex;

use aex::connection::context::Context;

use crate::protocols::{
    command::{Action, Entity, P2PCommand},
    commands::{
        ack::onlineack_handler, message::message_handler, node_sync::{node_sync_handler, node_sync_response_handler}, offline::offline_handler,
        online::online_handler, seed_sync::{seed_sync_commit_handler, seed_sync_request_handler, seed_sync_response_handler}, tick::tick_handler,
    },
    frame::P2PFrame,
};

#[allow(dead_code)]
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

    router.on(
        P2PCommand::to_u32(Entity::Witness, Action::Tick),
        Box::new(|ctx, frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                tick_handler(ctx, frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    // 注册节点同步处理器
    router.on(
        P2PCommand::to_u32(Entity::Node, Action::NodeSyncRequest),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                node_sync_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    router.on(
        P2PCommand::to_u32(Entity::Node, Action::NodeSyncResponse),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                node_sync_response_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    router.on(
        P2PCommand::to_u32(Entity::Node, Action::SeedSyncRequest),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                seed_sync_request_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    router.on(
        P2PCommand::to_u32(Entity::Node, Action::SeedSyncResponse),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                seed_sync_response_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    router.on(
        P2PCommand::to_u32(Entity::Node, Action::SeedSyncCommit),
        Box::new(|ctx, _frame, cmd: P2PCommand| {
            let c = cmd.clone();
            Box::pin(async move {
                seed_sync_commit_handler(ctx, _frame, c).await;
                Ok(true)
            })
        }),
        vec![],
    );

    tracing::info!("Registered handlers: {:?}", router.handlers);
    router
}
