//! Breez SDK event handling

use std::sync::Arc;
use tokio::sync::mpsc;

use super::BreezError;

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::{EventListener, LiquidSdk, SdkEvent};

/// Breez event types
#[derive(Debug, Clone)]
pub enum BreezEvent {
    PaymentSucceeded { payment_id: String, amount_sat: u64 },
    PaymentFailed { payment_id: String, error: String },
    PaymentPending { payment_id: String },
    BalanceUpdated,
    SyncComplete,
    Connected,
    Disconnected,
}

/// Event handler that converts SDK events to our internal event type
pub struct BreezEventHandler {
    sender: mpsc::UnboundedSender<BreezEvent>,
}

impl BreezEventHandler {
    pub fn new(sender: mpsc::UnboundedSender<BreezEvent>) -> Self {
        Self { sender }
    }

    #[cfg(feature = "breez")]
    fn handle_sdk_event(&self, event: SdkEvent) {
        let breez_event = match event {
            SdkEvent::PaymentSucceeded { details } => BreezEvent::PaymentSucceeded {
                payment_id: details.destination.unwrap_or_default(),
                amount_sat: details.amount_sat,
            },
            SdkEvent::PaymentFailed { details } => BreezEvent::PaymentFailed {
                payment_id: details.destination.clone().unwrap_or_else(|| "unknown".to_string()),
                error: format!("Payment failed: {:?}", details),
            },
            SdkEvent::PaymentPending { details } => BreezEvent::PaymentPending {
                payment_id: details.destination.unwrap_or_default(),
            },
            SdkEvent::Synced => BreezEvent::SyncComplete,
            // Handle other variants we don't specifically process
            SdkEvent::PaymentRefundable { .. } 
            | SdkEvent::PaymentRefunded { .. }
            | SdkEvent::PaymentRefundPending { .. }
            | SdkEvent::PaymentWaitingConfirmation { .. }
            | SdkEvent::PaymentWaitingFeeAcceptance { .. }
            | _ => return, // Silently ignore unknown/unhandled events
        };

        let _ = self.sender.send(breez_event);
    }
}

#[cfg(feature = "breez")]
impl EventListener for BreezEventHandler {
    fn on_event(&self, event: SdkEvent) {
        self.handle_sdk_event(event);
    }
}

/// Setup event listener for Breez SDK
#[cfg(feature = "breez")]
pub async fn setup_event_listener(
    sdk: Arc<LiquidSdk>,
) -> Result<mpsc::UnboundedReceiver<BreezEvent>, BreezError> {
    let (sender, receiver) = mpsc::unbounded_channel();

    sdk.add_event_listener(Box::new(BreezEventHandler::new(sender)))
        .await
        .map_err(|e| BreezError::EventListenerFailed(e.to_string()))?;

    Ok(receiver)
}

#[cfg(not(feature = "breez"))]
pub async fn setup_event_listener(_sdk: ()) -> Result<mpsc::UnboundedReceiver<BreezEvent>, BreezError> {
    Err(BreezError::NotInitialized)
}

