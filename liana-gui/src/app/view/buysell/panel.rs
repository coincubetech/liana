use iced::{
    widget::{container, pick_list, Space},
    Alignment, Length, Task,
};

use liana::miniscript::bitcoin::{self, Network};
use liana_ui::{
    color,
    component::{
        button,
        text::{self, text},
    },
    icon::*,
    theme,
    widget::*,
};

use crate::app::{
    self,
    view::{BuySellMessage, Message as ViewMessage},
};

#[derive(Debug, Clone, Copy)]
pub enum BuyOrSell {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LabelledAddress {
    pub address: bitcoin::Address,
    pub index: bitcoin::bip32::ChildNumber,
    pub label: Option<String>,
}

impl std::fmt::Display for LabelledAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.label {
            Some(l) => write!(f, "{}: {}", l, self.address),
            None => std::fmt::Display::fmt(&self.address, f),
        }
    }
}

pub enum BuySellFlowState {
    /// Detecting user's location via IP geolocation, true if geolocation failed and the user is manually prompted
    DetectingLocation(bool),
    /// Nigeria, Kenya and South Africa, ie Mavapay supported countries
    Mavapay(super::flow_state::MavapayState),
    /// For Onramper countries, render an interface to generate a new address for buysell
    AddressGeneration,
    /// A webview is currently active, and is rendered instead of a buysell UI
    WebviewRenderer { active: iced_wry::IcedWebview },
}

pub struct BuySellPanel {
    // Runtime state - determines which flow is active
    pub flow_state: BuySellFlowState,
    pub modal: app::state::vault::receive::Modal,
    pub buy_or_sell: Option<BuyOrSell>,

    // Common fields (always present)
    pub error: Option<String>,
    pub network: Network,

    // for address generation
    pub wallet: std::sync::Arc<crate::app::wallet::Wallet>,
    pub data_dir: crate::dir::LianaDirectory,
    pub generated_address: Option<LabelledAddress>,

    // services used by several buysell providers
    pub geolocation_service: crate::services::geolocation::HttpGeoLocator,
    pub detected_country_name: Option<String>,
    pub detected_country_iso: Option<String>,
    pub webview_manager: iced_wry::IcedWebviewManager,
}

impl BuySellPanel {
    pub fn new(
        network: bitcoin::Network,
        wallet: std::sync::Arc<crate::app::wallet::Wallet>,
        data_dir: crate::dir::LianaDirectory,
    ) -> Self {
        Self {
            buy_or_sell: None,
            error: None,
            network,
            wallet,
            data_dir,
            generated_address: None,
            modal: app::state::vault::receive::Modal::None,
            // Geolocation detection state
            geolocation_service: crate::services::geolocation::HttpGeoLocator::new(),
            detected_country_name: None,
            detected_country_iso: None,
            webview_manager: iced_wry::IcedWebviewManager::new(),
            // Start in detecting location state
            flow_state: BuySellFlowState::DetectingLocation(false),
        }
    }

    /// Opens Onramper widget session (only called for non-Mavapay countries)
    pub fn start_onramper_session(&mut self) -> iced::Task<BuySellMessage> {
        use crate::app::buysell::onramper;

        let mode = match self.buy_or_sell {
            None => return Task::none(),
            Some(BuyOrSell::Buy) => "buy",
            Some(BuyOrSell::Sell) => "sell",
        };

        let Some(iso_code) = self.detected_country_iso.as_ref() else {
            tracing::warn!("Unable to start session, country selection|detection was unsuccessful");
            return Task::none();
        };

        // This method is now only called for Onramper (non-Mavapay) flow
        let Some(currency) = crate::services::geolocation::get_countries()
            .iter()
            .find(|c| c.code == iso_code)
            .map(|c| c.currency.name)
        else {
            tracing::error!("Unknown country iso code: {}", iso_code);
            return Task::none();
        };

        // prepare parameters
        let address = self
            .generated_address
            .as_ref()
            .map(|a| a.address.to_string());

        match onramper::create_widget_url(&currency, address.as_deref(), &mode, self.network) {
            Ok(url) => Task::done(BuySellMessage::WebviewOpenUrl(url)),
            Err(error) => {
                tracing::error!("[ONRAMPER] Error: {}", error);
                Task::done(BuySellMessage::SessionError(error.to_string()))
            }
        }
    }

