use std::sync::Arc;

use iced::Task;
use liana_ui::widget::*;

use super::{Cache, Menu, State};
use crate::app::{message::Message, view, wallet::Wallet};
use crate::daemon::Daemon;
use crate::dir::LianaDirectory;

#[cfg(feature = "breez")]
use crate::app::breez::wallet::BreezWalletManager;
#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::PrepareSendResponse;

/// ActiveSend panel with Breez Lightning send functionality
pub struct ActiveSend {
    wallet: Option<Arc<Wallet>>,
    
    // Breez Lightning state
    #[cfg(feature = "breez")]
    pub breez_manager: Option<BreezWalletManager>,
    #[cfg(feature = "breez")]
    pub balance: Option<crate::app::breez::BalanceInfo>,
    #[cfg(feature = "breez")]
    pub lightning_address: Option<String>,
    pub network: liana::miniscript::bitcoin::Network,
    pub data_dir: LianaDirectory,
    
    // Send state
    pub destination: String,
    pub amount: String,
    pub destination_valid: bool,
    pub amount_valid: bool,
    #[cfg(feature = "breez")]
    pub prepare_send_response: Option<PrepareSendResponse>,
    pub preparing: bool,
    pub sending: bool,
    pub description: String,
    pub error: Option<String>,
}

impl ActiveSend {
    pub fn new(
        wallet: Arc<Wallet>,
        network: liana::miniscript::bitcoin::Network,
        data_dir: LianaDirectory,
    ) -> Self {
        Self {
            wallet: Some(wallet),
            #[cfg(feature = "breez")]
            breez_manager: None,
            #[cfg(feature = "breez")]
            balance: None,
            #[cfg(feature = "breez")]
            lightning_address: None,
            network,
            data_dir,
            destination: String::new(),
            amount: String::new(),
            destination_valid: false,
            amount_valid: false,
            #[cfg(feature = "breez")]
            prepare_send_response: None,
            preparing: false,
            sending: false,
            description: String::new(),
            error: None,
        }
    }
    
