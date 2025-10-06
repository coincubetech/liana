use iced::Task;
use liana::miniscript::bitcoin;
use std::sync::Arc;

#[cfg(feature = "webview")]
use iced_webview::{
    advanced::{Action as WebviewAction, WebView},
    PageType,
};
use liana_ui::widget::Element;

#[cfg(feature = "dev-meld")]
use crate::app::buysell::{meld::MeldError, ServiceProvider};

#[cfg(feature = "dev-onramp")]
use crate::app::buysell::onramper;

#[cfg(not(feature = "webview"))]
use crate::app::view::buysell::NativePage;

use crate::{
    app::{
        self,
        cache::Cache,
        message::Message,
        state::{receive::Modal, State},
        view::{self, BuySellMessage, Message as ViewMessage},
    },
    daemon::Daemon,
};

#[cfg(feature = "webview")]
#[derive(Debug, Clone)]
pub enum WebviewMessage {
    Action(iced_webview::advanced::Action),
    Created(iced_webview::ViewId),
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

/// Map webview messages to main app messages (static version for Task::map)
#[cfg(feature = "webview")]
fn map_webview_message_static(webview_msg: WebviewMessage) -> Message {
    match webview_msg {
        WebviewMessage::Action(action) => {
            Message::View(ViewMessage::BuySell(BuySellMessage::WebviewAction(action)))
        }
        WebviewMessage::Created(id) => {
            Message::View(ViewMessage::BuySell(BuySellMessage::WebviewCreated(id)))
        }
    }
}

/// lazily initialize the webview to reduce latent memory usage
#[cfg(feature = "webview")]
fn init_webview() -> WebView<iced_webview::Ultralight, WebviewMessage> {
    WebView::new().on_create_view(crate::app::state::buysell::WebviewMessage::Created)
}

pub struct BuySellPanel {
    // TODO: Detect country and currency using ip-api.com, with drop-down for manual selection
    pub error: Option<String>,
    pub network: bitcoin::Network,

    pub wallet: Arc<app::wallet::Wallet>,
    pub data_dir: crate::dir::LianaDirectory,
    pub modal: Modal,

    #[cfg(feature = "dev-meld")]
    pub meld_client: MeldClient,

    #[cfg(any(feature = "dev-onramp", feature = "dev-meld"))]
    pub addresses: Vec<LabelledAddress>,

    #[cfg(any(feature = "dev-onramp", feature = "dev-meld"))]
    pub picked_address: Option<usize>,

    // Ultralight webview component for Meld widget integration with performance optimizations
    #[cfg(feature = "webview")]
    pub webview: Option<WebView<iced_webview::Ultralight, WebviewMessage>>,

    // Current webview page url
    #[cfg(feature = "webview")]
    pub session_url: Option<String>,

    // Current active webview "page": view_id
    #[cfg(feature = "webview")]
    pub active_page: Option<iced_webview::ViewId>,

    // Native buysell
    #[cfg(not(feature = "webview"))]
    pub registration_state: crate::services::registration::RegistrationState,
}

impl BuySellPanel {
    pub fn new(
        network: bitcoin::Network,
        wallet: Arc<app::wallet::Wallet>,
        data_dir: crate::dir::LianaDirectory,
    ) -> Self {
        Self {
            error: None,
            network,

            wallet,
            data_dir,
            modal: Modal::None,

            #[cfg(feature = "dev-meld")]
            meld_client: MeldClient::new(),

            #[cfg(any(feature = "dev-onramp", feature = "dev-meld"))]
            addresses: Vec::new(),

            #[cfg(any(feature = "dev-onramp", feature = "dev-meld"))]
            picked_address: None,

            #[cfg(feature = "webview")]
            webview: None,
            #[cfg(feature = "webview")]
            session_url: None,
            #[cfg(feature = "webview")]
            active_page: None,

            #[cfg(not(feature = "webview"))]
            registration_state: Default::default(),
        }
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    #[cfg(not(feature = "webview"))]
    pub fn set_login_username(&mut self, v: String) {
        self.registration_state.login_username.value = v;
        self.registration_state.login_username.valid =
            !self.registration_state.login_username.value.is_empty();
    }

    #[cfg(not(feature = "webview"))]
    pub fn set_login_password(&mut self, v: String) {
        self.registration_state.login_password.value = v;
        self.registration_state.login_password.valid =
            !self.registration_state.login_password.value.is_empty();
    }

    #[cfg(not(feature = "webview"))]
    pub fn is_login_form_valid(&self) -> bool {
        self.registration_state.login_username.valid && self.registration_state.login_password.valid
    }
}

impl State for BuySellPanel {
    fn reload(
        &mut self,
        daemon: Arc<dyn Daemon + Sync + Send>,
        _wallet: Arc<app::wallet::Wallet>,
    ) -> Task<Message> {
        Task::perform(
            async move { daemon.list_revealed_addresses(false, true, 50, None).await },
            |res| match res {
                Ok(out) => {
                    let addresses = out
                        .addresses
                        .into_iter()
                        .map(|a| LabelledAddress {
                            address: a.address,
                            index: a.index,
                            label: a.label,
                        })
                        .collect();

                    Message::View(ViewMessage::BuySell(BuySellMessage::LoadedAddresses(
                        addresses,
                    )))
                }
                Err(err) => Message::View(ViewMessage::BuySell(BuySellMessage::SessionError(
                    err.to_string(),
                ))),
            },
        )
    }

    fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, ViewMessage> {
        // Return the meld view directly - dashboard wrapper will be applied by app/mod.rs
        view::dashboard(&app::Menu::BuySell, cache, None, self.view())
    }

    fn update(
        &mut self,
        daemon: Arc<dyn Daemon + Sync + Send>,
        cache: &Cache,
        message: Message,
    ) -> Task<Message> {
        let message = match message {
            // Handle global navigation for native flow (Previous)
            #[cfg(not(feature = "webview"))]
            Message::View(ViewMessage::Previous) => {
                match self.registration_state.native_page {
                    NativePage::Register => {
                        self.registration_state.native_page = NativePage::AccountSelect
                    }
                    NativePage::VerifyEmail => {
                        self.registration_state.native_page = NativePage::Register
                    }
                    _ => {}
                }

                return Task::none();
            }
            Message::View(ViewMessage::Select(index)) => {
                let Some(la) = self.addresses.get(index) else {
                    return Task::none();
                };

                self.modal = Modal::VerifyAddress(super::receive::VerifyAddressModal::new(
                    self.data_dir.clone(),
                    self.wallet.clone(),
                    cache.network,
                    la.address.clone(),
                    la.index,
                ));

                return Task::none();
            }
            Message::View(ViewMessage::ShowQrCode(index)) => {
                let Some(la) = self.addresses.get(index) else {
                    return Task::none();
                };

                if let Some(modal) = super::receive::ShowQrCodeModal::new(&la.address, la.index) {
                    self.modal = Modal::ShowQrCode(modal);
                }

                return Task::none();
            }
            Message::View(ViewMessage::Next) => {
                log::info!("[BUYSELL] Loaded...");
                return Task::none();
            }
            Message::View(ViewMessage::Close) => {
                self.modal = Modal::None;
                return Task::none();
            }
            Message::View(ViewMessage::BuySell(message)) => message,
            _ => return Task::none(),
        };

        match message {
            #[cfg(not(feature = "webview"))]
            BuySellMessage::LoginUsernameChanged(v) => {
                self.set_login_username(v);
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::LoginPasswordChanged(v) => {
                self.set_login_password(v);
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::SubmitLogin => {
                if self.is_login_form_valid() {
                    self.error = None;
                } else {
                    self.set_error("Please enter username and password".into());
                }

                return Task::none();
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::CreateAccountPressed => {
                self.set_error("Create Account not implemented yet".to_string());
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::AccountTypeSelected(t) => {
                self.registration_state.selected_account_type = Some(t);
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::GetStarted => {
                if self.registration_state.selected_account_type.is_some() {
                    // Navigate to registration page (native flow)
                    self.registration_state.native_page = NativePage::Register;
                }
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::FirstNameChanged(v) => {
                self.registration_state.first_name.value = v;
                self.registration_state.first_name.valid =
                    !self.registration_state.first_name.value.is_empty();
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::LastNameChanged(v) => {
                self.registration_state.last_name.value = v;
                self.registration_state.last_name.valid =
                    !self.registration_state.last_name.value.is_empty();
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::EmailChanged(v) => {
                self.registration_state.email.value = v;
                self.registration_state.email.valid =
                    self.registration_state.email.value.contains('@')
                        && self.registration_state.email.value.contains('.')
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::Password1Changed(v) => {
                self.registration_state.password1.value = v;
                self.registration_state.password1.valid = self.is_password_valid();
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::Password2Changed(v) => {
                self.registration_state.password2.value = v;
                self.registration_state.password2.valid = self.registration_state.password2.value
                    == self.registration_state.password1.value
                    && !self.registration_state.password2.value.is_empty();
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::TermsToggled(b) => {
                self.registration_state.terms_accepted = b;
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::SubmitRegistration => {
                tracing::info!("🔍 [REGISTRATION] Submit registration button clicked");

                if self.is_registration_valid() {
                    tracing::info!(
                        "✅ [REGISTRATION] Form validation passed, submitting registration"
                    );
                    let client = self.registration_state.client.clone();
                    let account_type = if self.registration_state.selected_account_type
                        == Some(crate::app::view::AccountType::Individual)
                    {
                        "personal"
                    } else {
                        "business"
                    }
                    .to_string();

                    let email = self.registration_state.email.value.clone();
                    let first_name = self.registration_state.first_name.value.clone();
                    let last_name = self.registration_state.last_name.value.clone();
                    let password = self.registration_state.password1.value.clone();

                    tracing::info!(
                        "📤 [REGISTRATION] Making API call with account_type: {}, email: {}",
                        account_type,
                        email
                    );

                    return Task::perform(
                        async move {
                            let request = crate::services::registration::SignUpRequest {
                                account_type,
                                email,
                                first_name,
                                last_name,
                                auth_details: vec![crate::services::registration::AuthDetail {
                                    provider: 1, // EmailProvider = 1
                                    password,
                                }],
                            };

                            tracing::info!("🚀 [REGISTRATION] Sending request to API");
                            let result = client.sign_up(request).await;
                            tracing::info!(
                                "📥 [REGISTRATION] API response received: {:?}",
                                result.is_ok()
                            );
                            result
                        },
                        |result| match result {
                            Ok(_response) => {
                                tracing::info!("🎉 [REGISTRATION] Registration successful!");
                                // Registration successful, navigate to email verification
                                Message::View(ViewMessage::BuySell(
                                    BuySellMessage::RegistrationSuccess,
                                ))
                            }
                            Err(error) => {
                                tracing::error!(
                                    "❌ [REGISTRATION] Registration failed: {}",
                                    error.error
                                );
                                // Registration failed, show error
                                Message::View(ViewMessage::BuySell(
                                    BuySellMessage::RegistrationError(error.error),
                                ))
                            }
                        },
                    );
                } else {
                    tracing::warn!(
                        "⚠️ [REGISTRATION] Form validation failed - button should be disabled"
                    );
                    tracing::warn!(
                        "   - First name: '{}' (valid: {})",
                        self.registration_state.first_name.value,
                        !self.registration_state.first_name.value.is_empty()
                    );
                    tracing::warn!(
                        "   - Last name: '{}' (valid: {})",
                        self.registration_state.last_name.value,
                        !self.registration_state.last_name.value.is_empty()
                    );
                    tracing::warn!(
                        "   - Email: '{}' (valid: {})",
                        self.registration_state.email.value,
                        self.registration_state.email.value.contains('@')
                            && self.registration_state.email.value.contains('.')
                    );
                    tracing::warn!(
                        "   - Password length: {} (valid: {})",
                        self.registration_state.password1.value.len(),
                        self.registration_state.password1.value.len() >= 8
                    );
                    tracing::warn!(
                        "   - Passwords match: {}",
                        self.registration_state.password1.value
                            == self.registration_state.password2.value
                    );
                    tracing::warn!(
                        "   - Terms accepted: {}",
                        self.registration_state.terms_accepted
                    );
                }
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::RegistrationSuccess => {
                // Registration successful, navigate to email verification
                self.registration_state.native_page = NativePage::VerifyEmail;
                self.registration_state.email_verification_status = Some(false); // pending verification
                self.error = None;
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::RegistrationError(error) => {
                self.error = Some(format!("Registration failed: {}", error));
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::CheckEmailVerificationStatus => {
                tracing::info!(
                    "🔍 [EMAIL_VERIFICATION] Checking email verification status for: {}",
                    self.registration_state.email.value
                );
                // Set to "checking" state
                self.registration_state.email_verification_status = None;
                let client = self.registration_state.client.clone();
                let email = self.registration_state.email.value.clone();

                return Task::perform(
                    async move {
                        tracing::info!("🚀 [EMAIL_VERIFICATION] Making API call to check status");
                        let result = client.check_email_verification_status(&email).await;
                        tracing::info!(
                            "📥 [EMAIL_VERIFICATION] API response received: {:?}",
                            result.is_ok()
                        );
                        result
                    },
                    |result| match result {
                        Ok(response) => {
                            tracing::info!(
                                "✅ [EMAIL_VERIFICATION] Status check successful: verified={}",
                                response.email_verified
                            );
                            Message::View(ViewMessage::BuySell(
                                BuySellMessage::EmailVerificationStatusChecked(
                                    response.email_verified,
                                ),
                            ))
                        }
                        Err(error) => {
                            tracing::error!(
                                "❌ [EMAIL_VERIFICATION] Status check failed: {}",
                                error.error
                            );
                            Message::View(ViewMessage::BuySell(
                                BuySellMessage::EmailVerificationStatusError(error.error),
                            ))
                        }
                    },
                );
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::EmailVerificationStatusChecked(verified) => {
                self.registration_state.email_verification_status = Some(verified);
                if verified {
                    self.error = Some("Email verified successfully!".to_string());
                } else {
                    self.error = None;
                }
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::EmailVerificationStatusError(error) => {
                self.registration_state.email_verification_status = Some(false); // fallback to pending
                self.error = Some(format!("Error checking verification status: {}", error));
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::ResendVerificationEmail => {
                tracing::info!(
                    "📧 [RESEND_EMAIL] Resending verification email to: {}",
                    self.registration_state.email.value
                );
                let client = self.registration_state.client.clone();
                let email = self.registration_state.email.value.clone();

                return Task::perform(
                    async move {
                        tracing::info!("🚀 [RESEND_EMAIL] Making API call to resend email");
                        let result = client.resend_verification_email(&email).await;
                        tracing::info!(
                            "📥 [RESEND_EMAIL] API response received: {:?}",
                            result.is_ok()
                        );
                        result
                    },
                    |result| match result {
                        Ok(_response) => {
                            tracing::info!("✅ [RESEND_EMAIL] Email resent successfully");
                            Message::View(ViewMessage::BuySell(BuySellMessage::ResendEmailSuccess))
                        }
                        Err(error) => {
                            tracing::error!(
                                "❌ [RESEND_EMAIL] Failed to resend email: {}",
                                error.error
                            );
                            Message::View(ViewMessage::BuySell(BuySellMessage::ResendEmailError(
                                error.error,
                            )))
                        }
                    },
                );
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::ResendEmailSuccess => {
                self.registration_state.email_verification_status = Some(false); // back to pending
                self.error = Some("Verification email resent successfully!".to_string());
            }
            #[cfg(not(feature = "webview"))]
            BuySellMessage::ResendEmailError(error) => {
                self.error = Some(format!("Error resending email: {}", error));
            }

            BuySellMessage::CreateNewAddress => {
                return Task::perform(
                    async move { daemon.get_new_address().await },
                    |res| match res {
                        Ok(out) => Message::View(ViewMessage::BuySell(
                            BuySellMessage::PickedAddress(LabelledAddress {
                                address: out.address,
                                index: out.derivation_index,
                                label: Some("new.buysell".to_string()),
                            }),
                        )),
                        Err(err) => Message::View(ViewMessage::BuySell(
                            BuySellMessage::SessionError(err.to_string()),
                        )),
                    },
                )
            }
            BuySellMessage::LoadedAddresses(addresses) => self.addresses = addresses,
            BuySellMessage::PickedAddress(la) => {
                let find = self
                    .addresses
                    .iter()
                    .enumerate()
                    .find(|(.., addr)| *addr == &la);

                match find {
                    Some((index, ..)) => self.picked_address = Some(index),
                    None => {
                        self.picked_address = Some(self.addresses.len());
                        self.addresses.push(la.clone());
                    }
                }
            }
            BuySellMessage::ClearCurrentAddress => {
                self.picked_address = None;

                if let Some(page) = self.active_page.take() {
                    return self
                        .webview
                        .get_or_insert_with(init_webview)
                        .update(WebviewAction::CloseView(page))
                        .map(map_webview_message_static);
                };
            }

            #[cfg(feature = "dev-onramp")]
            BuySellMessage::CreateSession => {
                let Some(idx) = &self.picked_address else {
                    return Task::none();
                };

                // TODO: infer currency from user ip
                let LabelledAddress { address, .. } = &self.addresses[*idx];
                let fiat_currency = "USD";

                let Some(onramper_url) =
                    onramper::create_widget_url(&fiat_currency, &address.to_string())
                else {
                    self.error = Some("Onramper API key not set as an environment variable (ONRAMPER_API_KEY) at compile time".to_string());
                    return Task::none();
                };

                tracing::info!(
                    "🚀 [BUYSELL] Creating new onramper widget session: {}",
                    &onramper_url
                );

                let open_webview = Message::View(ViewMessage::BuySell(
                    BuySellMessage::WebviewOpenUrl(onramper_url),
                ));

                return Task::done(open_webview);
            }

            #[cfg(feature = "dev-meld")]
            BuySellMessage::CreateSession => {
                let Some(idx) = &self.picked_address else {
                    return Task::none();
                };

                tracing::info!(
                    "🚀 [BUYSELL] Creating new session - clearing any existing session data"
                );

                // init session
                let LabelledAddress { address, .. } = &self.addresses[*idx];
                let wallet_address = address.to_string();

                // TODO: user should set this within webview
                let country_code = "USD";
                let source_amount = "60";

                tracing::info!(
                    "🚀 [BUYSELL] Making fresh API call with: address={}, country={}, amount={}",
                    wallet_address,
                    country_code,
                    source_amount
                );

                return Task::perform(
                    {
                        // TODO: allow users to select source provider, in a drop down
                        let provider = ServiceProvider::Transak;
                        let network = self.network;
                        let client = self.meld_client.clone();

                        async move {
                            match client
                                .create_widget_session(
                                    wallet_address.as_str(),
                                    country_code,
                                    source_amount,
                                    provider,
                                    network,
                                )
                                .await
                            {
                                Ok(url) => Ok(url),
                                Err(MeldError::Network(e)) => Err(format!("Network error: {}", e)),
                                Err(MeldError::Serialization(e)) => {
                                    Err(format!("Data error: {}", e))
                                }
                                Err(MeldError::Api(e)) => Err(format!("API error: {}", e)),
                            }
                        }
                    },
                    |result| match result {
                        Ok(widget_url) => {
                            tracing::info!(
                                "🌐 [BUYSELL] Meld session created with URL: {}",
                                widget_url
                            );

                            Message::View(ViewMessage::BuySell(BuySellMessage::WebviewOpenUrl(
                                widget_url,
                            )))
                        }
                        Err(error) => {
                            tracing::error!("❌ [MELD] Session creation failed: {}", error);
                            Message::View(ViewMessage::BuySell(BuySellMessage::SessionError(error)))
                        }
                    },
                );
            }

            BuySellMessage::SessionError(error) => {
                self.set_error(error);
            }

            // webview logic
            #[cfg(feature = "webview")]
            BuySellMessage::ViewTick(id) => {
                return self
                    .webview
                    .get_or_insert_with(init_webview)
                    .update(WebviewAction::Update(id))
                    .map(map_webview_message_static);
            }
            #[cfg(feature = "webview")]
            BuySellMessage::WebviewAction(action) => {
                return self
                    .webview
                    .get_or_insert_with(init_webview)
                    .update(action)
                    .map(map_webview_message_static);
            }
            #[cfg(feature = "webview")]
            BuySellMessage::WebviewOpenUrl(url) => {
                // Load URL into Ultralight webview
                tracing::info!("🌐 [BUYSELL] Loading Ultralight webview with URL: {}", url);
                self.session_url = Some(url.clone());

                // Create webview with URL string and immediately update to ensure content loads
                return self
                    .webview
                    .get_or_insert_with(init_webview)
                    .update(WebviewAction::CreateView(PageType::Url(url)))
                    .map(map_webview_message_static);
            }
            #[cfg(feature = "webview")]
            BuySellMessage::WebviewCreated(id) => {
                tracing::info!("🌐 [BUYSELL] Activating Webview Page: {}", id);

                // set active page to selected view id
                let og = self.active_page.take();
                self.active_page = Some(id);

                if let Some(id) = og {
                    let webview = self.webview.get_or_insert_with(init_webview);
                    return webview
                        .update(WebviewAction::CloseView(id))
                        .map(map_webview_message_static);
                }
            }
        };

        Task::none()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        #[cfg(feature = "webview")]
        {
            if let Some(id) = self.active_page {
                let interval = if cfg!(debug_assertions) {
                    std::time::Duration::from_millis(250)
                } else {
                    std::time::Duration::from_millis(100)
                };

                return iced::time::every(interval).with(id).map(|(i, ..)| {
                    Message::View(ViewMessage::BuySell(BuySellMessage::ViewTick(i)))
                });
            }
        }

        iced::Subscription::none()
    }
}