    pub fn view<'a>(&'a self) -> iced::Element<'a, ViewMessage, liana_ui::theme::Theme> {
        let column = {
            let column = Column::new()
                .push(Space::with_height(60))
                // COINCUBE branding
                .push(
                    Row::new()
                        .push(
                            Row::new()
                                .push(text::h4_bold("COIN").color(color::ORANGE))
                                .push(text::h4_bold("CUBE").color(color::WHITE))
                                .spacing(0),
                        )
                        .push(Space::with_width(Length::Fixed(8.0)))
                        .push(text::h5_regular("BUY/SELL").color(color::GREY_3))
                        .align_y(Alignment::Center),
                )
                // error display
                .push_maybe(self.error.as_ref().map(|err| {
                    Container::new(text(err).size(14).color(color::RED))
                        .padding(10)
                        .style(theme::card::invalid)
                }))
                .push_maybe(
                    self.error
                        .is_some()
                        .then(|| Space::with_height(Length::Fixed(20.0))),
                )
                // render flow state
                .push({
                    let element: iced::Element<ViewMessage, theme::Theme> = match &self.flow_state {
                        BuySellFlowState::DetectingLocation(m) => self.geolocation_ux(*m).into(),
                        BuySellFlowState::AddressGeneration => self.address_generation_ux().into(),
                        BuySellFlowState::Mavapay(state) => {
                            let element: iced::Element<BuySellMessage, theme::Theme> =
                                super::mavapay_ui::form(state).into();
                            element.map(|b| ViewMessage::BuySell(b))
                        }
                        BuySellFlowState::WebviewRenderer { active, .. } => {
                            BuySellPanel::webview_ux(self.network, active).into()
                        }
                    };

                    element
                });

            column
                .align_x(Alignment::Center)
                .spacing(7) // Reduced spacing for more compact layout
                .width(Length::Fill)
        };

        Container::new(column)
            .width(Length::Fill)
            .align_y(Alignment::Start)
            .align_x(Alignment::Center)
            .into()
    }

    fn webview_ux<'a>(
        network: liana::miniscript::bitcoin::Network,
        webview: &'a iced_wry::IcedWebview,
    ) -> Column<'a, ViewMessage> {
        iced::widget::column![
            webview.view(Length::Fixed(640.0), Length::Fixed(600.0)),
            // Network display banner
            Space::with_height(Length::Fixed(15.0)),
            {
                let (network_name, network_color) = match network {
                    liana::miniscript::bitcoin::Network::Bitcoin => {
                        ("Bitcoin Mainnet", color::GREEN)
                    }
                    liana::miniscript::bitcoin::Network::Testnet => {
                        ("Bitcoin Testnet", color::ORANGE)
                    }
                    liana::miniscript::bitcoin::Network::Testnet4 => {
                        ("Bitcoin Testnet4", color::ORANGE)
                    }
                    liana::miniscript::bitcoin::Network::Signet => ("Bitcoin Signet", color::BLUE),
                    liana::miniscript::bitcoin::Network::Regtest => ("Bitcoin Regtest", color::RED),
                };

                iced::widget::row![
                    // currently selected bitcoin network display
                    text("Network: ").size(12).color(color::GREY_3),
                    text(network_name).size(12).color(network_color),
                    // render a button that closes the webview
                    Space::with_width(Length::Fixed(25.0)),
                    {
                        button::secondary(Some(arrow_back()), "Start Over")
                            .on_press(ViewMessage::BuySell(BuySellMessage::ResetWidget))
                            .width(iced::Length::Fixed(300.0))
                    }
                ]
                .spacing(5)
                .align_y(Alignment::Center)
            }
        ]
    }

    fn address_generation_ux<'a>(&'a self) -> Column<'a, ViewMessage> {
        use iced::widget::scrollable;
        use liana_ui::component::{
            button, card,
            text::{p2_regular, Text},
        };

        let mut column = Column::new();
        column = match self.generated_address.as_ref() {
            Some(addr) => column
                .push(text("Generated Address").size(14).color(color::GREY_3))
                .push({
                    let address_text = addr.to_string();

                    card::simple(
                        Column::new()
                            .push(
                                Container::new(
                                    scrollable(
                                        Column::new()
                                            .push(Space::with_height(Length::Fixed(10.0)))
                                            .push(
                                                p2_regular(&address_text)
                                                    .small()
                                                    .style(theme::text::secondary),
                                            )
                                            // Space between the address and the scrollbar
                                            .push(Space::with_height(Length::Fixed(10.0))),
                                    )
                                    .direction(
                                        scrollable::Direction::Horizontal(
                                            scrollable::Scrollbar::new().width(2).scroller_width(2),
                                        ),
                                    ),
                                )
                                .width(Length::Fill),
                            )
                            .push(
                                Row::new()
                                    .push(
                                        button::secondary(None, "Verify on hardware device")
                                            .on_press(ViewMessage::Select(0)),
                                    )
                                    .push(Space::with_width(Length::Fill))
                                    .push(
                                        Button::new(qr_code_icon().style(theme::text::secondary))
                                            .on_press(ViewMessage::ShowQrCode(0))
                                            .style(theme::button::transparent_border),
                                    )
                                    .push(
                                        Button::new(clipboard_icon().style(theme::text::secondary))
                                            .on_press(ViewMessage::Clipboard(address_text))
                                            .style(theme::button::transparent_border),
                                    )
                                    .align_y(Alignment::Center),
                            )
                            .spacing(10),
                    )
                    .width(Length::Fill)
                })
                .push(
                    button::primary(Some(globe_icon()), "Continue")
                        .on_press_maybe(
                            self.detected_country_iso
                                .is_some()
                                .then_some(ViewMessage::BuySell(
                                    BuySellMessage::StartOnramperSession,
                                )),
                        )
                        .width(iced::Length::Fill),
                ),
            None => column
                .push({
                    let buy_or_sell = self.buy_or_sell.clone();

                    Column::new()
                        .push(
                            button::secondary(
                                Some(bitcoin_icon()),
                                "Buy Bitcoin using Fiat Currencies",
                            )
                            .on_press(ViewMessage::BuySell(BuySellMessage::SetBuyOrSell(
                                BuyOrSell::Buy,
                            )))
                            .style(move |th, st| match buy_or_sell {
                                Some(BuyOrSell::Buy) => liana_ui::theme::button::primary(th, st),
                                _ => liana_ui::theme::button::secondary(th, st),
                            })
                            .padding(30)
                            .width(iced::Length::Fill),
                        )
                        .push(
                            button::secondary(
                                Some(dollar_icon()),
                                "Sell Bitcoin to a Fiat Currency",
                            )
                            .on_press(ViewMessage::BuySell(BuySellMessage::SetBuyOrSell(
                                BuyOrSell::Sell,
                            )))
                            .style(move |th, st| match buy_or_sell {
                                Some(BuyOrSell::Sell) => liana_ui::theme::button::primary(th, st),
                                _ => liana_ui::theme::button::secondary(th, st),
                            })
                            .padding(30)
                            .width(iced::Length::Fill),
                        )
                        .spacing(15)
                        .padding(5)
                })
                .push_maybe({
                    self.buy_or_sell.is_some().then(|| {
                        container(Space::with_height(1))
                            .style(|_| {
                                iced::widget::container::background(iced::Background::Color(
                                    color::GREY_6,
                                ))
                            })
                            .width(Length::Fill)
                    })
                })
                .push_maybe({
                    (matches!(self.buy_or_sell, Some(BuyOrSell::Buy))).then(|| {
                        button::secondary(Some(plus_icon()), "Generate New Address")
                            .on_press_maybe(
                                matches!(self.buy_or_sell, Some(BuyOrSell::Buy)).then_some(
                                    ViewMessage::BuySell(BuySellMessage::CreateNewAddress),
                                ),
                            )
                            .width(iced::Length::Fill)
                    })
                })
                .push_maybe({
                    (matches!(self.buy_or_sell, Some(BuyOrSell::Sell))).then(|| {
                        button::secondary(Some(globe_icon()), "Continue")
                            .on_press_maybe(self.detected_country_iso.is_some().then_some(
                                ViewMessage::BuySell(BuySellMessage::StartOnramperSession),
                            ))
                            .width(iced::Length::Fill)
                    })
                }),
        };

        column
            .align_x(Alignment::Center)
            .spacing(12)
            .max_width(640)
            .width(Length::Fill)
    }

    fn geolocation_ux<'a>(&'a self, manual_selection: bool) -> Column<'a, ViewMessage> {
        use liana_ui::component::text;

        match manual_selection {
            true => Column::new()
                .push(
                    pick_list(
                        crate::services::geolocation::get_countries(),
                        None::<crate::services::geolocation::Country>,
                        |c| ViewMessage::BuySell(BuySellMessage::ManualCountrySelected(c)),
                    )
                    .padding(10)
                    .placeholder("Select Country: "),
                )
                .align_x(Alignment::Center)
                .width(Length::Fill),
            false => Column::new()
                .push(Space::with_height(Length::Fixed(30.0)))
                .push(text::p1_bold("Detecting your location...").color(color::WHITE))
                .push(Space::with_height(Length::Fixed(20.0)))
                .push(text("Please wait...").size(14).color(color::GREY_3))
                .align_x(Alignment::Center)
                .spacing(10)
                .max_width(500)
                .width(Length::Fill),
        }
    }
}