    /// Render the Lightning send view
    pub fn view_content<'a>(&'a self) -> Element<'a, view::Message> {
        use iced::widget::{Column, Row, Space, TextInput, container};
        use liana_ui::{color, component::{button as ui_button, text as ui_text}, theme};
        use liana_ui::component::text::Text as TextTrait;
        
        let mut col = Column::new()
            .spacing(20)
            .padding(20);

        // Header with balance and address
        col = col.push(self.view_lightning_header());
        
        col = col.push(ui_text::h2("Send Lightning Payment"));
        
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

        // Destination input field
        col = col.push(
            Column::new()
                .spacing(5)
                .push(TextTrait::small(ui_text::text("Lightning Invoice or Address")))
                .push(
                    TextInput::new("lnbc... or lightning address", &self.destination)
                        .on_input(|value| view::Message::Active(view::ActiveMessage::DestinationEdited(value)))
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
                        .on_input(|value| view::Message::Active(view::ActiveMessage::AmountEdited(value)))
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
                        .on_input(|value| view::Message::Active(view::ActiveMessage::DescriptionEdited(value)))
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
                        .on_press(view::Message::Active(view::ActiveMessage::PrepareSend))
                } else {
                    ui_button::primary(None, "Prepare Payment")
                };
                
                Row::new()
                    .spacing(10)
                    .push(prepare_button.width(iced::Length::Fill))
            } else {
                // Show "Send Payment" button when prepared
                let send_button = if self.sending {
                    ui_button::primary(None, "Sending...")
                } else {
                    ui_button::primary(None, "Send Payment")
                        .on_press(view::Message::Active(view::ActiveMessage::SendPayment))
                };
                
                Row::new()
                    .spacing(10)
                    .push(send_button.width(iced::Length::Fill))
            };
            
            col = col.push(button_row);
        }

        #[cfg(not(feature = "breez"))]
        {
            col = col.push(
                ui_text::text("Breez Lightning not enabled")
                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
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
                                .push(Space::with_width(iced::Length::Fill))
                                .push(ui_text::text(format!("{} sats", amount_sat)).size(14))
                        )
                        .push(
                            Row::new()
                                .spacing(10)
                                .push(ui_text::text("Network Fee:").size(14))
                                .push(Space::with_width(iced::Length::Fill))
                                .push(ui_text::text(format!("{} sats", fees_sat)).size(14))
                        )
                        .push(
                            Row::new()
                                .spacing(10)
                                .push(ui_text::text("Total:").size(16).style(|_| iced::widget::text::Style { color: Some(color::GREEN) }))
                                .push(Space::with_width(iced::Length::Fill))
                                .push(ui_text::text(format!("{} sats", total_sat)).size(16).style(|_| iced::widget::text::Style { color: Some(color::GREEN) }))
                        )
                )
                .padding(15)
                .style(theme::card::simple)
            );
        }

        col.into()
    }
    
    /// Render Coincube Active header with balance and address (similar to Buy/Sell)
    fn view_lightning_header<'a>(&'a self) -> Element<'a, view::Message> {
        use iced::widget::{Column, Row, Space, Container};
        use iced::{Alignment, Length};
        use liana_ui::{color, component::text as ui_text, theme};
        
        let mut header_col = Column::new()
            .spacing(15)
            .push(Space::with_height(Length::Fixed(100.0)));
        
        // COINCUBE Active branding (matching Buy/Sell style) - centered
        let branding = Container::new(
            Row::new()
                .push(
                    Row::new()
                        .push(ui_text::h4_bold("COIN").color(color::ORANGE))
                        .push(ui_text::h4_bold("CUBE").color(color::WHITE))
                        .spacing(0),
                )
                .push(Space::with_width(Length::Fixed(8.0)))
                .push(ui_text::h5_regular("Active").color(color::GREY_3))
                .align_y(Alignment::Center)
        )
        .width(Length::Fill)
        .align_x(Alignment::Center);
        
        header_col = header_col.push(branding);
        
        // Balance and address info cards
        #[cfg(feature = "breez")]
        {
            // Balance display card
            if let Some(ref balance_info) = self.balance {
                header_col = header_col.push(
                    Container::new(
                        Column::new()
                            .spacing(5)
                            .push(
                                ui_text::p2_regular("Lightning Balance")
                                    .color(color::GREY_3)
                            )
                            .push(
                                ui_text::h3(format!("⚡ {} sats", balance_info.lightning_balance_sat))
                                    .color(color::GREEN)
                            )
                            .align_x(Alignment::Center)
                    )
                    .width(Length::Fill)
                    .padding(15)
                    .style(theme::card::simple)
                );
            }
            
            // Lightning address display card
            if let Some(ref ln_address) = self.lightning_address {
                header_col = header_col.push(
                    Container::new(
                        Column::new()
                            .spacing(5)
                            .push(
                                ui_text::p2_regular("Lightning Address")
                                    .color(color::GREY_3)
                            )
                            .push(
                                ui_text::p1_regular(ln_address)
                                    .color(color::GREY_3)
                            )
                            .align_x(Alignment::Center)
                    )
                    .width(Length::Fill)
                    .padding(15)
                    .style(theme::card::simple)
                );
            }
        }
        
        header_col = header_col.push(Space::with_height(Length::Fixed(30.0)));
        
        header_col.into()
    }
    
    pub fn new_without_wallet(
        network: liana::miniscript::bitcoin::Network,
        data_dir: LianaDirectory,
        _cube_id: String,
        breez_manager: Option<BreezWalletManager>,
    ) -> Self {
        Self {
            wallet: None,
            #[cfg(feature = "breez")]
            breez_manager,
            #[cfg(feature = "breez")]
            balance: None,
            #[cfg(feature = "breez")]
            lightning_address: None,
            network,
            data_dir,
            destination: String::new(),
            amount: String::new(),
            destination_valid: false,
            amount_valid: false,
            #[cfg(feature = "breez")]
            prepare_send_response: None,
            preparing: false,
            sending: false,
            description: String::new(),
            error: None,
        }
    }
}

