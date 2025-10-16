use crate::{
    app::{menu::Menu, view::FiatAmountConverter},
    export::ImportExportMessage,
    node::bitcoind::RpcAuthType,
    services::fiat::{Currency, PriceSource},
};
use liana::miniscript::bitcoin::{bip32::Fingerprint, Address, OutPoint};

#[cfg(feature = "buysell")]
use crate::app::state::buysell::{LabelledAddress, PanelState};

pub trait Close {
    fn close() -> Self;
}

#[derive(Debug, Clone)]
pub enum Message {
    Scroll(f32),
    Reload,
    Clipboard(String),
    Menu(Menu),
    Close,
    Select(usize),
    SelectPayment(OutPoint),
    Label(Vec<String>, LabelMessage),
    NextReceiveAddress,
    ToggleShowPreviousAddresses,
    SelectAddress(Address),
    Settings(SettingsMessage),
    CreateSpend(CreateSpendMessage),
    ImportSpend(ImportSpendMessage),
    #[cfg(feature = "buysell")]
    BuySell(BuySellMessage),
    Spend(SpendTxMessage),
    Next,
    Previous,
    SelectHardwareWallet(usize),
    CreateRbf(CreateRbfMessage),
    ShowQrCode(usize),
    ImportExport(ImportExportMessage),
    HideRescanWarning,
    ExportPsbt,
    ImportPsbt,
    OpenUrl(String),
}

impl Close for Message {
    fn close() -> Self {
        Self::Close
    }
}

#[derive(Debug, Clone)]
pub enum LabelMessage {
    Edited(String),
    Cancel,
    Confirm,
}

#[derive(Debug, Clone)]
pub enum CreateSpendMessage {
    AddRecipient,
    BatchLabelEdited(String),
    DeleteRecipient(usize),
    SelectCoin(usize),
    RecipientEdited(usize, &'static str, String),
    RecipientFiatAmountEdited(usize, String, FiatAmountConverter),
    FeerateEdited(String),
    SelectPath(usize),
    Generate,
    SendMaxToRecipient(usize),
    Clear,
}

#[derive(Debug, Clone)]
pub enum ImportSpendMessage {
    Import,
    PsbtEdited(String),
    Confirm,
}

#[derive(Debug, Clone)]
pub enum SpendTxMessage {
    Delete,
    Sign,
    Broadcast,
    Save,
    Confirm,
    Cancel,
    SelectHotSigner,
    EditPsbt,
    PsbtEdited(String),
    Next,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    EditBitcoindSettings,
    BitcoindSettings(SettingsEditMessage),
    ElectrumSettings(SettingsEditMessage),
    RescanSettings(SettingsEditMessage),
    ImportExport(ImportExportMessage),
    EditRemoteBackendSettings,
    RemoteBackendSettings(RemoteBackendSettingsMessage),
    EditWalletSettings,
    ImportExportSection,
    ExportEncryptedDescriptor,
    ExportTransactions,
    ExportLabels,
    ExportWallet,
    ImportWallet,
    AboutSection,
    RegisterWallet,
    FingerprintAliasEdited(Fingerprint, String),
    WalletAliasEdited(String),
    Save,
    GeneralSection,
    Fiat(FiatMessage),
}

#[derive(Debug, Clone)]
pub enum RemoteBackendSettingsMessage {
    EditInvitationEmail(String),
    SendInvitation,
}

#[derive(Debug, Clone)]
pub enum SettingsEditMessage {
    Select,
    FieldEdited(&'static str, String),
    ValidateDomainEdited(bool),
    BitcoindRpcAuthTypeSelected(RpcAuthType),
    Cancel,
    Confirm,
    Clipboard(String),
}
#[cfg(feature = "buysell")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Individual,
    Business,
}

#[derive(Debug, Clone)]
pub enum CreateRbfMessage {
    New(bool),
    FeerateEdited(String),
    Cancel,
    Confirm,
}

#[cfg(feature = "buysell")]
#[derive(Debug, Clone)]
pub enum BuySellMessage {
    // Native login (default build)
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    LoginUsernameChanged(String),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    LoginPasswordChanged(String),

    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    SubmitLogin,
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    CreateAccountPressed,

    // Default build: account type selection
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    AccountTypeSelected(AccountType),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    GetStarted,

    // Default build: registration form (native flow)
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    FirstNameChanged(String),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    LastNameChanged(String),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    EmailChanged(String),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    Password1Changed(String),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    Password2Changed(String),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    TermsToggled(bool),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    SubmitRegistration,
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    CheckEmailVerificationStatus,
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    ResendVerificationEmail,
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    RegistrationSuccess,
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    RegistrationError(String),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    EmailVerificationStatusChecked(bool),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    EmailVerificationStatusError(String),
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    ResendEmailSuccess,
    #[cfg(all(feature = "buysell", not(feature = "webview")))]
    ResendEmailError(String),

    // Shared form fields (for provider-integrated builds)
    ResetWidget,
    SetPanelState(PanelState),
    CreateSession,
    SessionError(String),
    CreateNewAddress,
    AddressCreated(LabelledAddress),

    // webview messages (gated)
    #[cfg(feature = "webview")]
    WebviewCreated(iced_webview::ViewId),
    #[cfg(feature = "webview")]
    ViewTick(iced_webview::ViewId),
    #[cfg(feature = "webview")]
    WebviewAction(iced_webview::advanced::Action),
    #[cfg(feature = "webview")]
    WebviewOpenUrl(String),
}

#[derive(Debug, Clone)]
pub enum FiatMessage {
    Enable(bool),
    SourceEdited(PriceSource),
    CurrencyEdited(Currency),
}

impl From<FiatMessage> for Message {
    fn from(msg: FiatMessage) -> Self {
        Message::Settings(SettingsMessage::Fiat(msg))
    }
}
