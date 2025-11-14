//! Error types for Breez SDK integration

use std::fmt;

#[derive(Debug, Clone)]
pub enum BreezError {
    // Initialization errors
    NotInitialized,
    InvalidMnemonic(String),
    DerivationFailed(String),
    InvalidPath(String),
    ConnectionFailed(String),
    Config(String),

    // Payment errors
    InvalidDestination,
    AmountOutOfLimits { min: u64, max: u64 },
    PrepareFailed(String),
    SendFailed(String),
    ReceiveFailed(String),
    LimitsFetchFailed(String),

    // Balance errors
    BalanceFetchFailed(String),

    // Event handling errors
    EventListenerFailed(String),

    // Payment history errors
    PaymentListFailed(String),
    PaymentFetchFailed(String),

    // General errors
    SdkError(String),
}

impl fmt::Display for BreezError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "Breez SDK not initialized"),
            Self::InvalidMnemonic(msg) => write!(f, "Invalid mnemonic: {}", msg),
            Self::DerivationFailed(msg) => write!(f, "Key derivation failed: {}", msg),
            Self::InvalidPath(msg) => write!(f, "Invalid derivation path: {}", msg),
            Self::ConnectionFailed(msg) => write!(f, "Failed to connect to Breez SDK: {}", msg),
            Self::Config(msg) => write!(f, "Configuration error: {}", msg),
            Self::InvalidDestination => write!(f, "Invalid payment destination"),
            Self::AmountOutOfLimits { min, max } => {
                write!(f, "Amount must be between {} and {} sats", min, max)
            }
            Self::PrepareFailed(msg) => write!(f, "Failed to prepare payment: {}", msg),
            Self::SendFailed(msg) => write!(f, "Failed to send payment: {}", msg),
            Self::ReceiveFailed(msg) => write!(f, "Failed to receive payment: {}", msg),
            Self::LimitsFetchFailed(msg) => write!(f, "Failed to fetch payment limits: {}", msg),
            Self::BalanceFetchFailed(msg) => write!(f, "Failed to fetch balance: {}", msg),
            Self::EventListenerFailed(msg) => write!(f, "Failed to setup event listener: {}", msg),
            Self::PaymentListFailed(msg) => write!(f, "Failed to list payments: {}", msg),
            Self::PaymentFetchFailed(msg) => write!(f, "Failed to fetch payment: {}", msg),
            Self::SdkError(msg) => write!(f, "Breez SDK error: {}", msg),
        }
    }
}

impl std::error::Error for BreezError {}

#[cfg(feature = "breez")]
use breez_sdk_liquid::error::SdkError;

#[cfg(feature = "breez")]
impl From<SdkError> for BreezError {
    fn from(err: SdkError) -> Self {
        Self::SdkError(err.to_string())
    }
}
