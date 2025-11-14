//! Active panel - Main UI for Breez SDK integration

use std::path::PathBuf;
use iced::{widget::{container, scrollable, Space, TextInput}, Length};
use liana::miniscript::bitcoin::{self, Network};
use liana_ui::{
    color,
    component::{button as ui_button, text as ui_text},
    theme,
    widget::*,
};

use super::ActiveSubPanel;
use crate::app::{
    cache::Cache,
    view::{ActiveMessage, Message as ViewMessage},
};
use liana_ui::component::text::Text as TextTrait;

#[cfg(feature = "breez")]
use crate::app::breez::{BalanceInfo, BreezWalletManager, PaymentInfo};

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::{
    PaymentMethod, ReceivePaymentResponse,
    PrepareSendResponse,
};

pub struct ActivePanel {
    pub active_panel: ActiveSubPanel,
    pub error: Option<String>,
    pub network: Network,

    // Lightning wallet setup state
    #[cfg(feature = "breez")]
    pub lightning_wallet_state: LightningWalletState,

    // Breez SDK manager
    #[cfg(feature = "breez")]
    pub breez_manager: Option<BreezWalletManager>,
    #[cfg(feature = "breez")]
    pub balance: Option<BalanceInfo>,

    // Send state
    pub destination: String,
    pub amount: String,
    pub destination_valid: bool,
    pub amount_valid: bool,
    #[cfg(feature = "breez")]
    pub prepare_send_response: Option<PrepareSendResponse>,
    #[cfg(feature = "breez")]
    pub payment_limits: Option<(u64, u64)>, // (min_sat, max_sat)
    pub preparing: bool,
    pub sending: bool,

    // Receive state
    pub description: String,
    #[cfg(feature = "breez")]
    pub generated_invoice: Option<String>,
    #[cfg(feature = "breez")]
    pub lightning_address: Option<String>,

    // Payment history state
    #[cfg(feature = "breez")]
    pub payments: Vec<PaymentInfo>,
    #[cfg(feature = "breez")]
    pub loading_payments: bool,
    #[cfg(feature = "breez")]
    pub payment_error: Option<String>,

    // Wallet data
    pub wallet: Option<std::sync::Arc<crate::app::wallet::Wallet>>,
    pub data_dir: crate::dir::LianaDirectory,
}

#[cfg(feature = "breez")]
#[derive(Debug, Clone)]
pub enum LightningWalletState {
    /// No Lightning wallet exists yet
    NotCreated,
    /// Showing mnemonic for backup
    ShowingBackup { mnemonic: String, confirmed: bool },
    /// Showing import wallet screen
    ImportingWallet { mnemonic_input: String, error: Option<String> },
    /// Initializing Breez SDK (connecting to network)
    Initializing,
    /// SDK connected and wallet ready to use
    Ready,
}

// LightningWalletState is now used directly in parent modules

impl ActivePanel {
    pub fn new(
        network: Network,
        wallet: Option<std::sync::Arc<crate::app::wallet::Wallet>>,
        data_dir: crate::dir::LianaDirectory,
    ) -> Self {
        // Check if Lightning wallet already exists for this specific Bitcoin wallet
        // NOTE: We start in Initializing state if wallet exists - actual SDK connection
        // happens separately and will update state to Ready when complete
        #[cfg(feature = "breez")]
        let lightning_wallet_state = {
            if let Some(ref w) = wallet {
                let network_dir = data_dir.network_directory(network);
                let wallet_checksum = &w.descriptor_checksum;
                let wallet_exists = crate::app::breez::storage::lightning_wallet_exists(network_dir.path(), wallet_checksum);
                
                // Wallet initialization check
                log::debug!("Lightning wallet check for wallet {}: exists={}", wallet_checksum, wallet_exists);
                
                if wallet_exists {
                    // Wallet file exists - SDK will be initialized automatically
                    // State will change to Ready after successful connection
                    LightningWalletState::Initializing
                } else {
                    LightningWalletState::NotCreated
                }
            } else {
                LightningWalletState::NotCreated
            }
        };

        let panel = Self {
            active_panel: ActiveSubPanel::Main,
            error: None,
            network,
            #[cfg(feature = "breez")]
            lightning_wallet_state,
            #[cfg(feature = "breez")]
            breez_manager: None,
            #[cfg(feature = "breez")]
            balance: None,
            destination: String::new(),
            amount: String::new(),
            destination_valid: false,
            amount_valid: false,
            #[cfg(feature = "breez")]
            prepare_send_response: None,
            #[cfg(feature = "breez")]
            payment_limits: None,
            preparing: false,
            sending: false,
            description: String::new(),
            #[cfg(feature = "breez")]
            generated_invoice: None,
            #[cfg(feature = "breez")]
            lightning_address: None,
            #[cfg(feature = "breez")]
            payments: Vec::new(),
            #[cfg(feature = "breez")]
            loading_payments: false,
            #[cfg(feature = "breez")]
            payment_error: None,
            wallet,
            data_dir,
        };
        
        panel
    }
    
    /// Create empty panel without wallet
    pub fn new_empty(
        network: Network,
        data_dir: crate::dir::LianaDirectory,
    ) -> Self {
        #[cfg(feature = "breez")]
        let lightning_wallet_state = LightningWalletState::NotCreated;
        
        Self {
            active_panel: ActiveSubPanel::Main,
            error: None,
            network,
            #[cfg(feature = "breez")]
            lightning_wallet_state,
            #[cfg(feature = "breez")]
            breez_manager: None,
            #[cfg(feature = "breez")]
            balance: None,
            destination: String::new(),
            amount: String::new(),
            destination_valid: false,
            amount_valid: false,
            #[cfg(feature = "breez")]
            prepare_send_response: None,
            #[cfg(feature = "breez")]
            payment_limits: None,
            preparing: false,
            sending: false,
            description: String::new(),
            #[cfg(feature = "breez")]
            generated_invoice: None,
            #[cfg(feature = "breez")]
            lightning_address: None,
            #[cfg(feature = "breez")]
            payments: Vec::new(),
            #[cfg(feature = "breez")]
            loading_payments: false,
            #[cfg(feature = "breez")]
            payment_error: None,
            wallet: None,
            data_dir,
        }
    }
    
