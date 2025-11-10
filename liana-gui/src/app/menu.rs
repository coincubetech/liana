use liana::miniscript::bitcoin::{OutPoint, Txid};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Menu {
    Home,
    Receive,
    PSBTs,
    Transactions,
    TransactionPreSelected(Txid),
    Settings,
    SettingsPreSelected(SettingsOption),
    Coins,
    CreateSpendTx,
    Recovery,
    RefreshCoins(Vec<OutPoint>),
    PsbtPreSelected(Txid),
    #[cfg(feature = "buysell")]
    BuySell, //(Option<AccountInfo>),
    #[cfg(feature = "breez")]
    Activate(ActivateMenu),
}

/// Pre-selectable settings options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsOption {
    Node,
}

/// Activate sub-menu options for Lightning/Liquid payments.
#[cfg(feature = "breez")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivateMenu {
    Main,
    Send,
    Receive,
    History,
}

