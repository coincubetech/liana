//! Breez SDK Connection Manager
//!
//! Manages Breez SDK connections per cube to avoid redundant initializations.
//! Each cube (Bitcoin wallet) maintains its own separate Lightning wallet connection.
//!
//! ## Architecture
//! - One SDK connection per cube (identified by wallet_checksum)
//! - Connections are cached and reused when switching between cubes
//! - Automatic cleanup when cubes are removed
//! - Thread-safe using Arc<RwLock>

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use liana::miniscript::bitcoin;

use super::{wallet::BreezWalletManager, BreezError};

/// Connection state for a Breez SDK instance
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Not yet initialized
    NotInitialized,
    /// Currently initializing (async operation in progress)
    Initializing,
    /// Successfully connected and ready
    Connected,
    /// Disconnected (can be reconnected)
    Disconnected,
    /// Error occurred during initialization or operation
    Error(String),
}

/// Metadata about a cached connection
#[derive(Debug, Clone)]
struct ConnectionEntry {
    manager: Arc<BreezWalletManager>,
    state: ConnectionState,
    #[allow(dead_code)] // Used for debugging and future features
    wallet_checksum: String,
    last_used: std::time::SystemTime,
}

/// Breez SDK Connection Manager
///
/// Manages a pool of Breez SDK connections, one per cube.
/// Ensures connections are reused and properly cleaned up.
pub struct BreezConnectionManager {
    /// Map of wallet_checksum -> connection entry
    connections: Arc<RwLock<HashMap<String, ConnectionEntry>>>,
    /// Network this manager is for
    network: bitcoin::Network,
    /// Base data directory
    data_dir: PathBuf,
}