    /// Check if SDK initialization should be triggered
    /// This checks panel-level state only. The app-level check should be done separately.
    #[cfg(feature = "breez")]
    pub fn should_initialize_sdk(&self) -> bool {
        // Initialize if:
        // 1. State is Initializing (wallet file exists, needs connection)
        // 2. Panel doesn't have breez_manager yet
        let should_init = matches!(self.lightning_wallet_state, LightningWalletState::Initializing)
            && self.breez_manager.is_none();
        
        if should_init {
            log::debug!("Panel needs SDK initialization (state: Initializing, manager: None)");
        }
        
        should_init
    }
    
    /// Get initialization parameters if SDK should be initialized
    #[cfg(feature = "breez")]
    pub fn get_init_params(&self) -> Result<Option<(String, bitcoin::Network, PathBuf)>, String> {
        if !self.should_initialize_sdk() {
            return Ok(None);
        }
        
        let wallet = self.wallet.as_ref().ok_or_else(|| "No wallet available".to_string())?;
        let network_dir = self.data_dir.network_directory(self.network);
        let wallet_checksum = &wallet.descriptor_checksum;
        
        // Load mnemonic from storage
        match crate::app::breez::storage::load_lightning_mnemonic(network_dir.path(), wallet_checksum) {
            Ok(mnemonic) => {
                let breez_data_dir = network_dir.path().join(wallet_checksum).join("lightning");
                Ok(Some((mnemonic, self.network, breez_data_dir)))
            }
            Err(e) => {
                log::error!("❌ Failed to load Lightning mnemonic: {}", e);
                log::error!("This usually means:");
                log::error!("1. The mnemonic file is corrupted");
                log::error!("2. The Breez SDK database is corrupted");
                log::error!("3. File permissions issue");
                
                // Return error message to trigger recovery flow
                Err(format!("Lightning wallet file corrupted or unreadable: {}. The wallet needs to be reset.", e))
            }
        }
    }
    
    /// Check if Lightning wallet file exists but is corrupted/unreadable
    #[cfg(feature = "breez")]
    pub fn is_wallet_corrupted(&self) -> bool {
        let Some(wallet) = self.wallet.as_ref() else { return false; };
        let network_dir = self.data_dir.network_directory(self.network);
        let wallet_checksum = &wallet.descriptor_checksum;
        
        // File exists but can't be loaded = corrupted
        crate::app::breez::storage::lightning_wallet_exists(network_dir.path(), wallet_checksum)
            && crate::app::breez::storage::load_lightning_mnemonic(network_dir.path(), wallet_checksum).is_err()
    }

    pub fn view<'a>(&'a self, _cache: &'a Cache) -> Element<'a, ViewMessage> {
        // Create header with logo, network, and wallet address (similar to buysell)
        let header = self.view_header();
        
        // Get panel content
        let content = match self.active_panel {
            ActiveSubPanel::Main => self.view_main(),
            ActiveSubPanel::Send => self.view_send(),
            ActiveSubPanel::Receive => self.view_receive(),
            ActiveSubPanel::History => self.view_history(),
            ActiveSubPanel::Settings => self.view_settings(),
        };
        
        // Combine header + content (matching buysell structure)
        Column::new()
            .spacing(20)
            .push(header)
            .push(content)
            .into()
    }
    
