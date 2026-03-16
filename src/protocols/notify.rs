use std::sync::Arc;

use aex::{connection::context::Context, tcp::types::Command};
use tokio::sync::Mutex;

use crate::protocols::{
    command::{Action, Entity},
    frame::P2PFrame,
};

pub async fn notify<T: Command + Clone>(
    ctx: Arc<Mutex<Context>>,
    cmd: T,
    entity: Entity,
    action: Action,
    is_encrypt: bool,
) {
    let manager = {
        let guard = ctx.lock().await;
        guard.global.manager.clone()
    };
    manager
        .forward(|entries| async {
            for entry in entries {
                // 1. 先把临时值固定到一个变量名上，延长它的生命周期
                if let Some(ctx) = &entry.context {
                    P2PFrame::send::<T>(
                        ctx.clone(),
                        &Some(cmd.clone()),
                        entity,
                        action,
                        is_encrypt,
                    )
                    .await
                    .expect("error notify online server!");
                    println!("notify send!");
                }
            }
        })
        .await;
}
