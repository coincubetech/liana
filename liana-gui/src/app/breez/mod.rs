//! Breez SDK integration module
//!
//! This module provides integration with Breez SDK Liquid for Lightning Network
//! and Liquid Network capabilities.
//!
//! ## Features
//! - Lightning Network payments (BOLT11, BOLT12)
//! - Liquid Network transfers
//! - Chain swaps (Bitcoin ↔ Liquid)
//! - Submarine swaps for seamless interoperability
//!
//! ## Key Derivation
//! The Breez wallet is derived from Liana's master mnemonic using a custom
//! derivation path: `m/1776'/0'/0'` (1776 = custom purpose code for Lightning)

pub mod error;

#[cfg(feature = "breez")]
pub mod config;
#[cfg(feature = "breez")]
pub mod error_mapper;
#[cfg(feature = "breez")]
pub mod wallet;
#[cfg(feature = "breez")]
pub mod send;
#[cfg(feature = "breez")]
pub mod receive;
#[cfg(feature = "breez")]
pub mod events;
#[cfg(feature = "breez")]
pub mod storage;
#[cfg(feature = "breez")]
pub mod connection_manager;
#[cfg(feature = "breez")]
pub mod helpers;
#[cfg(feature = "breez")]
pub mod payments;

pub use error::BreezError;

#[cfg(feature = "breez")]
pub use config::BreezConfig;
#[cfg(feature = "breez")]
pub use wallet::BreezWalletManager;
#[cfg(feature = "breez")]
pub use send::BreezSendManager;
#[cfg(feature = "breez")]
pub use receive::{BalanceInfo, BreezReceiveManager};
#[cfg(feature = "breez")]
pub use error_mapper::{error_action_hint, friendly_error_message, full_error_message};
#[cfg(feature = "breez")]
pub use events::{setup_event_listener, BreezEventHandler, BreezEvent};
#[cfg(feature = "breez")]
pub use storage::{generate_lightning_mnemonic, lightning_wallet_exists, store_lightning_mnemonic, load_lightning_mnemonic};
#[cfg(feature = "breez")]
pub use connection_manager::{BreezConnectionManager, ConnectionState};
#[cfg(feature = "breez")]
pub use helpers::{get_or_create_breez_connection, auto_create_lightning_wallet};
#[cfg(feature = "breez")]
pub use payments::{BreezPaymentManager, PaymentInfo, PaymentDirection, PaymentStatus};



