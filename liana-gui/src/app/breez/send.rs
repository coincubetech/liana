//! Send payment functionality

use std::sync::Arc;

use super::BreezError;

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::{
    LiquidSdk, PayAmount, PaymentMethod, PrepareSendRequest, PrepareSendResponse,
    SendPaymentRequest, SendPaymentResponse, Limits
};

/// Breez send payment manager
pub struct BreezSendManager {
    #[cfg(feature = "breez")]
    sdk: Arc<LiquidSdk>,
}

impl BreezSendManager {
    #[cfg(feature = "breez")]
    pub fn new(sdk: Arc<LiquidSdk>) -> Self {
        Self { sdk }
    }

    #[cfg(not(feature = "breez"))]
    pub fn new(_sdk: ()) -> Self {
        Self {}
    }

    /// Prepare a send payment (validates destination and fetches fees)
    #[cfg(feature = "breez")]
    pub async fn prepare_send(
        &self,
        destination: String,
        amount_sat: Option<u64>,
    ) -> Result<PrepareSendResponse, BreezError> {
        let request = PrepareSendRequest {
            destination,
            amount: amount_sat.map(|sat| PayAmount::Bitcoin {
                receiver_amount_sat: sat,
            }),
        };

        self.sdk
            .prepare_send_payment(&request)
            .await
            .map_err(|e| BreezError::PrepareFailed(e.to_string()))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn prepare_send(
        &self,
        _destination: String,
        _amount_sat: Option<u64>,
    ) -> Result<(), BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Send a payment
    #[cfg(feature = "breez")]
    pub async fn send_payment(
        &self,
        prepare_response: &PrepareSendResponse,
    ) -> Result<SendPaymentResponse, BreezError> {
        self.sdk
            .send_payment(&SendPaymentRequest {
                prepare_response: prepare_response.clone(),
                use_asset_fees: None,
                payer_note: None,
            })
            .await
            .map_err(|e| BreezError::SendFailed(e.to_string()))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn send_payment(&self, _prepare_response: &()) -> Result<(), BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Fetch send payment limits for a given payment method
    #[cfg(feature = "breez")]
    pub async fn fetch_payment_limits(
        &self,
        payment_method: PaymentMethod,
    ) -> Result<Limits, BreezError> {
        self.sdk
            .fetch_lightning_limits()
            .await
            .map_err(|e| BreezError::LimitsFetchFailed(e.to_string()))
            .and_then(|limits| {
                match payment_method {
                    PaymentMethod::Bolt11Invoice => Ok(limits.send),
                    _ => Err(BreezError::LimitsFetchFailed("Unsupported payment method".to_string())),
                }
            })
    }

    #[cfg(not(feature = "breez"))]
    pub async fn fetch_payment_limits(&self, _payment_method: ()) -> Result<(), BreezError> {
        Err(BreezError::NotInitialized)
    }
}