impl State for ActiveSend {
    fn view<'a>(&'a self, menu: &'a Menu, cache: &'a Cache) -> Element<'a, view::Message> {
        view::dashboard(
            menu,
            cache,
            None,
            self.view_content(),
        )
    }

    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _cache: &Cache,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::View(view::Message::Active(active_msg)) => {
                match active_msg {
                    view::ActiveMessage::DestinationEdited(value) => {
                        self.destination = value;
                        // Basic validation - just check if not empty
                        self.destination_valid = !self.destination.trim().is_empty();
                        Task::none()
                    }
                    view::ActiveMessage::AmountEdited(value) => {
                        self.amount = value;
                        // Validate amount is a positive number
                        self.amount_valid = self.amount.parse::<u64>().is_ok();
                        Task::none()
                    }
                    view::ActiveMessage::DescriptionEdited(value) => {
                        self.description = value;
                        Task::none()
                    }
                    view::ActiveMessage::PrepareSend => {
                        #[cfg(feature = "breez")]
                        {
                            if let Some(ref manager) = self.breez_manager {
                                self.preparing = true;
                                self.error = None;
                                
                                let manager = manager.clone();
                                let destination = self.destination.clone();
                                
                                return Task::perform(
                                    async move {
                                        Self::prepare_send_async(manager, destination).await
                                    },
                                    |result| {
                                        Message::View(view::Message::Active(match result {
                                            Ok(response) => view::ActiveMessage::PaymentPrepared(response),
                                            Err(e) => view::ActiveMessage::PrepareFailed(e),
                                        }))
                                    },
                                );
                            } else {
                                self.error = Some("Breez SDK not initialized".to_string());
                            }
                        }
                        #[cfg(not(feature = "breez"))]
                        {
                            self.error = Some("Breez feature not enabled".to_string());
                        }
                        Task::none()
                    }
                    #[cfg(feature = "breez")]
                    view::ActiveMessage::PaymentPrepared(response) => {
                        self.preparing = false;
                        self.prepare_send_response = Some(response);
                        Task::none()
                    }
                    view::ActiveMessage::SendPayment => {
                        #[cfg(feature = "breez")]
                        {
                            if let Some(ref manager) = self.breez_manager {
                                if let Some(ref prepare_response) = self.prepare_send_response {
                                    self.sending = true;
                                    self.error = None;
                                    
                                    let manager = manager.clone();
                                    let prepare_response = prepare_response.clone();
                                    
                                    return Task::perform(
                                        async move {
                                            Self::send_payment_async(manager, prepare_response).await
                                        },
                                        |result| {
                                            Message::View(view::Message::Active(match result {
                                                Ok(payment_id) => view::ActiveMessage::PaymentSent(payment_id),
                                                Err(e) => view::ActiveMessage::SendFailed(e),
                                            }))
                                        },
                                    );
                                } else {
                                    self.error = Some("Payment not prepared".to_string());
                                }
                            } else {
                                self.error = Some("Breez SDK not initialized".to_string());
                            }
                        }
                        #[cfg(not(feature = "breez"))]
                        {
                            self.error = Some("Breez feature not enabled".to_string());
                        }
                        Task::none()
                    }
                    view::ActiveMessage::PaymentSent(payment_id) => {
                        self.sending = false;
                        self.prepare_send_response = None;
                        // Clear form
                        self.destination.clear();
                        self.amount.clear();
                        self.description.clear();
                        self.destination_valid = false;
                        self.amount_valid = false;
                        
                        tracing::info!("✅ Payment sent successfully: {}", payment_id);
                        Task::none()
                    }
                    view::ActiveMessage::SendFailed(error) => {
                        self.sending = false;
                        self.error = Some(error);
                        Task::none()
                    }
                    view::ActiveMessage::PrepareFailed(error) => {
                        self.preparing = false;
                        self.error = Some(error);
                        Task::none()
                    }
                    view::ActiveMessage::CancelPayment => {
                        self.prepare_send_response = None;
                        self.preparing = false;
                        self.sending = false;
                        Task::none()
                    }
                    _ => Task::none(),
                }
            }
            _ => Task::none(),
        }
    }

    fn reload(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        wallet: Arc<Wallet>,
    ) -> Task<Message> {
        self.wallet = Some(wallet);
        Task::none()
    }
}

