//! Active (Breez SDK) view module

mod panel;
mod history;

pub use panel::ActivePanel;
#[cfg(feature = "breez")]
pub use panel::LightningWalletState;
pub use history::PaymentFilter;

/// Active sub-panel in Active view
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ActiveSubPanel {
    #[default]
    Main,
    Send,
    Receive,
    History,
    Settings,
}
