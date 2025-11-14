//! Breez wallet initialization and key derivation

use std::path::Path;
use std::sync::Arc;

use liana::miniscript::bitcoin;

use super::{BreezConfig, BreezError};

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::{ConnectRequest, LiquidSdk};

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
    /// Initialize Breez SDK with mnemonic
    #[cfg(feature = "breez")]
    pub async fn initialize(
        liana_mnemonic: &str,
        network: bitcoin::Network,
        data_dir: &Path,
    ) -> Result<Self, BreezError> {
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

    /// Create a placeholder manager (used during initialization)
    #[cfg(feature = "breez")]
    pub fn new_placeholder(network: bitcoin::Network) -> Self {
        Self {
            sdk: None,
            network,
            config: BreezConfig::new(network, String::new()),
        }
    }

    #[cfg(not(feature = "breez"))]
    pub fn new_placeholder(network: bitcoin::Network) -> Self {
        Self { network }
    }
}

