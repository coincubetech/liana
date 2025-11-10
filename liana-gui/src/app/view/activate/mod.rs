//! Activate (Breez SDK) view module

mod panel;
mod history;

pub use panel::ActivatePanel;
#[cfg(feature = "breez")]
pub use panel::LightningWalletState;
pub use history::PaymentFilter;

/// Active sub-panel in Activate view
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ActivateSubPanel {
    #[default]
    Main,
    Send,
    Receive,
    History,
    Settings,
}