#[cfg(feature = "breez")]
impl ActiveSend {
    async fn prepare_send_async(
        manager: BreezWalletManager,
        destination: String,
    ) -> Result<breez_sdk_liquid::prelude::PrepareSendResponse, String> {
        use breez_sdk_liquid::prelude::PrepareSendRequest;
        
        let sdk = manager.sdk().map_err(|e| format!("SDK not available: {}", e))?;
        
        // Prepare the send payment
        let prepare_response = sdk
            .prepare_send_payment(&PrepareSendRequest {
                destination,
                amount: None, // Amount is embedded in BOLT11 invoice, None for parsing
            })
            .await
            .map_err(|e| format!("Failed to prepare payment: {}", e))?;
        
        tracing::info!("Payment prepared with fees: {:?} sats", prepare_response.fees_sat);
        
        Ok(prepare_response)
    }
    
    async fn send_payment_async(
        manager: BreezWalletManager,
        prepare_response: breez_sdk_liquid::prelude::PrepareSendResponse,
    ) -> Result<String, String> {
        use breez_sdk_liquid::prelude::SendPaymentRequest;
        
        let sdk = manager.sdk().map_err(|e| format!("SDK not available: {}", e))?;
        
        // Send the payment
        let send_response = sdk
            .send_payment(&SendPaymentRequest {
                prepare_response,
                payer_note: None,
                use_asset_fees: None,
            })
            .await
            .map_err(|e| format!("Failed to send payment: {}", e))?;
        
        tracing::info!("Payment sent successfully");
        // Use tx_id as the payment identifier
        Ok(send_response.payment.tx_id.clone().unwrap_or_else(|| "unknown".to_string()))
    }
}

/// ActiveReceive panel with Breez Lightning receive functionality
pub struct ActiveReceive {
    wallet: Option<Arc<Wallet>>,
    
    // Breez Lightning state
    #[cfg(feature = "breez")]
    pub breez_manager: Option<BreezWalletManager>,
    #[cfg(feature = "breez")]
    pub balance: Option<crate::app::breez::BalanceInfo>,
    #[cfg(feature = "breez")]
    pub lightning_address: Option<String>,
    pub network: liana::miniscript::bitcoin::Network,
    pub data_dir: LianaDirectory,
    
    // Receive state
    pub amount: String,
    pub description: String,
    #[cfg(feature = "breez")]
    pub generated_invoice: Option<String>,
    pub preparing: bool,
    pub error: Option<String>,
}

impl ActiveReceive {
    pub fn new(
        wallet: Arc<Wallet>,
        network: liana::miniscript::bitcoin::Network,
        data_dir: LianaDirectory,
    ) -> Self {
        Self {
            wallet: Some(wallet),
            #[cfg(feature = "breez")]
            breez_manager: None,
            #[cfg(feature = "breez")]
            balance: None,
            #[cfg(feature = "breez")]
            lightning_address: None,
            network,
            data_dir,
            amount: String::new(),
            description: String::new(),
            #[cfg(feature = "breez")]
            generated_invoice: None,
            preparing: false,
            error: None,
        }
    }
    
