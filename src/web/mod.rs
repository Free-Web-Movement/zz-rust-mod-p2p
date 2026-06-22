pub mod aex_re_exports;
pub mod api;
pub mod templates;
pub mod types;

use std::sync::Arc;

use futures::FutureExt;
use sea_orm::DatabaseConnection;

use crate::node::WebHandler;
use crate::user_store::UserStore;

use self::aex_re_exports::{Context, GlobalContext, HttpMetadata, HttpMethod};
use self::types::{MinterData, TransferFn};

/// Build the web handler closure.
///
/// The caller (root) must store a `MinterData` snapshot and a `TransferFn` callback
/// in `gctx` via `gctx.set(...).await` before starting the server.
pub fn build_handler(
    node_name: String,
    storage_dir: String,
    node_address: String,
    p2p_port: u16,
    db: DatabaseConnection,
    gctx: Arc<GlobalContext>,
    user_store: Arc<UserStore>,
) -> WebHandler {
    let node_name = Arc::new(node_name);
    let storage_dir = Arc::new(storage_dir);
    let node_address = Arc::new(node_address);
    let db = Arc::new(db);

    Arc::new(move |ctx: &mut Context| {
        let name = node_name.clone();
        let dir = storage_dir.clone();
        let addr = node_address.clone();
        let port = p2p_port;
        let db = db.clone();
        let gctx = gctx.clone();
        let user_store = user_store.clone();
        async move {
            let is_post = ctx
                .local
                .get_ref::<HttpMetadata>()
                .map(|m| m.method == HttpMethod::POST)
                .unwrap_or(false);
            let meta_path = ctx
                .local
                .get_ref::<HttpMetadata>()
                .map(|m| m.path.clone())
                .unwrap_or_default();

            if is_post && meta_path == "/api/transfer" {
                let tf = match gctx.get::<TransferFn>().await {
                    Some(f) => f,
                    None => {
                        ctx.send(r#"{"success":false,"error":"TransferFn not configured"}"#, None);
                        return true;
                    }
                };
                return api::handle_transfer(ctx, &*db, &addr, tf).await;
            }
            if !is_post && meta_path.starts_with("/api/address") {
                return api::handle_address_api(ctx, &*db, &meta_path).await;
            }
            if !is_post && meta_path == "/api/contacts" {
                return api::handle_list_contacts(ctx, &*db, &addr, gctx.clone(), &user_store).await;
            }
            if is_post && meta_path == "/api/contacts" {
                return api::handle_add_contact(ctx, &*db).await;
            }
            if meta_path.starts_with("/api/contacts") && meta_path.contains("?address=") {
                return api::handle_delete_contact(ctx, &*db, &meta_path).await;
            }
            if meta_path.starts_with("/api/chat_messages") && meta_path.contains("?contact=") {
                return api::handle_get_chat_messages(ctx, &user_store, &meta_path).await;
            }
            if !is_post && meta_path == "/api/conversations" {
                return api::handle_get_conversations(ctx, &user_store).await;
            }
            if !is_post
                && meta_path.starts_with("/api/profile")
                && meta_path.contains("?address=")
            {
                return api::handle_get_profile(ctx, &user_store, &meta_path).await;
            }
            if !is_post && meta_path == "/api/profile" {
                return api::handle_get_self_profile(ctx, &user_store, &addr).await;
            }
            if is_post && meta_path == "/api/profile" {
                return api::handle_save_profile(ctx, &*db, &user_store, &addr, &meta_path).await;
            }
            if is_post && meta_path == "/api/profile/avatar" {
                return api::handle_upload_avatar(ctx, &user_store, &addr, &meta_path).await;
            }
            if is_post && meta_path == "/api/send_chat" {
                return api::handle_send_chat(ctx, gctx.clone(), &addr, user_store.clone()).await;
            }
            if !is_post && meta_path == "/api/data" {
                let md = match gctx.get::<MinterData>().await {
                    Some(d) => d,
                    None => {
                        ctx.send(
                            r#"{"success":false,"error":"MinterData not configured"}"#,
                            None,
                        );
                        return true;
                    }
                };
                return api::handle_data_api(ctx, &*db, gctx.clone(), gctx.clone(), &md, &addr, port)
                    .await;
            }
            if meta_path == "/wallet" {
                let md = match gctx.get::<MinterData>().await {
                    Some(d) => d,
                    None => {
                        ctx.send(r#"{"success":false,"error":"MinterData not configured"}"#, None);
                        return true;
                    }
                };
                return api::handle_wallet_page(ctx, port, &name, &addr, gctx.clone(), &md, &*db, gctx.clone()).await;
            }
            if meta_path == "/chat" {
                let md = match gctx.get::<MinterData>().await {
                    Some(d) => d,
                    None => {
                        ctx.send(r#"{"success":false,"error":"MinterData not configured"}"#, None);
                        return true;
                    }
                };
                return api::handle_chat_page(ctx, port, &name, &addr, gctx.clone(), &md, &*db, gctx.clone()).await;
            }
            if meta_path == "/network" {
                let md = match gctx.get::<MinterData>().await {
                    Some(d) => d,
                    None => {
                        ctx.send(r#"{"success":false,"error":"MinterData not configured"}"#, None);
                        return true;
                    }
                };
                return api::handle_network_page(
                    ctx, port, &name, &dir, &*db, gctx.clone(), gctx.clone(), &md, &addr,
                )
                .await;
            }
            let md = match gctx.get::<MinterData>().await {
                Some(d) => d,
                None => {
                    ctx.send(
                        r#"{"success":false,"error":"MinterData not configured"}"#,
                        None,
                    );
                    return true;
                }
            };
            api::handle_index_page(ctx, port, &name, &dir, &addr, gctx.clone(), &md, &*db, gctx.clone()).await
        }
        .boxed()
    })
}
