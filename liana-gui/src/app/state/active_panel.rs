//! Active (Breez SDK) state management

use iced::Task;
use std::sync::Arc;

use liana_ui::widget::Element;

use crate::app::{
    cache::Cache,
    message::Message,
    state::State,
    view::{active::ActivePanel, ActiveMessage, Message as ViewMessage},
};
use crate::daemon::Daemon;

impl State for ActivePanel {
    fn view<'a>(&'a self, _menu: &'a crate::app::menu::Menu, cache: &'a Cache) -> Element<'a, ViewMessage> {
        // For active panel, we show simple content without the full dashboard wrapper
        // since we handle our own header and navigation
        crate::app::view::active::ActivePanel::view(self, cache)
    }

    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _cache: &Cache,
        message: Message,
    ) -> Task<Message> {
        // Handle global navigation
        match &message {
            Message::View(ViewMessage::Close) => {
                self.error = None;
                return Task::none();
            }
            _ => (),
        }

        let Message::View(ViewMessage::Active(message)) = message else {
            return Task::none();
        };

        match message {
            // Navigation messages
            ActiveMessage::ShowMainPanel => {
                self.active_panel = crate::app::view::active::ActiveSubPanel::Main;
            }
            ActiveMessage::ShowSendPanel => {
                self.active_panel = crate::app::view::active::ActiveSubPanel::Send;
            }
            ActiveMessage::ShowReceivePanel => {
                self.active_panel = crate::app::view::active::ActiveSubPanel::Receive;
            }
            ActiveMessage::ShowHistoryPanel => {
                self.active_panel = crate::app::view::active::ActiveSubPanel::History;
                // Auto-load payment history when entering history panel
                #[cfg(feature = "breez")]
                if self.payments.is_empty() && !self.loading_payments {
                    return self.load_payment_history();
                }
            }
            ActiveMessage::ShowSettingsPanel => {
                self.active_panel = crate::app::view::active::ActiveSubPanel::Settings;
            }
            
            // Lightning wallet setup
            #[cfg(feature = "breez")]
            ActiveMessage::CreateLightningWallet => {
                match crate::app::breez::generate_lightning_mnemonic() {
                    Ok(mnemonic) => {
                        self.lightning_wallet_state = 
                            crate::app::view::active::LightningWalletState::ShowingBackup {
                                mnemonic,
                                confirmed: false,
                            };
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to generate mnemonic: {}", e));
                    }
                }
            }
            #[cfg(feature = "breez")]
            ActiveMessage::ConfirmBackup => {
                if let crate::app::view::active::LightningWalletState::ShowingBackup { ref mnemonic, .. } = 
                    self.lightning_wallet_state
                {
                    let Some(wallet) = self.wallet.as_ref() else {
                        self.error = Some("No wallet available".to_string());
                        return Task::none();
                    };
                    
                    let mnemonic_clone = mnemonic.clone();
                    let network_dir = self.data_dir.network_directory(self.network);
                    let wallet_checksum = wallet.descriptor_checksum.clone();
                    
                    // Store Lightning wallet for this specific Bitcoin wallet
                    if let Err(e) = crate::app::breez::store_lightning_mnemonic(
                        network_dir.path(), 
                        &wallet_checksum,
                        &mnemonic_clone
                    ) {
                        self.error = Some(format!("Failed to store wallet: {}", e));
                        return Task::none();
                    }
                    
                    self.lightning_wallet_state = 
                        crate::app::view::active::LightningWalletState::Initializing;
                    
                    let network = self.network;
                    let breez_data_dir = network_dir.path().join(&wallet_checksum).join("lightning");
                    
                    return Task::perform(
                        async move {
                            // Initialize wallet manager
                            let manager = crate::app::breez::BreezWalletManager::initialize(
                                &mnemonic_clone,
                                network,
                                &breez_data_dir,
                            )
                            .await?;
                            
                            // Setup event listener
                            let sdk = manager.sdk()?;
                            let event_receiver = crate::app::breez::setup_event_listener(sdk).await?;
                            
                            Ok((std::sync::Arc::new(manager), event_receiver))
                        },
                        Message::BreezInitialized,
                    );
                }
            }
            #[cfg(feature = "breez")]
            ActiveMessage::ShowImportWallet => {
                self.lightning_wallet_state = 
                    crate::app::view::active::LightningWalletState::ImportingWallet {
                        mnemonic_input: String::new(),
                        error: None,
                    };
            }
            
            #[cfg(feature = "breez")]
            ActiveMessage::ImportMnemonicEdited(value) => {
                if let crate::app::view::active::LightningWalletState::ImportingWallet { 
                    ref mut mnemonic_input, 
                    .. 
                } = self.lightning_wallet_state {
                    *mnemonic_input = value;
                }
            }
            
            #[cfg(feature = "breez")]
            ActiveMessage::CancelImport => {
                self.lightning_wallet_state = 
                    crate::app::view::active::LightningWalletState::NotCreated;
            }
            
            #[cfg(feature = "breez")]
            ActiveMessage::ConfirmImport => {
                // Extract and clone mnemonic to avoid borrow checker issues
                let mnemonic_input = if let crate::app::view::active::LightningWalletState::ImportingWallet { 
                    ref mnemonic_input, 
                    .. 
                } = self.lightning_wallet_state {
                    mnemonic_input.clone()
                } else {
                    return Task::none();
                };
                
                // Validate mnemonic format
                let mnemonic_trimmed = mnemonic_input.trim().to_string();
                let words: Vec<&str> = mnemonic_trimmed.split_whitespace().collect();
                
                if words.len() != 24 {
                    self.lightning_wallet_state = 
                        crate::app::view::active::LightningWalletState::ImportingWallet {
                            mnemonic_input,
                            error: Some(format!("Invalid mnemonic: expected 24 words, got {}", words.len())),
                        };
                    return Task::none();
                }
                
                // Validate with BIP39
                use bip39::Mnemonic;
                match Mnemonic::parse(&mnemonic_trimmed) {
                        Ok(_) => {
                            let Some(wallet) = self.wallet.as_ref() else {
                                self.error = Some("No wallet available".to_string());
                                return Task::none();
                            };
                            
                            let network_dir = self.data_dir.network_directory(self.network);
                            let wallet_checksum = wallet.descriptor_checksum.clone();
                            
                            // Delete existing corrupted wallet if any
                            let lightning_dir = network_dir.path().join(&wallet_checksum).join("lightning");
                            if lightning_dir.exists() {
                                log::warn!("🗑️ Deleting existing Lightning wallet files for import...");
                                if let Err(e) = std::fs::remove_dir_all(&lightning_dir) {
                                    self.error = Some(format!("Failed to delete old wallet: {}", e));
                                    return Task::none();
                                }
                            }
                            
                            // Store imported mnemonic
                            if let Err(e) = crate::app::breez::store_lightning_mnemonic(
                                network_dir.path(), 
                                &wallet_checksum,
                                &mnemonic_trimmed
                            ) {
                                self.error = Some(format!("Failed to store imported wallet: {}", e));
                                return Task::none();
                            }
                            
                            log::info!("✅ Lightning wallet imported successfully");
                            
                            // Initialize SDK with imported mnemonic
                            self.lightning_wallet_state = 
                                crate::app::view::active::LightningWalletState::Initializing;
                            
                            let mnemonic_for_init = mnemonic_trimmed;
                            let network = self.network;
                            let breez_data_dir = lightning_dir;
                            
                            return Task::perform(
                                async move {
                                    // Initialize wallet manager
                                    let manager = crate::app::breez::BreezWalletManager::initialize(
                                        &mnemonic_for_init,
                                        network,
                                        &breez_data_dir,
                                    )
                                    .await?;
                                    
                                    // Setup event listener
                                    let sdk = manager.sdk()?;
                                    let event_receiver = crate::app::breez::setup_event_listener(sdk).await?;
                                    
                                    Ok((std::sync::Arc::new(manager), event_receiver))
                                },
                                Message::BreezInitialized,
                            );
                        }
                        Err(e) => {
                            self.lightning_wallet_state = 
                                crate::app::view::active::LightningWalletState::ImportingWallet {
                                    mnemonic_input,
                                    error: Some(format!("Invalid BIP39 mnemonic: {}", e)),
                                };
                        }
                    }
            }
            
            #[cfg(feature = "breez")]
            ActiveMessage::LightningWalletCreated(_result) => {
                // Handled by BreezInitialized message
            }
            
            // Send/Receive form messages
            ActiveMessage::DestinationEdited(v) => {
                self.destination = v;
                self.destination_valid = !self.destination.is_empty() 
                    && (self.destination.starts_with("lnbc") 
                        || self.destination.starts_with("lntb")
                        || self.destination.len() > 20);
            }
            ActiveMessage::AmountEdited(v) => {
                self.amount = v;
                self.amount_valid = self.amount.parse::<u64>().is_ok();
            }
            ActiveMessage::DescriptionEdited(v) | ActiveMessage::DescriptionChanged(v) => {
                self.description = v;
            }
            ActiveMessage::ReviewPayment => {
                #[cfg(feature = "breez")]
                return self.prepare_send();
                #[cfg(not(feature = "breez"))]
                Task::none()
            }
            ActiveMessage::SendPayment => {
                #[cfg(feature = "breez")]
                return self.send_payment();
                #[cfg(not(feature = "breez"))]
                Task::none()
            }
            ActiveMessage::CancelPayment => {
                self.prepare_send_response = None;
                self.preparing = false;
                self.sending = false;
            }
            ActiveMessage::GenerateInvoice => {
                #[cfg(feature = "breez")]
                return self.generate_invoice();
                #[cfg(not(feature = "breez"))]
                Task::none()
            }
            ActiveMessage::NewInvoice => {
                self.generated_invoice = None;
                self.amount = String::new();
                self.description = String::new();
            }
            ActiveMessage::RefreshHistory => {
                // TODO: Implement history refresh
            }
            ActiveMessage::PrepareSend => {
                #[cfg(feature = "breez")]
                return self.prepare_send();
                #[cfg(not(feature = "breez"))]
                {
                    self.error = Some("Breez feature not enabled".to_string());
                    Task::none()
                }
            }
            ActiveMessage::ShowConfirmation => {}
            ActiveMessage::ConfirmPayment => {
                #[cfg(feature = "breez")]
                return self.send_payment();
            }
            ActiveMessage::Send => {
                #[cfg(feature = "breez")]
                return self.send_payment();
                #[cfg(not(feature = "breez"))]
                {
                    self.error = Some("Breez feature not enabled".to_string());
                    Task::none()
                }
            }
            #[cfg(feature = "breez")]
            ActiveMessage::PaymentPrepared(response) => {
                self.prepare_send_response = Some(response);
                self.preparing = false;
                self.destination_valid = true;
                self.error = None;
            }
            #[cfg(feature = "breez")]
            ActiveMessage::PrepareFailed(err) => {
                self.preparing = false;
                self.error = Some(format!("Failed to prepare payment: {}", err));
            }
            #[cfg(feature = "breez")]
            ActiveMessage::PaymentSent(msg) => {
                self.sending = false;
                self.error = None;
                // Clear form
                self.destination = String::new();
                self.amount = String::new();
                self.prepare_send_response = None;
                self.destination_valid = false;
                self.amount_valid = false;
                // Show success message
                self.error = Some(msg);
                // Refresh balance
                return self.refresh_balance();
            }
            #[cfg(feature = "breez")]
            ActiveMessage::SendFailed(err) => {
                self.sending = false;
                self.error = Some(format!("Failed to send payment: {}", err));
            }
            #[cfg(feature = "breez")]
            ActiveMessage::LimitsFetched(min_sat, max_sat) => {
                self.payment_limits = Some((min_sat, max_sat));
            }
            ActiveMessage::FilterChanged(_filter) => {
                // TODO: Implement payment filtering
            }
            ActiveMessage::LoadPaymentHistory => {
                #[cfg(feature = "breez")]
                return self.load_payment_history();
                #[cfg(not(feature = "breez"))]
                {
                    self.payment_error = Some("Breez feature not enabled".to_string());
                }
            }
            #[cfg(feature = "breez")]
            ActiveMessage::PaymentHistoryLoaded(payments) => {
                self.payments = payments;
                self.loading_payments = false;
                self.payment_error = None;
            }
            #[cfg(feature = "breez")]
            ActiveMessage::PaymentHistoryLoadFailed(error) => {
                self.loading_payments = false;
                self.payment_error = Some(error);
            }
            ActiveMessage::RefreshHistory => {
                #[cfg(feature = "breez")]
                return self.load_payment_history();
            }
            ActiveMessage::PrepareReceive => {
                self.error = Some("Prepare receive not yet implemented".to_string());
            }
            #[cfg(feature = "breez")]
            ActiveMessage::InvoiceGenerated(invoice) => {
                self.generated_invoice = Some(invoice);
                self.preparing = false;
                self.error = None;
            }
            #[cfg(feature = "breez")]
            ActiveMessage::InvoiceGenerationFailed(err) => {
                self.preparing = false;
                self.error = Some(format!("Failed to generate invoice: {}", err));
            }
            #[cfg(feature = "breez")]
            ActiveMessage::InvoicePaymentReceived(msg) => {
                self.error = Some(format!("✅ Payment received! {}", msg));
                // Clear form
                self.generated_invoice = None;
                self.amount = String::new();
                self.description = String::new();
                // Refresh balance
                return self.refresh_balance();
            }
            #[cfg(feature = "breez")]
            ActiveMessage::BalanceUpdated(balance) => {
                self.balance = Some(balance);
            }
            ActiveMessage::RefreshBalance => {
                #[cfg(feature = "breez")]
                return self.refresh_balance();
                #[cfg(not(feature = "breez"))]
                {
                    self.error = Some("Breez feature not enabled".to_string());
                    Task::none()
                }
            }
            ActiveMessage::Error(e) => {
                self.error = Some(e);
            }
        }

        Task::none()
    }

    fn reload(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _wallet: Arc<crate::app::wallet::Wallet>,
    ) -> Task<Message> {
        #[cfg(feature = "breez")]
        return self.refresh_balance();
        #[cfg(not(feature = "breez"))]
        Task::none()
    }
}



