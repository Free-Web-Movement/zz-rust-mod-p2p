use std::sync::Arc;

use aex::connection::context::Context;
use tokio::sync::Mutex;
use zz_account::address::FreeWebMovementAddress;

use crate::protocols::{
    command::{Action, Entity},
    commands::online::OnlineCommand,
    frame::P2PFrame,
};

use crate::protocols::commands::offline::OfflineCommand;

pub async fn notify_online(ctx: Arc<Mutex<Context>>, cmd: OnlineCommand) {
    {
        let manager = {
            let guard = ctx.lock().await;
            guard.global.manager.clone()
        };

        let address: FreeWebMovementAddress = {
            let guard = ctx.lock().await;
            guard.get().await.unwrap()
        };
        manager
            .forward(|entries| async {
                for entry in entries {
                    // 1. 先把临时值固定到一个变量名上，延长它的生命周期
                    if let Some(ctx) = &entry.context {
                        let mut guard = ctx.lock().await;
                        if let Some(writer) = &mut guard.writer {
                            P2PFrame::send::<OnlineCommand>(
                                &address,
                                writer,
                                &Some(cmd.clone()),
                                Entity::Node,
                                Action::OnLine,
                                None,
                            )
                            .await
                            .expect("error notify online server!");
                            println!("notify send!");
                        }
                    }
                }
            })
            .await;
    }
}

pub async fn notify_offline(ctx: Arc<Mutex<Context>>, cmd: OfflineCommand) {
    {
        let manager = {
            let guard = ctx.lock().await;
            guard.global.manager.clone()
        };

        let address: FreeWebMovementAddress = {
            let guard = ctx.lock().await;
            guard.get().await.unwrap()
        };
        manager
            .forward(|entries| async {
                for entry in entries {
                    // 1. 先把临时值固定到一个变量名上，延长它的生命周期
                    if let Some(ctx) = &entry.context {
                        let mut guard = ctx.lock().await;
                        if let Some(writer) = &mut guard.writer {
                            P2PFrame::send::<OfflineCommand>(
                                &address,
                                writer,
                                &Some(cmd.clone()),
                                Entity::Node,
                                Action::OffLine,
                                None,
                            )
                            .await
                            .expect("error notify offline server!");
                            println!("notify send!");
                        }
                    }
                }
            })
            .await;
    }
}