    /// Render the Lightning receive view
    pub fn view_content<'a>(&'a self) -> Element<'a, view::Message> {
        use iced::widget::{Column, Row, TextInput, container};
        use liana_ui::{color, component::{button as ui_button, text as ui_text}, theme};
        use liana_ui::component::text::Text as TextTrait;
        
        let mut col = Column::new()
            .spacing(20)
            .padding(20);

        // Header with balance and address
        col = col.push(self.view_lightning_header());
        
        col = col.push(ui_text::h2("Receive Lightning Payment"));
        
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

        // Amount input field
        col = col.push(
            Column::new()
                .spacing(5)
                .push(TextTrait::small(ui_text::text("Amount (sats)")))
                .push(
                    TextInput::new("Leave empty for any amount", &self.amount)
                        .on_input(|value| view::Message::Active(view::ActiveMessage::AmountEdited(value)))
                        .padding(12)
                        .size(16)
                )
        );

        // Description input field
        col = col.push(
            Column::new()
                .spacing(5)
                .push(TextTrait::small(ui_text::text("Description (optional)")))
                .push(
                    TextInput::new("Payment description", &self.description)
                        .on_input(|value| view::Message::Active(view::ActiveMessage::DescriptionEdited(value)))
                        .padding(12)
                        .size(16)
                )
        );

        // Action buttons
        let mut buttons = Row::new().spacing(10);

        // Generate button
        let can_generate = !self.preparing;
        let generate_btn = ui_button::primary(None, if self.preparing { "Generating..." } else { "Generate Invoice" })
            .width(iced::Length::Fill);
        buttons = buttons.push(if can_generate {
            generate_btn.on_press(view::Message::Active(view::ActiveMessage::GenerateInvoice))
        } else {
            generate_btn
        });

        col = col.push(buttons);

        // Show generated invoice
        #[cfg(feature = "breez")]
        if let Some(ref invoice) = self.generated_invoice {
            col = col.push(
                container(
                    Column::new()
                        .spacing(15)
                        .push(
                            ui_text::text("✓ Invoice Generated")
                                .size(18)
                                .style(|_| iced::widget::text::Style { color: Some(color::GREEN) })
                        )
                        .push(
                            container(
                                ui_text::text(invoice)
                                    .size(12)
                                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                            )
                            .padding(10)
                            .style(theme::card::simple)
                        )
                        .push(
                            ui_button::secondary(None, "Copy to Clipboard")
                                .on_press(view::Message::Clipboard(invoice.clone()))
                                .width(iced::Length::Fill)
                        )
                )
                .padding(15)
                .style(theme::card::simple)
            );
        }

        #[cfg(not(feature = "breez"))]
        {
            col = col.push(
                ui_text::text("Breez Lightning not enabled")
                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
            );
        }

        col.into()
    }
    
    /// Render Coincube Active header (shared with ActiveSend)
    fn view_lightning_header<'a>(&'a self) -> Element<'a, view::Message> {
        use iced::widget::{Column, Row, Space, Container};
        use iced::{Alignment, Length};
        use liana_ui::{color, component::text as ui_text};
        
        Column::new()
            .spacing(20)
            .push(Space::with_height(Length::Fixed(150.0)))
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
                        .push(ui_text::h5_regular("Active").color(color::GREY_3))
                        .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center),
            )
            .push(Space::with_height(Length::Fixed(20.0)))
            .into()
    }
    
    pub fn new_without_wallet(
        network: liana::miniscript::bitcoin::Network,
        data_dir: LianaDirectory,
        _cube_id: String,
        breez_manager: Option<BreezWalletManager>,
    ) -> Self {
        Self {
            wallet: None,
            #[cfg(feature = "breez")]
            breez_manager,
            #[cfg(feature = "breez")]
            balance: None,
            #[cfg(feature = "breez")]
            lightning_address: None,
            network,
            data_dir,
            amount: String::new(),
            description: String::new(),
            #[cfg(feature = "breez")]
            generated_invoice: None,
            preparing: false,
            error: None,
        }
    }
}

