//! Breez SDK initialization

use std::path::PathBuf;
use std::sync::Arc;

use liana::miniscript::bitcoin;
use tokio::sync::mpsc;

use super::{wallet::BreezWalletManager, events::BreezEvent, BreezError};

/// Initialize Breez SDK in background
pub async fn initialize_breez_sdk(
    mnemonic: String,
    network: bitcoin::Network,
    data_dir: PathBuf,
) -> Result<(Arc<BreezWalletManager>, mpsc::UnboundedReceiver<BreezEvent>), BreezError> {
    log::info!("🚀 Starting Breez SDK initialization...");
    log::info!("Network: {:?}", network);
    log::info!("Data dir: {:?}", data_dir);
    
    // Check API key before attempting initialization
    if std::env::var("BREEZ_API_KEY").ok().filter(|k| !k.is_empty()).is_none() {
        return Err(BreezError::Config(
            "BREEZ_API_KEY not set. Get a free API key from https://breez.technology/request-api-key/".to_string()
        ));
    }
    
    log::info!("⏳ Initializing wallet manager...");
    // Initialize wallet manager
    let manager = BreezWalletManager::initialize(&mnemonic, network, &data_dir).await?;
    let manager = Arc::new(manager);
    log::info!("✅ Wallet manager initialized");

    log::info!("⏳ Setting up event listener...");
    // Setup event listener
    let sdk = manager.sdk()?;
    let event_receiver = super::events::setup_event_listener(sdk).await?;
    log::info!("✅ Event listener ready");
    
    log::info!("🎉 Breez SDK initialization complete!");

    Ok((manager, event_receiver))
}

/// Get mnemonic from Liana signer as a single string
pub fn get_mnemonic_from_signer(signer: &crate::signer::Signer) -> String {
    signer.mnemonic().join(" ")
}

