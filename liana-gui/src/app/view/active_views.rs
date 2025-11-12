use iced::{widget::Column, widget::Space, Length};
use liana_ui::{component::text::*, widget::*};

use crate::app::view::message::Message;

pub fn active_send_view(wallet_name: &str) -> Element<Message> {
    // This will be replaced with actual Breez send implementation
    Column::new()
        .spacing(20)
        .width(Length::Fill)
        .push(h3("Active - Send"))
        .push(text(format!("Wallet: {}", wallet_name)))
        .push(Space::with_height(Length::Fixed(20.0)))
        .push(text("Lightning send functionality integrated here.").size(15))
        .into()
}

pub fn active_receive_view(wallet_name: &str) -> Element<Message> {
    // This will be replaced with actual Breez receive implementation
    Column::new()
        .spacing(20)
        .width(Length::Fill)
        .push(h3("Active - Receive"))
        .push(text(format!("Wallet: {}", wallet_name)))
        .push(Space::with_height(Length::Fixed(20.0)))
        .push(text("Lightning receive functionality integrated here.").size(15))
        .into()
}

pub fn active_transactions_view(wallet_name: &str) -> Element<Message> {
    // This will be replaced with actual Breez transaction history
    Column::new()
        .spacing(20)
        .width(Length::Fill)
        .push(h3("Active - Transactions"))
        .push(text(format!("Wallet: {}", wallet_name)))
        .push(Space::with_height(Length::Fixed(20.0)))
        .push(text("Lightning transaction history integrated here.").size(15))
        .into()
}

pub fn active_settings_view(wallet_name: &str) -> Element<Message> {
    Column::new()
        .spacing(20)
        .width(Length::Fill)
        .push(h3("Active - Settings"))
        .push(text(format!("Wallet: {}", wallet_name)))
        .push(Space::with_height(Length::Fixed(20.0)))
        .push(text("This is a placeholder for the Active Settings page.").size(15))
        .push(text("Lightning Network settings will be configured here.").size(15))
        .into()
}
