//! Breez SDK configuration

use liana::miniscript::bitcoin;

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::Config;

/// Breez SDK configuration wrapper
#[derive(Debug, Clone)]
pub struct BreezConfig {
    #[cfg(feature = "breez")]
    pub config: Config,
    #[cfg(not(feature = "breez"))]
    _phantom: std::marker::PhantomData<()>,
}

impl BreezConfig {
    /// Create a new Breez configuration for the given network
    #[cfg(feature = "breez")]
    pub fn new(network: bitcoin::Network, working_dir: String) -> Self {
        use breez_sdk_liquid::prelude::LiquidNetwork;

        let liquid_network = match network {
            bitcoin::Network::Bitcoin => LiquidNetwork::Mainnet,
            bitcoin::Network::Testnet | bitcoin::Network::Testnet4 | bitcoin::Network::Signet => {
                LiquidNetwork::Testnet
            }
            bitcoin::Network::Regtest => LiquidNetwork::Regtest,
        };

        // Get API key from environment variable - REQUIRED for Breez SDK Liquid
        let api_key = std::env::var("BREEZ_API_KEY")
            .ok()
            .filter(|k| !k.is_empty());

        if api_key.is_none() {
            log::error!("❌ BREEZ_API_KEY not set! Breez SDK Liquid requires an API key to initialize.");
            log::error!("Please:");
            log::error!("1. Get a FREE API key from: https://breez.technology/request-api-key/");
            log::error!("2. Add it to your .env file: BREEZ_API_KEY=your_key_here");
            log::error!("3. Restart the application");
        } else {
            log::info!("✅ Breez API key found, initializing SDK...");
        }

        // Use the SDK's default configuration methods
        let mut config = match liquid_network {
            LiquidNetwork::Mainnet => Config::mainnet_esplora(api_key.clone()),
            LiquidNetwork::Testnet => Config::testnet_esplora(api_key),
            LiquidNetwork::Regtest => Config::regtest_esplora(),
        };

        // Override the working directory
        config.working_dir = working_dir;

        Self { config }
    }

    #[cfg(not(feature = "breez"))]
    pub fn new(_network: bitcoin::Network, _working_dir: String) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the Breez SDK config (only available with breez feature)
    #[cfg(feature = "breez")]
    pub fn as_sdk_config(&self) -> &Config {
        &self.config
    }
}

