//! Payment history and transaction management

use std::sync::Arc;

use super::BreezError;

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::{
    LiquidSdk, Payment, ListPaymentsRequest, PaymentState, PaymentType,
};

/// Payment manager for listing and managing transaction history
pub struct BreezPaymentManager {
    #[cfg(feature = "breez")]
    sdk: Arc<LiquidSdk>,
}

impl BreezPaymentManager {
    #[cfg(feature = "breez")]
    pub fn new(sdk: Arc<LiquidSdk>) -> Self {
        Self { sdk }
    }

    #[cfg(not(feature = "breez"))]
    pub fn new(_sdk: ()) -> Self {
        Self {}
    }

    /// Get all payments (full history)
    #[cfg(feature = "breez")]
    pub async fn list_payments(&self) -> Result<Vec<Payment>, BreezError> {
        self.sdk
            .list_payments(&ListPaymentsRequest {
                filters: None,
                from_timestamp: None,
                to_timestamp: None,
                offset: None,
                limit: None,
                details: None,
                sort_ascending: Some(false), // Most recent first
                states: None,
            })
            .await
            .map_err(|e| BreezError::PaymentListFailed(e.to_string()))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn list_payments(&self) -> Result<Vec<()>, BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Get payments with filters
    #[cfg(feature = "breez")]
    pub async fn list_payments_filtered(
        &self,
        from_timestamp: Option<i64>,
        to_timestamp: Option<i64>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<Payment>, BreezError> {
        self.sdk
            .list_payments(&ListPaymentsRequest {
                filters: None,
                from_timestamp,
                to_timestamp,
                offset,
                limit,
                details: None,
                sort_ascending: Some(false),
                states: None,
            })
            .await
            .map_err(|e| BreezError::PaymentListFailed(e.to_string()))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn list_payments_filtered(
        &self,
        _from_timestamp: Option<i64>,
        _to_timestamp: Option<i64>,
        _limit: Option<u32>,
        _offset: Option<u32>,
    ) -> Result<Vec<()>, BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Get recent payments (last N)
    #[cfg(feature = "breez")]
    pub async fn list_recent_payments(&self, limit: u32) -> Result<Vec<Payment>, BreezError> {
        self.sdk
            .list_payments(&ListPaymentsRequest {
                filters: None,
                from_timestamp: None,
                to_timestamp: None,
                offset: None,
                limit: Some(limit),
                details: None,
                sort_ascending: Some(false),
                states: None,
            })
            .await
            .map_err(|e| BreezError::PaymentListFailed(e.to_string()))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn list_recent_payments(&self, _limit: u32) -> Result<Vec<()>, BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Get a specific payment by its ID
    #[cfg(feature = "breez")]
    pub async fn get_payment(&self, payment_id: &str) -> Result<Option<Payment>, BreezError> {
        // Note: The SDK might not have a direct get_payment_by_id method
        // We'll need to list all and filter, or use the appropriate SDK method
        let payments = self.list_payments().await?;
        Ok(payments.into_iter().find(|p| {
            p.tx_id.as_ref().map(|id| id == payment_id).unwrap_or(false)
        }))
    }

    #[cfg(not(feature = "breez"))]
    pub async fn get_payment(&self, _payment_id: &str) -> Result<Option<()>, BreezError> {
        Err(BreezError::NotInitialized)
    }
}

/// Simplified payment info for UI display
#[derive(Debug, Clone)]
pub struct PaymentInfo {
    pub id: String,
    pub timestamp: i64,
    pub amount_sat: u64,
    pub direction: PaymentDirection,
    pub status: PaymentStatus,
    pub description: Option<String>,
    pub payment_type: String,
    pub fee_sat: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentDirection {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaymentStatus {
    Pending,
    Complete,
    Failed,
}

#[cfg(feature = "breez")]
impl From<Payment> for PaymentInfo {
    fn from(payment: Payment) -> Self {
        let direction = match payment.payment_type {
            PaymentType::Receive => PaymentDirection::Incoming,
            PaymentType::Send => PaymentDirection::Outgoing,
        };

        let status = match payment.status {
            PaymentState::Pending => PaymentStatus::Pending,
            PaymentState::Complete => PaymentStatus::Complete,
            PaymentState::Failed => PaymentStatus::Failed,
            _ => PaymentStatus::Failed,
        };

        // Extract description from details - PaymentDetails enum doesn't have description field
        // We'll use destination as description for now
        let description = payment.destination.clone();

        PaymentInfo {
            id: payment.tx_id.unwrap_or_else(|| "unknown".to_string()),
            timestamp: payment.timestamp as i64,
            amount_sat: payment.amount_sat,
            direction,
            status,
            description: Some(description.unwrap_or_else(|| "Lightning payment".to_string())),
            payment_type: format!("{:?}", payment.payment_type),
            fee_sat: Some(payment.fees_sat),
        }
    }
}

impl PaymentInfo {
    /// Format amount with direction (+/-)
    pub fn formatted_amount(&self) -> String {
        let sign = match self.direction {
            PaymentDirection::Incoming => "+",
            PaymentDirection::Outgoing => "-",
        };
        format!("{}{} sats", sign, self.amount_sat)
    }

    /// Format timestamp as human-readable date
    pub fn formatted_date(&self) -> String {
        use chrono::{Local, TimeZone};
        
        let dt = Local.timestamp_opt(self.timestamp, 0);
        match dt.single() {
            Some(datetime) => datetime.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => "Unknown date".to_string(),
        }
    }

    /// Get status as display string
    pub fn status_display(&self) -> &str {
        match self.status {
            PaymentStatus::Pending => "Pending",
            PaymentStatus::Complete => "Complete",
            PaymentStatus::Failed => "Failed",
        }
    }

    /// Get direction as display string
    pub fn direction_display(&self) -> &str {
        match self.direction {
            PaymentDirection::Incoming => "Received",
            PaymentDirection::Outgoing => "Sent",
        }
    }
}