impl BreezConnectionManager {
    /// Create a new connection manager
    pub fn new(network: bitcoin::Network, data_dir: PathBuf) -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            network,
            data_dir,
        }
    }

    /// Get or create a Breez SDK connection for a specific cube
    ///
    /// If a connection already exists and is healthy, it will be reused.
    /// Otherwise, a new connection will be initialized.
    ///
    /// # Arguments
    /// * `wallet_checksum` - Unique identifier for the cube's vault wallet
    /// * `mnemonic` - Lightning wallet mnemonic for this cube
    pub async fn get_or_create(
        &self,
        wallet_checksum: &str,
        mnemonic: &str,
    ) -> Result<Arc<BreezWalletManager>, BreezError> {
        // Check if connection already exists
        {
            let connections = self.connections.read().await;
            if let Some(entry) = connections.get(wallet_checksum) {
                match &entry.state {
                    ConnectionState::Connected => {
                        tracing::info!(
                            "♻️ Reusing existing Breez SDK connection for cube: {}",
                            wallet_checksum
                        );

                        // Clone the manager before dropping the lock
                        let manager_clone = entry.manager.clone();

                        // Drop read lock and update last used time
                        drop(connections);
                        let mut connections = self.connections.write().await;
                        if let Some(entry) = connections.get_mut(wallet_checksum) {
                            entry.last_used = std::time::SystemTime::now();
                        }

                        return Ok(manager_clone);
                    }
                    ConnectionState::Initializing => {
                        tracing::warn!(
                            "⏳ Connection already initializing for cube: {}",
                            wallet_checksum
                        );
                        return Err(BreezError::Config(
                            "Connection initialization already in progress".to_string(),
                        ));
                    }
                    ConnectionState::Disconnected => {
                        tracing::info!(
                            "🔄 Connection was disconnected, will reinitialize for cube: {}",
                            wallet_checksum
                        );
                        // Will fall through to reinitialize
                    }
                    ConnectionState::Error(e) => {
                        tracing::warn!(
                            "⚠️ Previous connection had error, will retry for cube {}: {}",
                            wallet_checksum,
                            e
                        );
                        // Will fall through to reinitialize
                    }
                    ConnectionState::NotInitialized => {
                        // Will fall through to initialize
                    }
                }
            }
        }

        // Mark as initializing
        {
            let mut connections = self.connections.write().await;
            connections.insert(
                wallet_checksum.to_string(),
                ConnectionEntry {
                    manager: Arc::new(BreezWalletManager::new_placeholder(self.network)),
                    state: ConnectionState::Initializing,
                    wallet_checksum: wallet_checksum.to_string(),
                    last_used: std::time::SystemTime::now(),
                },
            );
        }

        tracing::info!(
            "🔌 Initializing new Breez SDK connection for cube: {}",
            wallet_checksum
        );

        // Initialize new connection
        let breez_data_dir = self.data_dir.join(wallet_checksum).join("breez");

        let manager =
            match BreezWalletManager::initialize(mnemonic, self.network, &breez_data_dir).await {
                Ok(manager) => {
                    tracing::info!(
                        "✅ Breez SDK connection initialized successfully for cube: {}",
                        wallet_checksum
                    );
                    manager
                }
                Err(e) => {
                    tracing::error!(
                        "❌ Failed to initialize Breez SDK for cube {}: {:?}",
                        wallet_checksum,
                        e
                    );

                    // Mark as error
                    let mut connections = self.connections.write().await;
                    if let Some(entry) = connections.get_mut(wallet_checksum) {
                        entry.state = ConnectionState::Error(e.to_string());
                    }

                    return Err(e);
                }
            };

        let manager_arc = Arc::new(manager);

        // Update to connected state
        {
            let mut connections = self.connections.write().await;
            connections.insert(
                wallet_checksum.to_string(),
                ConnectionEntry {
                    manager: manager_arc.clone(),
                    state: ConnectionState::Connected,
                    wallet_checksum: wallet_checksum.to_string(),
                    last_used: std::time::SystemTime::now(),
                },
            );
        }

        Ok(manager_arc)
    }

    /// Disconnect a specific cube's Breez SDK connection
    ///
    /// This should be called when:
    /// - User switches to a different cube
    /// - Cube is deleted
    /// - App is shutting down
    pub async fn disconnect(&self, wallet_checksum: &str) -> Result<(), BreezError> {
        let entry = {
            let mut connections = self.connections.write().await;
            connections.remove(wallet_checksum)
        };

        if let Some(mut entry) = entry {
            tracing::info!("🔌 Disconnecting Breez SDK for cube: {}", wallet_checksum);

            // Try to unwrap Arc and disconnect
            if let Ok(mut manager) = Arc::try_unwrap(entry.manager) {
                manager.disconnect().await?;
                tracing::info!("✅ Breez SDK disconnected for cube: {}", wallet_checksum);
            } else {
                tracing::warn!(
                    "⚠️ Could not unwrap Arc for cube {} - connection still has references",
                    wallet_checksum
                );
                entry.state = ConnectionState::Disconnected;
            }
        } else {
            tracing::debug!("No active connection found for cube: {}", wallet_checksum);
        }

        Ok(())
    }

    /// Disconnect all Breez SDK connections
    ///
    /// Should be called when:
    /// - Application is shutting down
    /// - User logs out
    /// - Network is changed
    pub async fn disconnect_all(&self) -> Result<(), BreezError> {
        tracing::info!("🔌 Disconnecting all Breez SDK connections...");

        let checksums: Vec<String> = {
            let connections = self.connections.read().await;
            connections.keys().cloned().collect()
        };

        let mut errors = Vec::new();

        for checksum in checksums {
            if let Err(e) = self.disconnect(&checksum).await {
                tracing::error!("Failed to disconnect cube {}: {:?}", checksum, e);
                errors.push((checksum, e));
            }
        }

        if !errors.is_empty() {
            let error_msg = format!("Failed to disconnect {} connections", errors.len());
            tracing::error!("{}: {:?}", error_msg, errors);
            return Err(BreezError::SdkError(error_msg));
        }

        tracing::info!("✅ All Breez SDK connections disconnected");
        Ok(())
    }

    /// Get connection state for a specific cube
    pub async fn get_state(&self, wallet_checksum: &str) -> ConnectionState {
        let connections = self.connections.read().await;
        connections
            .get(wallet_checksum)
            .map(|e| e.state.clone())
            .unwrap_or(ConnectionState::NotInitialized)
    }

    /// Check if a connection exists and is connected
    pub async fn is_connected(&self, wallet_checksum: &str) -> bool {
        matches!(
            self.get_state(wallet_checksum).await,
            ConnectionState::Connected
        )
    }

    /// Get list of all active connections
    pub async fn list_connections(&self) -> Vec<(String, ConnectionState)> {
        let connections = self.connections.read().await;
        connections
            .iter()
            .map(|(checksum, entry)| (checksum.clone(), entry.state.clone()))
            .collect()
    }

    /// Clean up stale connections (disconnected or error state for > 1 hour)
    pub async fn cleanup_stale(&self) -> Result<(), BreezError> {
        let now = std::time::SystemTime::now();
        let stale_duration = std::time::Duration::from_secs(3600); // 1 hour

        let stale_checksums: Vec<String> = {
            let connections = self.connections.read().await;
            connections
                .iter()
                .filter(|(_, entry)| {
                    matches!(
                        entry.state,
                        ConnectionState::Disconnected | ConnectionState::Error(_)
                    ) && now
                        .duration_since(entry.last_used)
                        .map(|d| d > stale_duration)
                        .unwrap_or(false)
                })
                .map(|(checksum, _)| checksum.clone())
                .collect()
        };

        for checksum in stale_checksums {
            tracing::info!("🧹 Cleaning up stale connection for cube: {}", checksum);
            self.disconnect(&checksum).await?;
        }

        Ok(())
    }

    /// Get number of active connections
    pub async fn connection_count(&self) -> usize {
        let connections = self.connections.read().await;
        connections.len()
    }
}

impl Clone for BreezConnectionManager {
    fn clone(&self) -> Self {
        Self {
            connections: self.connections.clone(),
            network: self.network,
            data_dir: self.data_dir.clone(),
        }
    }
}

impl std::fmt::Debug for BreezConnectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BreezConnectionManager")
            .field("network", &self.network)
            .field("data_dir", &self.data_dir)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let manager =
            BreezConnectionManager::new(bitcoin::Network::Testnet, PathBuf::from("/tmp/test"));
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn test_connection_state() {
        let manager =
            BreezConnectionManager::new(bitcoin::Network::Testnet, PathBuf::from("/tmp/test"));

        let state = manager.get_state("test_wallet").await;
        assert_eq!(state, ConnectionState::NotInitialized);
    }

    #[tokio::test]
    async fn test_is_connected() {
        let manager =
            BreezConnectionManager::new(bitcoin::Network::Testnet, PathBuf::from("/tmp/test"));

        assert!(!manager.is_connected("test_wallet").await);
    }
}