impl State for ActiveReceive {
    fn view<'a>(&'a self, menu: &'a Menu, cache: &'a Cache) -> Element<'a, view::Message> {
        view::dashboard(
            menu,
            cache,
            None,
            self.view_content(),
        )
    }

    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _cache: &Cache,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::View(view::Message::Active(active_msg)) => {
                match active_msg {
                    view::ActiveMessage::AmountEdited(value) => {
                        self.amount = value;
                        Task::none()
                    }
                    view::ActiveMessage::DescriptionEdited(value) => {
                        self.description = value;
                        Task::none()
                    }
                    view::ActiveMessage::GenerateInvoice => {
                        #[cfg(feature = "breez")]
                        {
                            tracing::info!("🔔 GenerateInvoice message received, breez_manager present: {}", self.breez_manager.is_some());
                            if let Some(ref manager) = self.breez_manager {
                                self.preparing = true;
                                self.error = None;
                                
                                let manager = manager.clone();
                                let amount = self.amount.clone();
                                let description = self.description.clone();
                                
                                return Task::perform(
                                    async move {
                                        Self::generate_invoice_async(manager, amount, description).await
                                    },
                                    |result| {
                                        Message::View(view::Message::Active(match result {
                                            Ok(invoice) => view::ActiveMessage::InvoiceGenerated(invoice),
                                            Err(e) => view::ActiveMessage::PrepareFailed(e),
                                        }))
                                    },
                                );
                            } else {
                                self.error = Some("Breez SDK not initialized".to_string());
                            }
                        }
                        #[cfg(not(feature = "breez"))]
                        {
                            self.error = Some("Breez feature not enabled".to_string());
                        }
                        Task::none()
                    }
                    view::ActiveMessage::InvoiceGenerated(invoice) => {
                        self.preparing = false;
                        self.generated_invoice = Some(invoice);
                        Task::none()
                    }
                    view::ActiveMessage::PrepareFailed(error) => {
                        self.preparing = false;
                        self.error = Some(error);
                        Task::none()
                    }
                    _ => Task::none(),
                }
            }
            _ => Task::none(),
        }
    }

    fn reload(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        wallet: Arc<Wallet>,
    ) -> Task<Message> {
        self.wallet = Some(wallet);
        Task::none()
    }
}

#[cfg(feature = "breez")]
impl ActiveReceive {
    async fn generate_invoice_async(
        manager: BreezWalletManager,
        amount_str: String,
        description: String,
    ) -> Result<String, String> {
        use breez_sdk_liquid::prelude::{PaymentMethod, PrepareReceiveRequest, ReceiveAmount, ReceivePaymentRequest};
        
        let sdk = manager.sdk().map_err(|e| format!("SDK not available: {}", e))?;
        
        // Parse amount (optional for BOLT11)
        let amount = if amount_str.trim().is_empty() {
            None
        } else {
            let sats: u64 = amount_str.parse().map_err(|_| "Invalid amount".to_string())?;
            Some(ReceiveAmount::Bitcoin { payer_amount_sat: sats })
        };
        
        // Prepare the receive payment
        let prepare_response = sdk
            .prepare_receive_payment(&PrepareReceiveRequest {
                payment_method: PaymentMethod::Bolt11Invoice,
                amount,
            })
            .await
            .map_err(|e| format!("Failed to prepare: {}", e))?;
        
        tracing::info!("Prepared invoice with fees: {} sats", prepare_response.fees_sat);
        
        // Generate the invoice
        let receive_response = sdk
            .receive_payment(&ReceivePaymentRequest {
                prepare_response,
                payer_note: if description.is_empty() { None } else { Some(description.clone()) },
                description: if description.is_empty() { None } else { Some(description) },
                use_description_hash: None,
            })
            .await
            .map_err(|e| format!("Failed to generate invoice: {}", e))?;
        
        tracing::info!("Invoice generated successfully");
        Ok(receive_response.destination)
    }
}

/// ActiveTransactions panel with Breez Lightning transaction history
pub struct ActiveTransactions {
    wallet: Option<Arc<Wallet>>,
    
    // Breez Lightning state  
    #[cfg(feature = "breez")]
    pub breez_manager: Option<BreezWalletManager>,
    #[cfg(feature = "breez")]
    pub balance: Option<crate::app::breez::BalanceInfo>,
    #[cfg(feature = "breez")]
    pub lightning_address: Option<String>,
    pub network: liana::miniscript::bitcoin::Network,
    pub data_dir: LianaDirectory,
    
    pub error: Option<String>,
}

impl ActiveTransactions {
    pub fn new(
        wallet: Arc<Wallet>,
        network: liana::miniscript::bitcoin::Network,
        data_dir: LianaDirectory,
    ) -> Self {
        Self {
            wallet: Some(wallet),
            #[cfg(feature = "breez")]
            breez_manager: None,
            #[cfg(feature = "breez")]
            balance: None,
            #[cfg(feature = "breez")]
            lightning_address: None,
            network,
            data_dir,
            error: None,
        }
    }
    