    fn view_header<'a>(&'a self) -> Element<'a, ViewMessage> {
        // Derive Bitcoin address from wallet descriptor at index 0
        let address_text = if let Some(wallet) = &self.wallet {
            let secp = liana::miniscript::bitcoin::secp256k1::Secp256k1::verification_only();
            let receive_desc = wallet.main_descriptor.receive_descriptor();
            receive_desc
                .derive(0.into(), &secp)
                .address(self.network)
                .to_string()
        } else {
            "No wallet".to_string()
        };
        
        // Create header matching buysell panel structure
        Column::new()
            .push(Space::with_height(150))
            // COINCUBE branding - centered
            .push(
                Container::new(
                    Row::new()
                        .push(
                            Row::new()
                                .push(ui_text::h4_bold("COIN").color(color::ORANGE))
                                .push(ui_text::h4_bold("CUBE").color(color::WHITE))
                                .spacing(0),
                        )
                        .push(Space::with_width(Length::Fixed(8.0)))
                        .push(
                            ui_text::text("⚡")
                                .size(24)
                                .style(|_| iced::widget::text::Style { color: Some(color::ORANGE) })
                        )
                        .align_y(iced::Alignment::Center)
                )
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
            )
            .push(Space::with_height(20))
            // Network indicator
            .push(
                ui_text::p2_regular(match self.network {
                    Network::Bitcoin => "Bitcoin Lightning Network",
                    Network::Testnet => "Testnet Lightning Network",
                    Network::Signet => "Signet Lightning Network",
                    Network::Regtest => "Regtest Lightning Network",
                    _ => "Lightning Network",
                })
                .color(color::GREY_3)
            )
            .push(Space::with_height(10))
            // Bitcoin Wallet Address
            .push(
                Container::new(
                    Column::new()
                        .spacing(5)
                        .push(
                            ui_text::p2_regular("Bitcoin Wallet Address")
                                .color(color::GREY_3)
                        )
                        .push(
                            ui_text::p1_regular(&address_text)
                                .color(color::GREY_3)
                        )
                        .align_x(iced::Alignment::Center)
                )
                .width(Length::Fill)
                .padding(15)
                .style(theme::card::simple)
            )

            .push(Space::with_height(10))
            // Lightning address (if available)
            .push_maybe({
                #[cfg(feature = "breez")]
                {
                    self.lightning_address.as_ref().map(|ln_addr| {
                        Container::new(
                            Column::new()
                                .spacing(5)
                                .push(
                                    ui_text::p2_regular("Lightning Address")
                                        .color(color::ORANGE)
                                )
                                .push(
                                    ui_text::p1_regular(ln_addr)
                                        .color(color::ORANGE)
                                )
                                .align_x(iced::Alignment::Center)
                        )
                        .width(Length::Fill)
                        .padding(15)
                        .style(theme::card::simple)
                    })
                }
                
                #[cfg(not(feature = "breez"))]
                {
                    None::<Container<'a, ViewMessage>>
                }
            })
            .push(Space::with_height(40))
            .align_x(iced::Alignment::Center)
            .into()
    }

    fn view_main<'a>(&'a self) -> Element<'a, ViewMessage> {
        let mut col = Column::new()
            .spacing(20)
            .padding(20);

        // Title
        col = col.push(
            ui_text::h1("Activate Lightning")
                .width(Length::Fill),
        );

        // Error display
        if let Some(ref error) = self.error {
            col = col.push(
                container(
                    ui_text::text(error)
                        .style(|_| iced::widget::text::Style { color: Some(color::RED) })
                )
                .padding(10)
                .style(|theme| theme::card::invalid(theme))
            );
        }

        // Check Lightning wallet state
        #[cfg(feature = "breez")]
        {
            match &self.lightning_wallet_state {
                LightningWalletState::NotCreated => {
                    return self.view_create_wallet();
                }
                LightningWalletState::ShowingBackup { mnemonic, confirmed } => {
                    return self.view_backup_mnemonic(mnemonic, *confirmed);
                }
                LightningWalletState::ImportingWallet { mnemonic_input, error } => {
                    return self.view_import_wallet(mnemonic_input, error.as_deref());
                }
                LightningWalletState::Initializing => {
                col = col.push(
                    container(
                        Column::new()
                            .spacing(10)
                            .push(
                                ui_text::text("⚡ Initializing Lightning Wallet...")
                                    .width(Length::Fill),
                            )
                            .push(
                                TextTrait::small(ui_text::text("This may take a moment"))
                                    .width(Length::Fill)
                                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) }),
                            ),
                    )
                    .padding(20)
                    .style(|theme| theme::card::simple(theme))
                );
                return col.into();
                }
                LightningWalletState::Ready => {
                    // SDK is connected, continue to show main interface
                }
            }
        }

        // Connection and Balance display
        #[cfg(feature = "breez")]
        if let Some(ref _manager) = self.breez_manager {
            // Connected - show balance
            if let Some(ref balance) = self.balance {
                col = col.push(self.view_balance(balance));
            } else {
                col = col.push(
                    container(
                        ui_text::text("Loading balance...")
                            .width(Length::Fill),
                    )
                    .padding(10)
                    .style(|theme| theme::card::simple(theme))
                );
            }
        } else {
            // Not connected yet
            col = col.push(
                container(
                    Column::new()
                        .spacing(10)
                        .push(
                            ui_text::text("⚡ Initializing Lightning Network...")
                                .width(Length::Fill),
                        )
                        .push(
                            TextTrait::small(ui_text::text("This may take a moment on first run"))
                                .width(Length::Fill)
                                .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) }),
                        ),
                )
                .padding(20)
                .style(|theme| theme::card::simple(theme))
            );
        }

        #[cfg(not(feature = "breez"))]
        {
            col = col.push(
                ui_text::text("Breez feature not enabled. Rebuild with --features breez")
                    .width(Length::Fill)
                    .horizontal_alignment(iced::alignment::Horizontal::Center)
                    .style(|_| iced::widget::text::Style { color: Some(color::RED) })
            );
        }

        // Info text: Send/Receive/History buttons are in the sidebar menu
        col = col.push(
            container(
                Column::new()
                    .spacing(10)
                    .push(
                        ui_text::text("💡 Quick Tip")
                            .size(16)
                            .style(|_| iced::widget::text::Style { color: Some(color::ORANGE) })
                    )
                    .push(
                        ui_text::text("Use the menu on the left to:")
                            .size(14)
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
                    .push(
                        ui_text::text("• Send - Send Lightning payments")
                            .size(14)
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
                    .push(
                        ui_text::text("• Receive - Generate invoices")
                            .size(14)
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
                    .push(
                        ui_text::text("• History - View transactions")
                            .size(14)
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
            )
            .padding(20)
            .style(|theme| theme::card::simple(theme))
        );

        col.into()
    }

    #[cfg(feature = "breez")]
    fn view_create_wallet<'a>(&'a self) -> Element<'a, ViewMessage> {
        // Check if wallet is corrupted (file exists but unreadable)
        let is_corrupted = self.is_wallet_corrupted();
        
        let title = if is_corrupted {
            "Reset Lightning Wallet"
        } else {
            "Create Lightning Wallet"
        };
        
        let description = if is_corrupted {
            "⚠️ Your Lightning wallet file is corrupted or the database is out of sync. \
             You need to reset it and create a new wallet. Your Bitcoin wallet is safe and unaffected."
        } else {
            "Your Lightning wallet is separate from your Bitcoin wallet for enhanced security."
        };
        
        Container::new(
            Column::new()
                .spacing(30)
                .padding(40)
                .max_width(600)
                .push(
                    Column::new()
                        .spacing(15)
                        .push(
                            ui_text::h2(title)
                        )
                        .push({
                            let text_color = if is_corrupted { color::ORANGE } else { color::GREY_3 };
                            ui_text::text(description)
                                .style(move |_| iced::widget::text::Style { 
                                    color: Some(text_color)
                                })
                        })
                )
                .push_maybe(if is_corrupted {
                    Some(
                        container(
                            Column::new()
                                .spacing(10)
                                .push(
                                    ui_text::text("⚠️ Important Information:")
                                        .size(16)
                                        .style(|_| iced::widget::text::Style { color: Some(color::RED) })
                                )
                                .push(
                                    ui_text::text("• This will delete the corrupted Lightning wallet database")
                                        .size(14)
                                        .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                                )
                                .push(
                                    ui_text::text("• Your Bitcoin balance is safe (separate wallet)")
                                        .size(14)
                                        .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                                )
                                .push(
                                    ui_text::text("• If you have funds in Lightning, backup your old mnemonic first!")
                                        .size(14)
                                        .style(|_| iced::widget::text::Style { color: Some(color::ORANGE) })
                                )
                        )
                        .padding(20)
                        .style(|theme| {
                            let mut style = theme::card::simple(theme);
                            style.border.color = color::ORANGE;
                            style.border.width = 2.0;
                            style
                        })
                    )
                } else {
                    None
                })
                .push(
                    container(
                        Column::new()
                            .spacing(15)
                            .push(ui_text::p1_bold("Why a separate wallet?"))
                            .push(ui_text::text("✓ Lightning is a hot wallet (always online)"))
                            .push(ui_text::text("✓ Your Bitcoin wallet stays secure"))
                            .push(ui_text::text("✓ Industry best practice for security"))
                            .push(ui_text::text("✓ You'll back up both seeds separately"))
                    )
                    .padding(20)
                    .style(theme::card::simple)
                )
                .push(
                    container(
                        Column::new()
                            .spacing(10)
                            .push(
                                Row::new()
                                    .spacing(10)
                                    .push(ui_text::text("⚠️").size(20))
                                    .push(ui_text::p1_bold("Important"))
                            )
                            .push(ui_text::text("You will receive a NEW recovery phrase (24 words) for your Lightning wallet. Keep it safe and separate from your Bitcoin recovery phrase."))
                    )
                    .padding(20)
                    .style(|theme| {
                        let mut style = theme::card::simple(theme);
                        style.border.color = color::ORANGE;
                        style.border.width = 2.0;
                        style
                    })
                )
                .push(
                    if is_corrupted {
                        Row::new()
                            .spacing(10)
                            .push(
                                ui_button::primary(None, "Reset & Create New")
                                    .on_press(ViewMessage::Active(ActiveMessage::CreateLightningWallet))
                                    .width(Length::Fill)
                            )
                            .push(
                                ui_button::secondary(None, "Import Existing Wallet")
                                    .on_press(ViewMessage::Active(ActiveMessage::ShowImportWallet))
                                    .width(Length::Fill)
                            )
                    } else {
                        Row::new()
                            .spacing(10)
                            .push(
                                ui_button::primary(None, "Create New Wallet")
                                    .on_press(ViewMessage::Active(ActiveMessage::CreateLightningWallet))
                                    .width(Length::Fill)
                            )
                            .push(
                                ui_button::secondary(None, "Import Existing Wallet")
                                    .on_press(ViewMessage::Active(ActiveMessage::ShowImportWallet))
                                    .width(Length::Fill)
                            )
                    }
                )
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
    }
    
    #[cfg(feature = "breez")]
    fn view_import_wallet<'a>(&'a self, mnemonic_input: &'a str, error_msg: Option<&'a str>) -> Element<'a, ViewMessage> {
        let mut col = Column::new()
            .spacing(30)
            .padding(40)
            .max_width(700)
            .push(
                Column::new()
                    .spacing(15)
                    .push(ui_text::h2("Import Lightning Wallet"))
                    .push(
                        ui_text::text("Enter your 24-word recovery phrase to restore your Lightning wallet.")
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
            );
        
        if let Some(err) = error_msg {
            col = col.push(
                container(
                    ui_text::text(err)
                        .style(|_| iced::widget::text::Style { color: Some(color::RED) })
                )
                .padding(15)
                .style(|theme| {
                    let mut style = theme::card::simple(theme);
                    style.border.color = color::RED;
                    style.border.width = 2.0;
                    style
                })
            );
        }
        
        Container::new(
            col
                .push(
                    Column::new()
                        .spacing(10)
                        .push(
                            ui_text::text("Recovery Phrase (24 words separated by spaces)")
                                .size(14)
                                .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                        )
                        .push(
                            TextInput::new(
                                "word1 word2 word3 ... word24",
                                mnemonic_input
                            )
                            .on_input(|value| ViewMessage::Active(ActiveMessage::ImportMnemonicEdited(value)))
                            .padding(15)
                            .size(14)
                        )
                )
                .push(
                    container(
                        Column::new()
                            .spacing(10)
                            .push(
                                ui_text::text("⚠️ Important:")
                                    .size(14)
                                    .style(|_| iced::widget::text::Style { color: Some(color::ORANGE) })
                            )
                            .push(
                                ui_text::text("• Make sure you have the correct 24-word phrase for THIS Lightning wallet")
                                    .size(12)
                                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                            )
                            .push(
                                ui_text::text("• This is different from your Bitcoin wallet recovery phrase")
                                    .size(12)
                                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                            )
                            .push(
                                ui_text::text("• Using the wrong phrase will create a different wallet with zero balance")
                                    .size(12)
                                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                            )
                    )
                    .padding(15)
                    .style(|theme| theme::card::simple(theme))
                )
                .push(
                    Row::new()
                        .spacing(10)
                        .push(
                            ui_button::secondary(None, "Cancel")
                                .on_press(ViewMessage::Active(ActiveMessage::CancelImport))
                                .width(Length::Fill)
                        )
                        .push(
                            ui_button::primary(None, "Import Wallet")
                                .on_press(ViewMessage::Active(ActiveMessage::ConfirmImport))
                                .width(Length::Fill)
                        )
                )
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
    }

    #[cfg(feature = "breez")]
    fn view_backup_mnemonic<'a>(&'a self, mnemonic: &'a str, _confirmed: bool) -> Element<'a, ViewMessage> {
        let words: Vec<&str> = mnemonic.split_whitespace().collect();
        
        Container::new(
            Column::new()
                .spacing(30)
                .padding(40)
                .max_width(700)
                .push(
                    Column::new()
                        .spacing(15)
                        .push(
                            ui_text::h2("Back Up Your Lightning Wallet")
                        )
                        .push(
                            ui_text::text("Write down these 24 words in order. You'll need them to recover your Lightning wallet.")
                                .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                        )
                )
                .push(
                    container(
                        words.chunks(6).fold(Column::new().spacing(10), |col, chunk| {
                            let row = chunk.iter().fold(
                                Row::new().spacing(10),
                                |row, word| {
                                    let word_num = words.iter().position(|w| w == word).unwrap() + 1;
                                    row.push(
                                        container(
                                            Row::new()
                                                .spacing(5)
                                                .push(
                                                    ui_text::text(&format!("{}.", word_num))
                                                        .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                                                )
                                                .push(ui_text::p1_bold(word))
                                        )
                                        .padding(10)
                                        .width(Length::Fill)
                                    )
                                }
                            );
                            col.push(row)
                        })
                    )
                    .padding(20)
                    .style(|theme| {
                        let mut style = theme::card::simple(theme);
                        style.background = Some(iced::Background::Color(iced::Color::from_rgb(0.1, 0.1, 0.1)));
                        style
                    })
                )
                .push(
                    container(
                        Column::new()
                            .spacing(10)
                            .push(
                                Row::new()
                                    .spacing(10)
                                    .push(ui_text::text("⚠️").size(20))
                                    .push(ui_text::p1_bold("Security Warning"))
                            )
                            .push(ui_text::text("• Never share these words with anyone"))
                            .push(ui_text::text("• Store them offline in a safe place"))
                            .push(ui_text::text("• Anyone with these words can access your Lightning funds"))
                    )
                    .padding(20)
                    .style(|theme| {
                        let mut style = theme::card::simple(theme);
                        style.border.color = color::RED;
                        style.border.width = 2.0;
                        style
                    })
                )
                .push(
                    ui_button::primary(None, "I Have Backed Up My Recovery Phrase")
                        .on_press(ViewMessage::Active(ActiveMessage::ConfirmBackup))
                        .width(Length::Fill)
                )
        )
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into()
    }

    #[cfg(feature = "breez")]
    fn view_balance<'a>(&'a self, balance: &'a BalanceInfo) -> Element<'a, ViewMessage> {
        let mut col = Column::new()
            .spacing(10)
            .padding(20)
            .width(Length::Fill);

        col = col.push(
            ui_text::h2("Balance")
                .width(Length::Fill),
        );

        // Lightning balance
        col = col.push(
            Row::new()
                .spacing(10)
                .push(ui_text::text("Lightning:"))
                .push(ui_text::text(format!("{} sats", balance.lightning_balance_sat)))
        );

        // Liquid balance
        col = col.push(
            Row::new()
                .spacing(10)
                .push(ui_text::text("Liquid:"))
                .push(ui_text::text(format!("{} sats", balance.liquid_balance_sat)))
        );

        // Pending
        if balance.pending_send_sat > 0 || balance.pending_receive_sat > 0 {
            col = col.push(
                Row::new()
                    .spacing(10)
                    .push(ui_text::text("Pending:"))
                    .push(ui_text::text(format!(
                        "Send: {}, Receive: {}",
                        balance.pending_send_sat, balance.pending_receive_sat
                    )))
            );
        }

        // Refresh button
        col = col.push(
            ui_button::secondary(None, "Refresh")
                .on_press(ViewMessage::Active(ActiveMessage::RefreshBalance))
                .width(Length::Shrink)
        );

        container(col)
            .padding(10)
            .style(|theme| theme::card::simple(theme))
            .into()
    }

    fn view_send<'a>(&'a self) -> Element<'a, ViewMessage> {
        let mut col = Column::new()
            .spacing(20)
            .padding(20);

        col = col.push(ui_text::h2("Send Payment"));

        // Destination input field
        col = col.push(
            Column::new()
                .spacing(5)
                .push(TextTrait::small(ui_text::text("Lightning Invoice or Address")))
                .push(
                    TextInput::new("lnbc... or address", &self.destination)
                        .on_input(|value| ViewMessage::Active(ActiveMessage::DestinationEdited(value)))
                        .padding(12)
                        .size(16)
                )
        );

        // Amount input field
        col = col.push(
            Column::new()
                .spacing(5)
                .push(TextTrait::small(ui_text::text("Amount (sats)")))
                .push(
                    TextInput::new("1000", &self.amount)
                        .on_input(|value| ViewMessage::Active(ActiveMessage::AmountEdited(value)))
                        .padding(12)
                        .size(16)
                )
        );

        // Description input field (optional)
        col = col.push(
            Column::new()
                .spacing(5)
                .push(TextTrait::small(ui_text::text("Description (optional)")))
                .push(
                    TextInput::new("Payment note", &self.description)
                        .on_input(|value| ViewMessage::Active(ActiveMessage::DescriptionEdited(value)))
                        .padding(12)
                        .size(16)
                )
        );

        // Action buttons row
        #[cfg(feature = "breez")]
        {
            let button_row = if self.prepare_send_response.is_none() {
                // Show "Prepare Payment" button
                let prepare_enabled = self.destination_valid && self.amount_valid && !self.preparing;
                let prepare_button = if prepare_enabled {
                    ui_button::primary(None, if self.preparing { "Preparing..." } else { "Prepare Payment" })
                        .on_press(ViewMessage::Active(ActiveMessage::PrepareSend))
                } else {
                    ui_button::primary(None, "Prepare Payment")
                };
                
                Row::new()
                    .spacing(10)
                    .push(prepare_button.width(Length::Fill))
                    .push(
                        ui_button::secondary(None, "Back")
                            .on_press(ViewMessage::Active(ActiveMessage::ShowMainPanel))
                            .width(Length::Fill)
                    )
            } else {
                // Show "Send Payment" button when prepared
                let send_button = if self.sending {
                    ui_button::primary(None, "Sending...")
                } else {
                    ui_button::primary(None, "Send Payment")
                        .on_press(ViewMessage::Active(ActiveMessage::SendPayment))
                };
                
                Row::new()
                    .spacing(10)
                    .push(send_button.width(Length::Fill))
                    .push(
                        ui_button::secondary(None, "Back")
                            .on_press(ViewMessage::Active(ActiveMessage::ShowMainPanel))
                            .width(Length::Fill)
                    )
            };
            
            col = col.push(button_row);
        }

        // Payment limits display
        #[cfg(feature = "breez")]
        if let Some((min_sat, max_sat)) = self.payment_limits {
            col = col.push(
                container(
                    Row::new()
                        .spacing(10)
                        .push(
                            ui_text::text("ℹ")
                                .size(16)
                                .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                        )
                        .push(
                            TextTrait::small(ui_text::text(format!(
                                "Payment limits: {} - {} sats",
                                min_sat, max_sat
                            )))
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                        )
                )
                .padding(10)
                .style(theme::card::simple)
            );
        }

        // Prepared payment info (fee breakdown)
        #[cfg(feature = "breez")]
        if let Some(ref prep_response) = self.prepare_send_response {
            let fees_sat = prep_response.fees_sat.unwrap_or(0);
            let amount_sat = self.amount.parse::<u64>().unwrap_or(0);
            let total_sat = amount_sat + fees_sat;
            
            col = col.push(
                container(
                    Column::new()
                        .spacing(12)
                        .push(
                            ui_text::text("Payment Breakdown")
                                .size(16)
                                .style(|_| iced::widget::text::Style { color: Some(color::GREEN) })
                        )
                        .push(
                            Row::new()
                                .spacing(10)
                                .push(ui_text::text("Amount:").size(14))
                                .push(Space::with_width(Length::Fill))
                                .push(ui_text::text(format!("{} sats", amount_sat)).size(14))
                        )
                        .push(
                            Row::new()
                                .spacing(10)
                                .push(ui_text::text("Network Fee:").size(14))
                                .push(Space::with_width(Length::Fill))
                                .push(ui_text::text(format!("{} sats", fees_sat)).size(14))
                        )
                        .push(
                            Container::new(Space::with_height(Length::Fixed(1.0)))
                                .width(Length::Fill)
                                .style(|_theme| {
                                    iced::widget::container::Style::default()
                                        .background(color::GREY_2)
                                })
                        )
                        .push(
                            Row::new()
                                .spacing(10)
                                .push(ui_text::text("Total:").size(16))
                                .push(Space::with_width(Length::Fill))
                                .push(
                                    ui_text::text(format!("{} sats", total_sat))
                                        .size(16)
                                        .style(|_| iced::widget::text::Style { color: Some(color::GREEN) })
                                )
                        )
                )
                .padding(15)
                .width(Length::Fill)
                .style(theme::card::simple)
            );
        }

        col.into()
    }

    fn view_receive<'a>(&'a self) -> Element<'a, ViewMessage> {
        let mut col = Column::new()
            .spacing(20)
            .padding(20);

        col = col.push(ui_text::h2("Receive Payment"));

        // Info text
        col = col.push(
            ui_text::text("Generate a Lightning invoice to receive payments")
                .small()
                .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) }),
        );

        // Amount input (optional)
        col = col.push(
            ui_text::text(format!("Amount: {} sats", if self.amount.is_empty() { "Any" } else { &self.amount }))
        );

        // Amount validation feedback
        if !self.amount.is_empty() {
            if self.amount_valid {
                col = col.push(
                    ui_text::text("✓ Valid amount")
                        .small()
                        .style(|_| iced::widget::text::Style { color: Some(color::GREEN) }),
                );
            } else {
                col = col.push(
                    ui_text::text("⚠ Invalid amount format")
                        .small()
                        .style(|_| iced::widget::text::Style { color: Some(color::ORANGE) }),
                );
            }
        } else {
            col = col.push(
                ui_text::text("Invoice will accept any amount")
                    .small()
                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) }),
            );
        }

        // Description input field
        col = col.push(
            Column::new()
                .spacing(5)
                .push(TextTrait::small(ui_text::text("Description (optional)")))
                .push(
                    TextInput::new("What is this payment for?", &self.description)
                        .on_input(|value| ViewMessage::Active(ActiveMessage::DescriptionEdited(value)))
                        .padding(12)
                        .size(16)
                )
        );

        // Receive limits display
        #[cfg(feature = "breez")]
        if let Some((min_sat, max_sat)) = self.payment_limits {
            col = col.push(
                container(
                    Column::new()
                        .spacing(5)
                        .push(TextTrait::small(ui_text::text("Receive Limits:")))
                        .push(
                            TextTrait::small(ui_text::text(format!(
                                "Min: {} sats, Max: {} sats",
                                min_sat, max_sat
                            )))
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                        )
                )
                .padding(10)
                .style(|theme| theme::card::simple(theme))
            );
        }

        // Generated invoice display
        #[cfg(feature = "breez")]
        if let Some(ref invoice) = self.generated_invoice {
            col = col.push(self.view_invoice(invoice));
        }

        // Status message
        if self.preparing {
            col = col.push(
                ui_text::text("Generating invoice...")
                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) }),
            );
        }

        // Buttons
        let mut buttons = Row::new().spacing(10);

        // Back button
        buttons = buttons.push(
            ui_button::secondary(None, "Back")
                .on_press(ViewMessage::Previous)
                .width(Length::Shrink),
        );

        // Generate button
        let can_generate = !self.preparing;
        let generate_btn = ui_button::primary(None, "Generate Invoice")
            .width(Length::Shrink);
        buttons = buttons.push(if can_generate {
            generate_btn.on_press(ViewMessage::Active(ActiveMessage::GenerateInvoice))
        } else {
            generate_btn
        });

        // Copy button (only shown when invoice is generated)
        #[cfg(feature = "breez")]
        if self.generated_invoice.is_some() {
            buttons = buttons.push(
                ui_button::secondary(None, "Copy Invoice")
                    .on_press(ViewMessage::Clipboard(
                        self.generated_invoice.clone().unwrap_or_default()
                    ))
                    .width(Length::Shrink)
            );
        }

        col = col.push(buttons);

        col.into()
    }

    #[cfg(feature = "breez")]
    fn view_invoice<'a>(&'a self, invoice: &'a str) -> Element<'a, ViewMessage> {
        let mut col = Column::new()
            .spacing(15)
            .width(Length::Fill);

        col = col.push(
            ui_text::text("⚡ Lightning Invoice Generated")
                .style(|_| iced::widget::text::Style { color: Some(color::GREEN) }),
        );

        // QR Code placeholder (would use actual QR code generation library)
        col = col.push(
            container(
                Column::new()
                    .spacing(10)
                    .push(
                        ui_text::text("📱 QR CODE")
                            .size(48)
                            .width(Length::Fill),
                    )
                    .push(
                        TextTrait::small(ui_text::text("(Scan with Lightning wallet)"))
                            .width(Length::Fill)
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) }),
                    ),
            )
            .padding(30)
            .width(Length::Fill)
            .style(|theme| theme::card::simple(theme))
        );

        // Invoice string (scrollable, monospace)
        col = col.push(
            container(
                scrollable(
                    ui_text::text(invoice)
                        .size(12)
                        .width(Length::Fill)
                )
                .height(Length::Fixed(80.0))
            )
            .padding(10)
            .width(Length::Fill)
            .style(|theme| theme::card::simple(theme))
        );

        // Invoice details
        col = col.push(
            container(
                Column::new()
                    .spacing(5)
                    .push(TextTrait::small(ui_text::text("Invoice Details:")))
                    .push(
                        TextTrait::small(ui_text::text(format!(
                            "Amount: {}",
                            if self.amount.is_empty() {
                                "Any".to_string()
                            } else {
                                format!("{} sats", self.amount)
                            }
                        )))
                        .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
                    .push(
                        TextTrait::small(ui_text::text(format!(
                            "Description: {}",
                            if self.description.is_empty() {
                                "None".to_string()
                            } else {
                                self.description.clone()
                            }
                        )))
                        .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
                    .push(
                        TextTrait::small(ui_text::text("Status: Waiting for payment"))
                            .style(|_| iced::widget::text::Style { color: Some(color::ORANGE) })
                    )
            )
            .padding(10)
            .width(Length::Fill)
            .style(|theme| theme::card::simple(theme))
        );

        container(col)
            .padding(15)
            .style(theme::card::simple)
            .into()
    }

    fn view_settings<'a>(&'a self) -> Element<'a, ViewMessage> {
        let mut col = Column::new()
            .spacing(20)
            .padding(20);

        col = col.push(ui_text::h2("Lightning Settings"));

        // Network info
        col = col.push(
            container(
                Column::new()
                    .spacing(10)
                    .push(ui_text::text("Network").size(14))
                    .push(
                        TextTrait::small(ui_text::text(format!("{:?}", self.network)))
                            .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
            )
            .padding(15)
            .width(Length::Fill)
            .style(theme::card::simple)
        );

        // Connection status
        #[cfg(feature = "breez")]
        {
            let status_text = if self.breez_manager.is_some() {
                "✓ Connected to Breez SDK"
            } else {
                "⚠ Not connected"
            };
            
            let status_color = if self.breez_manager.is_some() {
                color::GREEN
            } else {
                color::ORANGE
            };
            
            col = col.push(
                container(
                    Column::new()
                        .spacing(10)
                        .push(ui_text::text("Connection Status").size(14))
                        .push(
                            ui_text::text(status_text)
                                .style(move |_| iced::widget::text::Style { color: Some(status_color) })
                        )
                )
                .padding(15)
                .width(Length::Fill)
                .style(theme::card::simple)
            );
        }

        #[cfg(not(feature = "breez"))]
        {
            col = col.push(
                container(
                    ui_text::text("Breez feature not enabled")
                        .style(|_| iced::widget::text::Style { color: Some(color::ORANGE) })
                )
                .padding(15)
                .width(Length::Fill)
                .style(theme::card::simple)
            );
        }

        // Back button
        col = col.push(
            ui_button::secondary(None, "Back")
                .on_press(ViewMessage::Previous)
                .width(Length::Shrink),
        );

        col.into()
    }

    #[allow(dead_code)]
    #[allow(dead_code)]
    fn view_history<'a>(&'a self) -> Element<'a, ViewMessage> {
        let mut col = Column::new()
            .spacing(20)
            .padding(20);

        use super::history::{view_history, PaymentFilter};
        
        #[cfg(feature = "breez")]
        {
            view_history(&self.payments, PaymentFilter::All)
        }
        
        #[cfg(not(feature = "breez"))]
        {
            view_history(&[], PaymentFilter::All)
        }
    }

    // Breez SDK operations
    #[cfg(feature = "breez")]
    pub fn prepare_send(&mut self) -> iced::Task<crate::app::message::Message> {
        use crate::app::breez::BreezSendManager;

        // Clear previous state
        self.error = None;
        self.prepare_send_response = None;
        self.preparing = true;

        let Some(ref manager) = self.breez_manager else {
            self.error = Some("Breez not initialized".to_string());
            self.preparing = false;
            return iced::Task::none();
        };

        let Ok(sdk) = manager.sdk() else {
            self.error = Some("Breez SDK not available".to_string());
            self.preparing = false;
            return iced::Task::none();
        };

        // Validate amount
        let amount_sat = match self.amount.parse::<u64>() {
            Ok(amt) if amt > 0 => Some(amt),
            _ => {
                self.error = Some("Please enter a valid amount greater than 0".to_string());
                self.preparing = false;
                return iced::Task::none();
            }
        };

        let destination = self.destination.clone();
        let send_manager = BreezSendManager::new(sdk);

        iced::Task::perform(
            async move { send_manager.prepare_send(destination, amount_sat).await },
            |result| {
                crate::app::message::Message::View(ViewMessage::Active(match result {
                    Ok(response) => ActiveMessage::PaymentPrepared(response),
                    Err(e) => ActiveMessage::PrepareFailed(e.to_string()),
                }))
            },
        )
    }

    #[cfg(feature = "breez")]
    pub fn send_payment(&mut self) -> iced::Task<crate::app::message::Message> {
        use crate::app::breez::BreezSendManager;

        self.error = None;
        self.sending = true;

        let Some(ref manager) = self.breez_manager else {
            self.error = Some("Breez not initialized".to_string());
            self.sending = false;
            return iced::Task::none();
        };

        let Ok(sdk) = manager.sdk() else {
            self.error = Some("Breez SDK not available".to_string());
            self.sending = false;
            return iced::Task::none();
        };

        let Some(ref prep_response) = self.prepare_send_response else {
            self.error = Some("Please prepare payment first".to_string());
            self.sending = false;
            return iced::Task::none();
        };

        let prep_response_clone = prep_response.clone();
        let send_manager = BreezSendManager::new(sdk);

        iced::Task::perform(
            async move { send_manager.send_payment(&prep_response_clone).await },
            |result| {
                crate::app::message::Message::View(ViewMessage::Active(match result {
                    Ok(response) => ActiveMessage::PaymentSent(format!(
                        "Payment sent successfully! TX: {}",
                        response.payment.tx_id.clone().unwrap_or_default()
                    )),
                    Err(e) => ActiveMessage::SendFailed(e.to_string()),
                }))
            },
        )
    }

    #[cfg(feature = "breez")]
    pub fn fetch_limits(&mut self) -> iced::Task<crate::app::message::Message> {
        use crate::app::breez::BreezSendManager;

        let Some(ref manager) = self.breez_manager else {
            return iced::Task::none();
        };

        let Ok(sdk) = manager.sdk() else {
            return iced::Task::none();
        };

        let send_manager = BreezSendManager::new(sdk);

        iced::Task::perform(
            async move {
                send_manager
                    .fetch_payment_limits(PaymentMethod::Bolt11Invoice)
                    .await
            },
            |result| {
                crate::app::message::Message::View(ViewMessage::Active(match result {
                    Ok(limits) => ActiveMessage::LimitsFetched(limits.min_sat, limits.max_sat),
                    Err(_) => ActiveMessage::Error("Failed to fetch limits".to_string()),
                }))
            },
        )
    }

    #[cfg(feature = "breez")]
    pub fn generate_invoice(&mut self) -> iced::Task<crate::app::message::Message> {
        use crate::app::breez::BreezReceiveManager;

        // Clear previous state
        self.error = None;
        self.generated_invoice = None;
        self.preparing = true;

        let Some(ref manager) = self.breez_manager else {
            self.error = Some("Breez not initialized".to_string());
            self.preparing = false;
            return iced::Task::none();
        };

        let Ok(sdk) = manager.sdk() else {
            self.error = Some("Breez SDK not available".to_string());
            self.preparing = false;
            return iced::Task::none();
        };

        // Validate amount if provided
        let amount = if !self.amount.is_empty() {
            match self.amount.parse::<u64>() {
                Ok(amt) if amt > 0 => Some(amt),
                _ => {
                    self.error = Some("Please enter a valid amount or leave empty".to_string());
                    self.preparing = false;
                    return iced::Task::none();
                }
            }
        } else {
            None
        };

        let description = if self.description.is_empty() {
            None
        } else {
            Some(self.description.clone())
        };

        let receive_manager = BreezReceiveManager::new(sdk);

        // First prepare, then receive
        iced::Task::perform(
            async move {
                // Prepare receive to get fees and limits
                let prep_response = receive_manager.prepare_receive(amount, description.clone()).await?;
                
                // Actually receive payment (generate invoice)
                let receive_response = receive_manager.receive_payment(&prep_response, description).await?;
                
                Ok(receive_response)
            },
            |result: Result<ReceivePaymentResponse, crate::app::breez::BreezError>| {
                crate::app::message::Message::View(ViewMessage::Active(match result {
                    Ok(response) => ActiveMessage::InvoiceGenerated(response.destination),
                    Err(e) => ActiveMessage::InvoiceGenerationFailed(e.to_string()),
                }))
            },
        )
    }

    #[cfg(feature = "breez")]
    pub fn refresh_balance(&mut self) -> iced::Task<crate::app::message::Message> {
        use crate::app::breez::BreezReceiveManager;

        let Some(ref manager) = self.breez_manager else {
            return iced::Task::none();
        };

        let Ok(sdk) = manager.sdk() else {
            return iced::Task::none();
        };

        let receive_manager = BreezReceiveManager::new(sdk);

        iced::Task::perform(async move { receive_manager.get_balance().await }, |result| {
            crate::app::message::Message::View(ViewMessage::Active(match result {
                Ok(balance) => ActiveMessage::BalanceUpdated(balance),
                Err(e) => ActiveMessage::Error(e.to_string()),
            }))
        })
    }

    #[cfg(feature = "breez")]
    pub fn fetch_receive_limits(&mut self) -> iced::Task<crate::app::message::Message> {
        use crate::app::breez::BreezReceiveManager;

        let Some(ref manager) = self.breez_manager else {
            return iced::Task::none();
        };

        let Ok(sdk) = manager.sdk() else {
            return iced::Task::none();
        };

        let receive_manager = BreezReceiveManager::new(sdk);

        iced::Task::perform(
            async move { receive_manager.fetch_receive_limits().await },
            |result| {
                crate::app::message::Message::View(ViewMessage::Active(match result {
                    Ok(limits) => ActiveMessage::LimitsFetched(limits.min_sat, limits.max_sat),
                    Err(_) => ActiveMessage::Error("Failed to fetch receive limits".to_string()),
                }))
            },
        )
    }

    #[cfg(feature = "breez")]
    pub fn load_payment_history(&mut self) -> iced::Task<crate::app::message::Message> {
        use crate::app::breez::BreezPaymentManager;

        let Some(ref manager) = self.breez_manager else {
            return iced::Task::none();
        };

        let Ok(sdk) = manager.sdk() else {
            return iced::Task::none();
        };

        self.loading_payments = true;
        self.payment_error = None;

        let payment_manager = BreezPaymentManager::new(sdk);

        iced::Task::perform(
            async move {
                match payment_manager.list_payments().await {
                    Ok(payments) => {
                        // Convert SDK Payment to our PaymentInfo
                        let payment_infos: Vec<crate::app::breez::PaymentInfo> = 
                            payments.into_iter().map(|p| p.into()).collect();
                        crate::app::view::ActiveMessage::PaymentHistoryLoaded(payment_infos)
                    }
                    Err(e) => crate::app::view::ActiveMessage::PaymentHistoryLoadFailed(
                        format!("Failed to load payments: {}", e)
                    ),
                }
            },
            |msg| crate::app::message::Message::View(crate::app::view::Message::Active(msg)),
        )
    }
}








