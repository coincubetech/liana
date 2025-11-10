//! Payment confirmation dialog

use iced::{Alignment, Length};
use liana_ui::{
    color,
    component::{button as ui_button, text as ui_text},
    theme,
    widget::*,
};

use crate::app::view::{ActivateMessage, Message as ViewMessage};

#[cfg(feature = "breez")]
pub fn view_confirmation<'a>(
    destination: &'a str,
    amount: &'a str,
    prepare_response: &'a breez_sdk_liquid::PrepareSendResponse,
) -> Element<'a, ViewMessage> {
    let mut col = Column::new()
        .spacing(20)
        .padding(30)
        .align_x(Alignment::Center)
        .width(Length::Fixed(500.0));

    // Title
    col = col.push(
        ui_text::h2("Confirm Payment")
            .horizontal_alignment(iced::alignment::Horizontal::Center),
    );

    // Warning message
    col = col.push(
        ui_text::text("⚠ This action cannot be undone")
            .style(color::ORANGE)
            .horizontal_alignment(iced::alignment::Horizontal::Center),
    );

    // Payment details card
    let mut details = Column::new().spacing(10).padding(15);

    details = details.push(
        ui_text::text("Payment Details:")
            .size(16)
            .style(color::GREY_3),
    );

    details = details.push(
        Row::new()
            .spacing(10)
            .push(ui_text::text("Destination:").size(14))
            .push(
                ui_text::text(destination)
                    .size(14)
                    .style(color::GREY_3),
            ),
    );

    details = details.push(
        Row::new()
            .spacing(10)
            .push(ui_text::text("Amount:").size(14))
            .push(
                ui_text::text(format!("{} sats", amount))
                    .size(14)
                    .style(color::GREEN),
            ),
    );

    details = details.push(
        Row::new()
            .spacing(10)
            .push(ui_text::text("Fee:").size(14))
            .push(
                ui_text::text(format!("{} sats", prepare_response.fees_sat))
                    .size(14)
                    .style(color::ORANGE),
            ),
    );

    // Calculate total
    let amount_num: u64 = amount.parse().unwrap_or(0);
    let total = amount_num + prepare_response.fees_sat;

    details = details.push(
        container(
            Row::new()
                .spacing(10)
                .push(ui_text::text("Total:").size(16))
                .push(
                    ui_text::text(format!("{} sats", total))
                        .size(16)
                        .style(color::GREEN),
                ),
        )
        .padding(iced::Padding::new(10.0).top(10.0)),
    );

    col = col.push(
        container(details)
            .width(Length::Fill)
            .style(theme::Container::Card(theme::Card::Simple)),
    );

    // Question
    col = col.push(
        ui_text::text("Are you sure you want to send this payment?")
            .size(14)
            .horizontal_alignment(iced::alignment::Horizontal::Center)
            .style(color::GREY_3),
    );

    // Buttons
    let buttons = Row::new()
        .spacing(15)
        .push(
            ui_button::secondary(None, "Cancel")
                .on_press(ViewMessage::Activate(ActivateMessage::CancelPayment))
                .width(Length::Fill),
        )
        .push(
            ui_button::primary(None, "Confirm & Send")
                .on_press(ViewMessage::Activate(ActivateMessage::ConfirmPayment))
                .width(Length::Fill),
        );

    col = col.push(buttons);

    // Modal container (overlay style)
    container(
        container(col)
            .width(Length::Shrink)
            .style(theme::Container::Card(theme::Card::Simple)),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(theme::Container::Bordered)
    .into()
}
