//! Breez wallet initialization and key derivation

use std::path::Path;
use std::sync::Arc;

use liana::miniscript::bitcoin;

use super::{BreezConfig, BreezError};

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::{ConnectRequest, LiquidSdk};

#[cfg(feature = "breez")]
use bip39::Mnemonic;

#[cfg(feature = "breez")]
use liana::miniscript::bitcoin::bip32::{DerivationPath, Xpriv};

#[cfg(feature = "breez")]
use bitcoin::secp256k1::Secp256k1;

/// Custom derivation path for Breez wallet from Liana's master seed
/// m/1776'/0'/0' where 1776 is our custom purpose code for Lightning
pub const BREEZ_DERIVATION_PATH: &str = "m/1776'/0'/0'";

/// Breez wallet manager
#[derive(Clone)]
pub struct BreezWalletManager {
    #[cfg(feature = "breez")]
    pub sdk: Option<Arc<LiquidSdk>>,
    network: bitcoin::Network,
    #[cfg(feature = "breez")]
    #[allow(dead_code)]
    config: BreezConfig,
}

impl std::fmt::Debug for BreezWalletManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BreezWalletManager")
            .field("network", &self.network)
            .finish_non_exhaustive()
    }
}

impl BreezWalletManager {
    /// Derive Breez wallet seed from Liana's master mnemonic
    #[cfg(feature = "breez")]
    pub fn derive_breez_seed(
        liana_mnemonic: &str,
        network: bitcoin::Network,
    ) -> Result<Vec<u8>, BreezError> {
        // Parse Liana's mnemonic
        let mnemonic = Mnemonic::parse(liana_mnemonic)
            .map_err(|e| BreezError::InvalidMnemonic(e.to_string()))?;

        // Generate seed from mnemonic (no passphrase)
        let seed_bytes = mnemonic.to_seed("");

        // Derive extended private key
        let master_key = Xpriv::new_master(network, &seed_bytes)
            .map_err(|e| BreezError::DerivationFailed(e.to_string()))?;

        // Derive child key at custom path
        let derivation_path: DerivationPath = BREEZ_DERIVATION_PATH
            .parse()
            .map_err(|e| BreezError::InvalidPath(format!("{:?}", e)))?;

        let derived_key = master_key
            .derive_priv(&Secp256k1::new(), &derivation_path)
            .map_err(|e| BreezError::DerivationFailed(e.to_string()))?;

        // Convert to seed bytes for Breez SDK
        Ok(derived_key.private_key.secret_bytes().to_vec())
    }

    #[cfg(not(feature = "breez"))]
    pub fn derive_breez_seed(
        _liana_mnemonic: &str,
        _network: bitcoin::Network,
    ) -> Result<Vec<u8>, BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Initialize Breez SDK with derived seed
    #[cfg(feature = "breez")]
    pub async fn initialize(
        liana_mnemonic: &str,
        network: bitcoin::Network,
        data_dir: &Path,
    ) -> Result<Self, BreezError> {
        // Derive seed
        let _breez_seed = Self::derive_breez_seed(liana_mnemonic, network)?;

        // Create Breez SDK configuration
        let config = BreezConfig::new(
            network,
            data_dir.join("breez").to_string_lossy().to_string(),
        );

        // Connect to Breez SDK using mnemonic directly
        let connect_request = ConnectRequest {
            config: config.config.clone(),
            mnemonic: Some(liana_mnemonic.to_string()),
            passphrase: None,
            seed: None,
        };

        let sdk = LiquidSdk::connect(connect_request)
            .await
            .map_err(|e| BreezError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            sdk: Some(sdk),
            network,
            config,
        })
    }

    #[cfg(not(feature = "breez"))]
    pub async fn initialize(
        _liana_mnemonic: &str,
        network: bitcoin::Network,
        _data_dir: &Path,
    ) -> Result<Self, BreezError> {
        Ok(Self { network })
    }

    /// Get SDK instance
    #[cfg(feature = "breez")]
    pub fn sdk(&self) -> Result<Arc<LiquidSdk>, BreezError> {
        self.sdk.clone().ok_or(BreezError::NotInitialized)
    }

    #[cfg(not(feature = "breez"))]
    pub fn sdk(&self) -> Result<(), BreezError> {
        Err(BreezError::NotInitialized)
    }

    /// Get network
    pub fn network(&self) -> bitcoin::Network {
        self.network
    }

    /// Disconnect from Breez SDK
    #[cfg(feature = "breez")]
    pub async fn disconnect(&mut self) -> Result<(), BreezError> {
        if let Some(sdk) = self.sdk.take() {
            if let Ok(sdk) = Arc::try_unwrap(sdk) {
                sdk.disconnect()
                    .await
                    .map_err(|e| BreezError::SdkError(e.to_string()))?;
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "breez"))]
    pub async fn disconnect(&mut self) -> Result<(), BreezError> {
        Ok(())
    }
}

