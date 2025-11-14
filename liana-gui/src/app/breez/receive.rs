//! Receive payment functionality

use std::sync::Arc;

use super::BreezError;

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::{
    GetInfoResponse, Limits, LiquidSdk, PaymentMethod, PrepareReceiveRequest,
    PrepareReceiveResponse, ReceiveAmount, ReceivePaymentRequest, ReceivePaymentResponse,
};

/// Balance information
#[derive(Debug, Clone, Default)]
pub struct BalanceInfo {
    pub lightning_balance_sat: u64,
    pub liquid_balance_sat: u64,
    pub pending_send_sat: u64,
    pub pending_receive_sat: u64,
}

/// Breez receive payment manager
pub struct BreezReceiveManager {
    #[cfg(feature = "breez")]
    sdk: Arc<LiquidSdk>,
}

impl BreezReceiveManager {
    #[cfg(feature = "breez")]
    pub fn new(sdk: Arc<LiquidSdk>) -> Self {
        Self { sdk }
    }

    #[cfg(not(feature = "breez"))]
    pub fn new(_sdk: ()) -> Self {
        Self {}
    }

    /// Get wallet info including balances
    #[cfg(feature = "breez")]
    pub async fn get_wallet_info(&self) -> Result<GetInfoResponse, BreezError> {
        self.sdk
            .get_info()
            .await
            .map_err(|e| BreezError::BalanceFetchFailed(e.to_string()))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn get_wallet_info(&self) -> Result<(), BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Get balance information
    #[cfg(feature = "breez")]
    pub async fn get_balance(&self) -> Result<BalanceInfo, BreezError> {
        let info = self.get_wallet_info().await?;

        Ok(BalanceInfo {
            lightning_balance_sat: info.wallet_info.balance_sat,
            liquid_balance_sat: info.wallet_info.pending_send_sat,
            pending_send_sat: info.wallet_info.pending_send_sat,
            pending_receive_sat: info.wallet_info.pending_receive_sat,
        })
    }

    #[cfg(not(feature = "breez"))]
    pub async fn get_balance(&self) -> Result<BalanceInfo, BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Prepare to receive a payment (generates invoice/address)
    #[cfg(feature = "breez")]
    pub async fn prepare_receive(
        &self,
        amount_sat: Option<u64>,
        _description: Option<String>,
    ) -> Result<PrepareReceiveResponse, BreezError> {
        let request = PrepareReceiveRequest {
            payment_method: PaymentMethod::Bolt11Invoice,
            amount: amount_sat.map(|sat| ReceiveAmount::Bitcoin {
                payer_amount_sat: sat,
            }),
        };

        self.sdk
            .prepare_receive_payment(&request)
            .await
            .map_err(|e| BreezError::PrepareFailed(e.to_string()))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn prepare_receive(
        &self,
        _amount_sat: Option<u64>,
        _description: Option<String>,
    ) -> Result<(), BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Generate a Lightning invoice
    #[cfg(feature = "breez")]
    pub async fn receive_payment(
        &self,
        prepare_response: &PrepareReceiveResponse,
        _description: Option<String>,
    ) -> Result<ReceivePaymentResponse, BreezError> {
        self.sdk
            .receive_payment(&ReceivePaymentRequest {
                prepare_response: prepare_response.clone(),
                description: None,
                use_description_hash: None,
                payer_note: None,
            })
            .await
            .map_err(|e| BreezError::ReceiveFailed(e.to_string()))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn receive_payment(
        &self,
        _prepare_response: &(),
        _description: Option<String>,
    ) -> Result<(), BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Fetch receive payment limits
    #[cfg(feature = "breez")]
    pub async fn fetch_receive_limits(&self) -> Result<Limits, BreezError> {
        self.sdk
            .fetch_lightning_limits()
            .await
            .map_err(|e| BreezError::LimitsFetchFailed(e.to_string()))
            .map(|limits| limits.receive)
    }

    #[cfg(not(feature = "breez"))]
    pub async fn fetch_receive_limits(&self) -> Result<(), BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Get a Bitcoin on-chain address (for chain swaps)
    #[cfg(feature = "breez")]
    pub async fn get_bitcoin_address(&self) -> Result<String, BreezError> {
        // Note: Implement based on Breez SDK Liquid API
        // This might require calling a different method or preparing a swap
        Err(BreezError::ReceiveFailed(
            "Bitcoin address generation not yet implemented".to_string(),
        ))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn get_bitcoin_address(&self) -> Result<String, BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Get a Liquid address
    #[cfg(feature = "breez")]
    pub async fn get_liquid_address(&self) -> Result<String, BreezError> {
        // Note: Implement based on Breez SDK Liquid API
        Err(BreezError::ReceiveFailed(
            "Liquid address generation not yet implemented".to_string(),
        ))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn get_liquid_address(&self) -> Result<String, BreezError> {
        Err(BreezError::NotInitialized)
    }
}
