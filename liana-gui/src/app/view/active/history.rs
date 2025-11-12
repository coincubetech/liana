//! Transaction history view for Activate

use iced::{widget::container, Alignment, Length};
use iced::widget::scrollable;

use liana_ui::{
    color,
    component::{button as ui_button, text as ui_text},
    theme,
    widget::*,
};

use crate::app::view::{ActiveMessage, Message as ViewMessage};
use liana_ui::component::text::Text as TextTrait;

#[cfg(feature = "breez")]
use breez_sdk_liquid::prelude::{Payment, PaymentType};

/// Format Unix timestamp to human-readable date/time
#[allow(dead_code)]
fn format_timestamp(timestamp: Option<u64>) -> String {
    match timestamp {
        Some(ts) if ts > 0 => {
            // Convert Unix timestamp to readable format
            // Using chrono if available, otherwise basic formatting
            #[cfg(feature = "breez")]
            {
                use chrono::{DateTime, Local, TimeZone, Utc};
                
                if let Some(datetime) = Utc.timestamp_opt(ts as i64, 0).single() {
                    let local: DateTime<Local> = datetime.into();
                    local.format("%Y-%m-%d %H:%M:%S").to_string()
                } else {
                    format!("{}", ts)
                }
            }
            
            #[cfg(not(feature = "breez"))]
            format!("{}", ts)
        }
        _ => "Unknown".to_string(),
    }
}

/// Payment filter options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PaymentFilter {
    All,
    Sent,
    Received,
    Pending,
    Failed,
}

impl Default for PaymentFilter {
    fn default() -> Self {
        Self::All
    }
}

impl std::fmt::Display for PaymentFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "All"),
            Self::Sent => write!(f, "Sent"),
            Self::Received => write!(f, "Received"),
            Self::Pending => write!(f, "Pending"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

pub fn view_history<'a>(
    #[cfg(feature = "breez")] payments: &'a [Payment],
    #[cfg(not(feature = "breez"))] _payments: &'a [()],
    filter: PaymentFilter,
) -> Element<'a, ViewMessage> {
    let mut col = Column::new()
        .spacing(20)
        .padding(20)
        .width(Length::Fill);

    // Title
    col = col.push(
        Row::new()
            .spacing(10)
            .align_y(Alignment::Center)
            .push(ui_text::h2("Transaction History"))
            .push(
                ui_button::secondary(None, "Refresh")
                    .on_press(ViewMessage::Active(ActiveMessage::RefreshHistory))
                    .width(Length::Shrink)
            )
    );

    // Filter buttons
    let filter_row = Row::new()
        .spacing(10)
        .push(filter_button(PaymentFilter::All, filter))
        .push(filter_button(PaymentFilter::Sent, filter))
        .push(filter_button(PaymentFilter::Received, filter))
        .push(filter_button(PaymentFilter::Pending, filter))
        .push(filter_button(PaymentFilter::Failed, filter));

    col = col.push(filter_row);

    // Payment list
    #[cfg(feature = "breez")]
    {
        let filtered_payments: Vec<&Payment> = payments
            .iter()
            .filter(|p| matches_filter(p, filter))
            .collect();

        if filtered_payments.is_empty() {
            col = col.push(
                container(
                    ui_text::text("No transactions found")
                        .width(Length::Fill)
                        .style(|_| iced::widget::text::Style { color: Some(color::GREY_3) }),
                )
                .padding(40)
                .width(Length::Fill),
            );
        } else {
            for payment in filtered_payments {
                col = col.push(payment_item(payment));
            }
        }
    }

    #[cfg(not(feature = "breez"))]
    {
        col = col.push(
            container(
                ui_text::text("Breeze feature not enabled")
                    .width(Length::Fill)
                    .horizontal_alignment(iced::alignment::Horizontal::Center)
                    .style(|_| iced::widget::text::Style { color: Some(color::RED) }),
            )
            .padding(40),
        );
    }

    container(
        scrollable(col)
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[allow(dead_code)]
fn filter_button(filter: PaymentFilter, current: PaymentFilter) -> Element<'static, ViewMessage> {
    let label: &'static str = match filter {
        PaymentFilter::All => "All",
        PaymentFilter::Sent => "Sent",
        PaymentFilter::Received => "Received",
        PaymentFilter::Pending => "Pending",
        PaymentFilter::Failed => "Failed",
    };
    
    if filter == current {
        ui_button::primary(None, label)
            .on_press(ViewMessage::Active(ActiveMessage::FilterChanged(format!("{:?}", filter))))
            .width(Length::Shrink)
            .into()
    } else {
        ui_button::secondary(None, label)
            .on_press(ViewMessage::Active(ActiveMessage::FilterChanged(format!("{:?}", filter))))
            .width(Length::Shrink)
            .into()
    }
}

#[cfg(feature = "breez")]
#[allow(dead_code)]
fn matches_filter(payment: &Payment, filter: PaymentFilter) -> bool {
    use breez_sdk_liquid::prelude::PaymentState;
    
    match filter {
        PaymentFilter::All => true,
        PaymentFilter::Sent => matches!(payment.payment_type, PaymentType::Send),
        PaymentFilter::Received => matches!(payment.payment_type, PaymentType::Receive),
        PaymentFilter::Pending => matches!(payment.status, PaymentState::Pending),
        PaymentFilter::Failed => matches!(payment.status, PaymentState::Failed),
    }
}

#[cfg(feature = "breez")]
fn payment_item<'a>(payment: &'a Payment) -> Element<'a, ViewMessage> {
    use breez_sdk_liquid::prelude::PaymentState;
    
    let is_send = matches!(payment.payment_type, PaymentType::Send);
    let icon_text = if is_send { "↗" } else { "↙" };
    let icon_color = if is_send { color::ORANGE } else { color::GREEN };
    
    let status_text = match payment.status {
        PaymentState::Pending => "Pending",
        PaymentState::Complete => "Complete",
        PaymentState::Failed => "Failed",
        _ => "Unknown",
    };
    
    let status_color = match payment.status {
        PaymentState::Complete => color::GREEN,
        PaymentState::Pending => color::ORANGE,
        PaymentState::Failed => color::RED,
        _ => color::GREY_3,
    };

    container(
        Row::new()
            .spacing(15)
            .align_y(Alignment::Center)
            .push(
                ui_text::text(icon_text)
                    .size(24)
                    .style(move |_theme| iced::widget::text::Style { color: Some(icon_color) })
            )
            .push(
                Column::new()
                    .spacing(5)
                    .push(
                        Row::new()
                            .spacing(10)
                            .push(ui_text::text(format!("{} sats", payment.amount_sat)))
                            .push(TextTrait::small(ui_text::text(status_text)).style(move |_theme| iced::widget::text::Style { color: Some(status_color) }))
                    )
                    .push(
                        TextTrait::small(ui_text::text(
                            "Lightning payment" // Payment.details is PaymentDetails enum, not Option<String>
                        ))
                        .style(|_theme| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
                    .push(
                        ui_text::text(format_timestamp(Some(payment.timestamp as u64)))
                        .size(10)
                        .style(|_theme| iced::widget::text::Style { color: Some(color::GREY_3) })
                    )
                    .width(Length::Fill)
            )
    )
    .padding(15)
    .width(Length::Fill)
    .style(theme::card::simple)
    .into()
}
