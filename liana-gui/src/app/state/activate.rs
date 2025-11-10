//! Activate (Breez SDK) state management

use iced::Task;
use std::sync::Arc;

use liana_ui::widget::Element;

use crate::app::{
    cache::Cache,
    message::Message,
    state::State,
    view::{self, activate::{ActivatePanel, ActivateSubPanel}, ActivateMessage, Message as ViewMessage},
};
use crate::daemon::Daemon;

impl State for ActivatePanel {
    fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, ViewMessage> {
        // Map the active panel to the correct menu item for proper highlighting
        match self.active_panel {
            crate::app::view::activate::ActivateSubPanel::Main => 
                view::dashboard(
                    &crate::app::Menu::Activate(crate::app::menu::ActivateMenu::Send),
                    cache,
                    None,
                    crate::app::view::activate::ActivatePanel::view(self, cache),
                ),
            crate::app::view::activate::ActivateSubPanel::Send => 
                view::dashboard(
                    &crate::app::Menu::Activate(crate::app::menu::ActivateMenu::Send),
                    cache,
                    None,
                    crate::app::view::activate::ActivatePanel::view(self, cache),
                ),
            crate::app::view::activate::ActivateSubPanel::Receive => 
                view::dashboard(
                    &crate::app::Menu::Activate(crate::app::menu::ActivateMenu::Receive),
                    cache,
                    None,
                    crate::app::view::activate::ActivatePanel::view(self, cache),
                ),
            crate::app::view::activate::ActivateSubPanel::History => 
                view::dashboard(
                    &crate::app::Menu::Activate(crate::app::menu::ActivateMenu::History),
                    cache,
                    None,
                    crate::app::view::activate::ActivatePanel::view(self, cache),
                ),
            crate::app::view::activate::ActivateSubPanel::Settings => 
                view::dashboard(
                    &crate::app::Menu::Activate(crate::app::menu::ActivateMenu::Send),
                    cache,
                    None,
                    crate::app::view::activate::ActivatePanel::view(self, cache),
                ),
        }
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

        let Message::View(ViewMessage::Activate(message)) = message else {
            return Task::none();
        };

        match message {
            // Navigation messages
            ActivateMessage::ShowMainPanel => {
                self.active_panel = crate::app::view::activate::ActivateSubPanel::Main;
            }
            ActivateMessage::ShowSendPanel => {
                self.active_panel = crate::app::view::activate::ActivateSubPanel::Send;
            }
            ActivateMessage::ShowReceivePanel => {
                self.active_panel = crate::app::view::activate::ActivateSubPanel::Receive;
            }
            ActivateMessage::ShowHistoryPanel => {
                self.active_panel = crate::app::view::activate::ActivateSubPanel::History;
            }
            ActivateMessage::ShowSettingsPanel => {
                self.active_panel = crate::app::view::activate::ActivateSubPanel::Settings;
            }
            
            // Lightning wallet setup
            #[cfg(feature = "breez")]
            ActivateMessage::CreateLightningWallet => {
                match crate::app::breez::generate_lightning_mnemonic() {
                    Ok(mnemonic) => {
                        self.lightning_wallet_state = 
                            crate::app::view::activate::LightningWalletState::ShowingBackup {
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
            ActivateMessage::ConfirmBackup => {
                if let crate::app::view::activate::LightningWalletState::ShowingBackup { ref mnemonic, .. } = 
                    self.lightning_wallet_state
                {
                    let mnemonic_clone = mnemonic.clone();
                    let network_dir = self.data_dir.network_directory(self.network);
                    let wallet_checksum = self.wallet.descriptor_checksum.clone();
                    
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
                        crate::app::view::activate::LightningWalletState::Initializing;
                    
                    let network = self.network;
                    let breez_data_dir = network_dir.path().join(&wallet_checksum).join("lightning");
                    
                    return Task::perform(
                        async move {
                            crate::app::breez::init::initialize_breez_sdk(
                                mnemonic_clone,
                                network,
                                breez_data_dir,
                            )
                            .await
                        },
                        Message::BreezInitialized,
                    );
                }
            }
            #[cfg(feature = "breez")]
            ActivateMessage::ShowImportWallet => {
                self.lightning_wallet_state = 
                    crate::app::view::activate::LightningWalletState::ImportingWallet {
                        mnemonic_input: String::new(),
                        error: None,
                    };
            }
            
            #[cfg(feature = "breez")]
            ActivateMessage::ImportMnemonicEdited(value) => {
                if let crate::app::view::activate::LightningWalletState::ImportingWallet { 
                    ref mut mnemonic_input, 
                    .. 
                } = self.lightning_wallet_state {
                    *mnemonic_input = value;
                }
            }
            
            #[cfg(feature = "breez")]
            ActivateMessage::CancelImport => {
                self.lightning_wallet_state = 
                    crate::app::view::activate::LightningWalletState::NotCreated;
            }
            
            #[cfg(feature = "breez")]
            ActivateMessage::ConfirmImport => {
                // Extract and clone mnemonic to avoid borrow checker issues
                let mnemonic_input = if let crate::app::view::activate::LightningWalletState::ImportingWallet { 
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
                        crate::app::view::activate::LightningWalletState::ImportingWallet {
                            mnemonic_input,
                            error: Some(format!("Invalid mnemonic: expected 24 words, got {}", words.len())),
                        };
                    return Task::none();
                }
                
                // Validate with BIP39
                use bip39::Mnemonic;
                match Mnemonic::parse(&mnemonic_trimmed) {
                        Ok(_) => {
                            let network_dir = self.data_dir.network_directory(self.network);
                            let wallet_checksum = self.wallet.descriptor_checksum.clone();
                            
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
                                crate::app::view::activate::LightningWalletState::Initializing;
                            
                            let mnemonic_for_init = mnemonic_trimmed;
                            let network = self.network;
                            let breez_data_dir = lightning_dir;
                            
                            return Task::perform(
                                async move {
                                    crate::app::breez::init::initialize_breez_sdk(
                                        mnemonic_for_init,
                                        network,
                                        breez_data_dir,
                                    )
                                    .await
                                },
                                Message::BreezInitialized,
                            );
                        }
                        Err(e) => {
                            self.lightning_wallet_state = 
                                crate::app::view::activate::LightningWalletState::ImportingWallet {
                                    mnemonic_input,
                                    error: Some(format!("Invalid BIP39 mnemonic: {}", e)),
                                };
                        }
                    }
            }
            
            #[cfg(feature = "breez")]
            ActivateMessage::LightningWalletCreated(_result) => {
                // Handled by BreezInitialized message
            }
            
            // Send/Receive form messages
            ActivateMessage::DestinationEdited(v) => {
                self.destination = v;
                self.destination_valid = !self.destination.is_empty() 
                    && (self.destination.starts_with("lnbc") 
                        || self.destination.starts_with("lntb")
                        || self.destination.len() > 20);
            }
            ActivateMessage::AmountEdited(v) => {
                self.amount = v;
                self.amount_valid = self.amount.parse::<u64>().is_ok();
            }
            ActivateMessage::DescriptionEdited(v) | ActivateMessage::DescriptionChanged(v) => {
                self.description = v;
            }
            ActivateMessage::ReviewPayment => {
                #[cfg(feature = "breez")]
                return self.prepare_send();
                #[cfg(not(feature = "breez"))]
                Task::none()
            }
            ActivateMessage::SendPayment => {
                #[cfg(feature = "breez")]
                return self.send_payment();
                #[cfg(not(feature = "breez"))]
                Task::none()
            }
            ActivateMessage::CancelPayment => {
                self.prepare_send_response = None;
                self.preparing = false;
                self.sending = false;
            }
            ActivateMessage::GenerateInvoice => {
                #[cfg(feature = "breez")]
                return self.generate_invoice();
                #[cfg(not(feature = "breez"))]
                Task::none()
            }
            ActivateMessage::NewInvoice => {
                self.generated_invoice = None;
                self.amount = String::new();
                self.description = String::new();
            }
            ActivateMessage::RefreshHistory => {
                // TODO: Implement history refresh
            }
            ActivateMessage::PrepareSend => {
                #[cfg(feature = "breez")]
                return self.prepare_send();
                #[cfg(not(feature = "breez"))]
                {
                    self.error = Some("Breez feature not enabled".to_string());
                    Task::none()
                }
            }
            ActivateMessage::ShowConfirmation => {}
            ActivateMessage::ConfirmPayment => {
                #[cfg(feature = "breez")]
                return self.send_payment();
            }
            ActivateMessage::Send => {
                #[cfg(feature = "breez")]
                return self.send_payment();
                #[cfg(not(feature = "breez"))]
                {
                    self.error = Some("Breez feature not enabled".to_string());
                    Task::none()
                }
            }
            #[cfg(feature = "breez")]
            ActivateMessage::PaymentPrepared(response) => {
                self.prepare_send_response = Some(response);
                self.preparing = false;
                self.destination_valid = true;
                self.error = None;
            }
            #[cfg(feature = "breez")]
            ActivateMessage::PrepareFailed(err) => {
                self.preparing = false;
                self.error = Some(format!("Failed to prepare payment: {}", err));
            }
            #[cfg(feature = "breez")]
            ActivateMessage::PaymentSent(msg) => {
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
            ActivateMessage::SendFailed(err) => {
                self.sending = false;
                self.error = Some(format!("Failed to send payment: {}", err));
            }
            #[cfg(feature = "breez")]
            ActivateMessage::LimitsFetched(min_sat, max_sat) => {
                self.payment_limits = Some((min_sat, max_sat));
            }
            ActivateMessage::FilterChanged(_filter) => {}
            ActivateMessage::PaymentsLoaded(_payments) => {}
            ActivateMessage::PrepareReceive => {
                self.error = Some("Prepare receive not yet implemented".to_string());
            }
            #[cfg(feature = "breez")]
            ActivateMessage::InvoiceGenerated(invoice) => {
                self.generated_invoice = Some(invoice);
                self.preparing = false;
                self.error = None;
            }
            #[cfg(feature = "breez")]
            ActivateMessage::InvoiceGenerationFailed(err) => {
                self.preparing = false;
                self.error = Some(format!("Failed to generate invoice: {}", err));
            }
            #[cfg(feature = "breez")]
            ActivateMessage::InvoicePaymentReceived(msg) => {
                self.error = Some(format!("✅ Payment received! {}", msg));
                // Clear form
                self.generated_invoice = None;
                self.amount = String::new();
                self.description = String::new();
                // Refresh balance
                return self.refresh_balance();
            }
            #[cfg(feature = "breez")]
            ActivateMessage::BalanceUpdated(balance) => {
                self.balance = Some(balance);
            }
            ActivateMessage::RefreshBalance => {
                #[cfg(feature = "breez")]
                return self.refresh_balance();
                #[cfg(not(feature = "breez"))]
                {
                    self.error = Some("Breez feature not enabled".to_string());
                    Task::none()
                }
            }
            ActivateMessage::Error(e) => {
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



