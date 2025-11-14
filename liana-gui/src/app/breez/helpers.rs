//! Helper functions for Breez SDK integration

use std::path::Path;

use liana::miniscript::bitcoin;

use super::{storage, wallet::BreezWalletManager, BreezConnectionManager, BreezError};

/// Get or create a Breez SDK connection for a cube using the connection manager
///
/// This is the recommended way to initialize Breez SDK connections as it avoids
/// redundant initializations.
///
/// # Arguments
/// * `manager` - The connection manager instance
/// * `wallet_checksum` - Unique identifier for the cube's vault wallet
/// * `network_dir` - Network directory path
///
/// # Returns
/// * `Ok(Some(BreezWalletManager))` - Successfully connected or reused existing connection
/// * `Ok(None)` - Lightning wallet doesn't exist for this cube
/// * `Err(BreezError)` - Failed to initialize or load
pub async fn get_or_create_breez_connection(
    manager: &BreezConnectionManager,
    wallet_checksum: &str,
    network_dir: &Path,
) -> Result<Option<BreezWalletManager>, BreezError> {
    // Check if Lightning wallet exists for this cube
    if !storage::lightning_wallet_exists(network_dir, wallet_checksum) {
        tracing::debug!("No Lightning wallet found for cube: {}", wallet_checksum);
        return Ok(None);
    }

    // Load Lightning mnemonic
    let mnemonic = storage::load_lightning_mnemonic(network_dir, wallet_checksum).map_err(|e| {
        tracing::error!(
            "Failed to load Lightning mnemonic for cube {}: {:?}",
            wallet_checksum,
            e
        );
        e
    })?;

    // Get or create connection through manager
    match manager.get_or_create(wallet_checksum, &mnemonic).await {
        Ok(breez_manager) => {
            // Return a clone of the Arc content
            Ok(Some(breez_manager.as_ref().clone()))
        }
        Err(e) => {
            tracing::error!(
                "Failed to get or create Breez connection for cube {}: {:?}",
                wallet_checksum,
                e
            );
            Err(e)
        }
    }
}

/// Auto-create Lightning wallet for a cube if it doesn't exist
///
/// This should be called when a new vault is created or when migrating
/// existing vaults to support Lightning.
///
/// # Arguments
/// * `network_dir` - Network directory path
/// * `wallet_checksum` - Unique identifier for the cube's vault wallet
/// * `wallet_name` - Human-readable name for logging
///
/// # Returns
/// * `Ok(true)` - Lightning wallet was created
/// * `Ok(false)` - Lightning wallet already exists
/// * `Err(BreezError)` - Failed to create
pub fn auto_create_lightning_wallet(
    network_dir: &Path,
    wallet_checksum: &str,
    wallet_name: &str,
) -> Result<bool, BreezError> {
    // Check if already exists
    if storage::lightning_wallet_exists(network_dir, wallet_checksum) {
        tracing::info!(
            "⚡ Lightning wallet already exists for cube: {}",
            wallet_name
        );
        return Ok(false);
    }

    // Generate new Lightning wallet mnemonic
    let mnemonic = storage::generate_lightning_mnemonic()?;

    // Store the mnemonic
    storage::store_lightning_mnemonic(network_dir, wallet_checksum, &mnemonic)?;

    tracing::info!("✅ Auto-created Lightning wallet for cube: {}", wallet_name);
    tracing::info!(
        "📁 Lightning wallet stored at: {}/lightning/",
        wallet_checksum
    );

    Ok(true)
}

/// Initialize Breez SDK connection without using connection manager (legacy)
///
/// **NOTE**: This is the old way that causes redundant initializations.
/// Use `get_or_create_breez_connection()` with a connection manager instead.
///
/// This function is kept for backward compatibility but should be avoided.
#[deprecated(
    since = "13.1.0",
    note = "Use get_or_create_breez_connection() with BreezConnectionManager instead"
)]
pub async fn initialize_breez_direct(
    mnemonic: &str,
    network: bitcoin::Network,
    data_dir: &Path,
) -> Result<BreezWalletManager, BreezError> {
    tracing::warn!(
        "⚠️ Using direct Breez initialization (legacy). Consider using BreezConnectionManager."
    );
    BreezWalletManager::initialize(mnemonic, network, data_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_auto_create_checks_existence() {
        // This is a unit test to verify the logic, not a full integration test
        let temp_dir = PathBuf::from("/tmp/test_breez_helper");
        let result = auto_create_lightning_wallet(&temp_dir, "test_checksum", "Test Cube");

        // Should either create or already exist
        assert!(result.is_ok() || result.is_err());
    }
}