    /// Render the Lightning transaction history view
    pub fn view_content<'a>(&'a self) -> Element<'a, view::Message> {
        use iced::widget::{Column, container};
        use liana_ui::{color, component::text as ui_text, theme};
        
        let mut col = Column::new()
            .spacing(20)
            .padding(20);

        // Header with balance and address
        col = col.push(self.view_lightning_header());
        
        col = col.push(ui_text::h2("Lightning Transaction History"));
        
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

        #[cfg(feature = "breez")]
        {
            if self.breez_manager.is_some() {
                col = col.push(
                    ui_text::text("Transaction history will be displayed here")
                        .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                );
            } else {
                col = col.push(
                    ui_text::text("Lightning wallet not initialized")
                        .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
                );
            }
        }

        #[cfg(not(feature = "breez"))]
        {
            col = col.push(
                ui_text::text("Breez Lightning not enabled")
                    .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) })
            );
        }

        col.into()
    }
    
    /// Render Coincube Active header (shared with ActiveSend)
    fn view_lightning_header<'a>(&'a self) -> Element<'a, view::Message> {
        use iced::widget::{Column, Row, Space, Container};
        use iced::{Alignment, Length};
        use liana_ui::{color, component::text as ui_text};
        
        Column::new()
            .spacing(20)
            .push(Space::with_height(Length::Fixed(150.0)))
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
                        .push(ui_text::h5_regular("Active").color(color::GREY_3))
                        .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .align_x(Alignment::Center),
            )
            .push(Space::with_height(Length::Fixed(20.0)))
            .into()
    }
    
    pub fn new_without_wallet(
        network: liana::miniscript::bitcoin::Network,
        data_dir: LianaDirectory,
        _cube_id: String,
        breez_manager: Option<BreezWalletManager>,
    ) -> Self {
        Self {
            wallet: None,
            #[cfg(feature = "breez")]
            breez_manager,
            #[cfg(feature = "breez")]
            balance: None,
            #[cfg(feature = "breez")]
            lightning_address: None,
            network,
            data_dir,
            error: None,
        }
    }

    pub fn preselect(&mut self, _tx: crate::daemon::model::HistoryTransaction) {
        // Placeholder: In the future, this will preselect a transaction
    }
}

impl State for ActiveTransactions {
    fn view<'a>(&'a self, menu: &'a Menu, cache: &'a Cache) -> Element<'a, view::Message> {
        view::dashboard(
            menu,
            cache,
            None,
            self.view_content(),
        )
    }

    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _cache: &Cache,
        _message: Message,
    ) -> Task<Message> {
        Task::none()
    }

    fn reload(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        wallet: Arc<Wallet>,
    ) -> Task<Message> {
        self.wallet = Some(wallet);
        Task::none()
    }
}

/// ActiveSettings is a placeholder panel for the Active Settings page
pub struct ActiveSettings {
    wallet: Option<Arc<Wallet>>,
}

impl ActiveSettings {
    pub fn new(wallet: Arc<Wallet>) -> Self {
        Self { wallet: Some(wallet) }
    }
    
    pub fn new_without_wallet() -> Self {
        Self { wallet: None }
    }
}

impl State for ActiveSettings {
    fn view<'a>(&'a self, menu: &'a Menu, cache: &'a Cache) -> Element<'a, view::Message> {
        let wallet_name = self.wallet.as_ref()
            .map(|w| w.name.as_str())
            .unwrap_or("No Wallet");
        
        view::dashboard(
            menu,
            cache,
            None,
            view::active_views::active_settings_view(wallet_name),
        )
    }

    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _cache: &Cache,
        _message: Message,
    ) -> Task<Message> {
        Task::none()
    }

    fn reload(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        wallet: Arc<Wallet>,
    ) -> Task<Message> {
        self.wallet = Some(wallet);
        Task::none()
    }
}
