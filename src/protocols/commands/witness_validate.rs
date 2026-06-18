use std::sync::Arc;

use aex::connection::context::Context;
use aex::tcp::types::Codec;
use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::sync::mpsc;

use crate::protocols::command::P2PCommand;
use crate::protocols::command::{Action, Entity};
use crate::protocols::frame::P2PFrame;

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct WitnessValidateRequest {
    pub sender_id: String,
    pub nonce: [u8; 32],
}

impl Codec for WitnessValidateRequest {}

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct WitnessValidateResponse {
    pub sender_id: String,
    pub nonce: [u8; 32],
}

impl Codec for WitnessValidateResponse {}

#[derive(Debug, Clone)]
pub struct ValidationEvent {
    pub node_id: String,
    pub success: bool,
}

type ValidationSender = mpsc::Sender<ValidationEvent>;
static VALIDATION_EVENT_SENDER: once_cell::sync::OnceCell<ValidationSender> =
    once_cell::sync::OnceCell::new();

pub fn set_validation_event_sender(tx: ValidationSender) {
    let _ = VALIDATION_EVENT_SENDER.set(tx);
}

fn get_validation_event_sender() -> Option<&'static ValidationSender> {
    VALIDATION_EVENT_SENDER.get()
}

pub async fn witness_validate_handler(
    ctx: Arc<Mutex<Context>>,
    frame: P2PFrame,
    cmd: P2PCommand,
) {
    let req: WitnessValidateRequest = match Codec::decode(&cmd.data) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to decode WitnessValidateRequest: {}", e);
            return;
        }
    };

    tracing::debug!(
        "Validation request from {} (nonce={:?})",
        req.sender_id,
        &req.nonce[..4]
    );

    let sender_id = frame.body.address.clone();
    let resp = WitnessValidateResponse {
        sender_id,
        nonce: req.nonce,
    };

    if let Err(e) = P2PFrame::send::<WitnessValidateResponse>(
        ctx.clone(),
        &Some(resp),
        Entity::Witness,
        Action::ValidateAck,
        false,
    )
    .await
    {
        tracing::error!("Failed to send validation response: {}", e);
    }
}

pub async fn witness_validate_ack_handler(
    _ctx: Arc<Mutex<Context>>,
    _frame: P2PFrame,
    cmd: P2PCommand,
) {
    let resp: WitnessValidateResponse = match Codec::decode(&cmd.data) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to decode WitnessValidateResponse: {}", e);
            return;
        }
    };

    tracing::debug!(
        "Validation ack from {} (nonce={:?})",
        resp.sender_id,
        &resp.nonce[..4]
    );

    if let Some(tx) = get_validation_event_sender() {
        let event = ValidationEvent {
            node_id: resp.sender_id.clone(),
            success: true,
        };
        if let Err(e) = tx.try_send(event) {
            tracing::warn!("Failed to send validation event: {}", e);
        }
    }
}

pub async fn send_validate_request(
    target_ctx: Arc<Mutex<Context>>,
    sender_id: &str,
    nonce: [u8; 32],
) -> anyhow::Result<()> {
    let req = WitnessValidateRequest {
        sender_id: sender_id.to_string(),
        nonce,
    };

    P2PFrame::send::<WitnessValidateRequest>(
        target_ctx,
        &Some(req),
        Entity::Witness,
        Action::Validate,
        false,
    )
    .await?;

    Ok(())
}
