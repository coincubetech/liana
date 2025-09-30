use iced::Task;
use std::sync::Arc;

#[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
use crate::services::mavapay::{BankAccount, Beneficiary};

use iced_webview::{
    advanced::{Action as WebviewAction, WebView},
    PageType,
};
use liana_ui::widget::Element;

#[cfg(feature = "dev-meld")]
use crate::app::buysell::{meld::MeldError, ServiceProvider};

#[cfg(all(feature = "dev-onramp", not(feature = "dev-meld")))]
use crate::app::buysell::onramper;

#[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
use crate::app::view::buysell::NativePage;
use crate::app::buysell::{onramper, meld::MeldError, ServiceProvider};


use crate::{
    app::{
        self,
        cache::Cache,
        message::Message,
        state::State,
        view::{self, buysell::BuySellPanel, BuySellMessage, Message as ViewMessage},
    },
    daemon::Daemon,
};

#[derive(Debug, Clone)]
pub enum WebviewMessage {
    Action(iced_webview::advanced::Action),
    Created(iced_webview::ViewId),
}

/// Map webview messages to main app messages (static version for Task::map)
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
fn init_webview() -> WebView<iced_webview::Ultralight, WebviewMessage> {
    WebView::new().on_create_view(crate::app::state::buysell::WebviewMessage::Created)
}

impl Default for BuySellPanel {
    fn default() -> Self {
        Self::new(liana::miniscript::bitcoin::Network::Bitcoin)
    }
}

impl State for BuySellPanel {
    fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, ViewMessage> {
        // Return the meld view directly - dashboard wrapper will be applied by app/mod.rs
        view::dashboard(&app::Menu::BuySell, cache, None, self.view())
    }

        fn reload(
            &mut self,
            _daemon: Arc<dyn Daemon + Sync + Send>,
            _wallet: Arc<crate::app::wallet::Wallet>,
        ) -> Task<Message> {
            let locator = crate::services::geolocation::CachedGeoLocator::new_from_env();
            Task::perform(
                async move { locator.detect_region().await },
                |result| match result {
                    Ok((region, country)) => {
                        let region_str = match region {
                            crate::services::geolocation::Region::Africa => "africa".to_string(),
                            crate::services::geolocation::Region::International => {
                                "international".to_string()
                            }
                        };
                        Message::View(ViewMessage::BuySell(BuySellMessage::RegionDetected(
                            region_str,
                            country,
                        )))
                    }
                    Err(error) => Message::View(ViewMessage::BuySell(
                        BuySellMessage::RegionDetectionError(error),
                    )),
                },
            )
        }


    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _cache: &Cache,
        message: Message,
    ) -> Task<Message> {
        // Handle global navigation for native flow (Previous)
        if let Message::View(ViewMessage::Previous) = &message {
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            {
                match self.native_page {
                    NativePage::Register => self.native_page = NativePage::AccountSelect,
                    NativePage::VerifyEmail => self.native_page = NativePage::Register,
                    _ => {}
                }
            }
            return Task::none();
        }

        let Message::View(ViewMessage::BuySell(message)) = message else {
            return Task::none();
        };

        match message {
            BuySellMessage::LoginUsernameChanged(v) => {
                self.set_login_username(v);
            }
            BuySellMessage::LoginPasswordChanged(v) => {
                self.set_login_password(v);
            }
            BuySellMessage::SubmitLogin => {
                return self.handle_native_login();
            }
            BuySellMessage::CreateAccountPressed => {
                // Navigate to registration page
                #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
                {
                    self.native_page = NativePage::Register;
                }
                #[cfg(any(feature = "dev-meld", feature = "dev-onramp"))]

                {
                    self.set_error("Create Account not implemented yet".to_string());
                }
            }
            BuySellMessage::WalletAddressChanged(address) => {
                self.set_wallet_address(address);
            }
            #[cfg(feature = "dev-meld")]
            BuySellMessage::CountryCodeChanged(code) => {
                self.set_country_code(code);
            }
            #[cfg(feature = "dev-onramp")]
            BuySellMessage::FiatCurrencyChanged(fiat) => {
                self.set_fiat_currency(fiat);
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::AccountTypeSelected(t) => {
                self.selected_account_type = Some(t);
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::GetStarted => {
                if self.selected_account_type.is_none() {
                    // button disabled; ignore
                } else {
                    // Navigate to login page (native flow)
                    self.native_page = NativePage::Login;
                }
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::FirstNameChanged(v) => {
                self.first_name.value = v;
                self.first_name.valid = !self.first_name.value.is_empty();
            }
            BuySellMessage::DetectRegion => {
                // Detection is automatically triggered by reload(); nothing to do here
            }
            BuySellMessage::RegionDetected(region, country) => {
                // Do not log IP addresses. Region/country are fine.
                tracing::info!("region = {}, country = {}", region, country);
                self.detected_region = Some(region);
                self.detected_country = Some(country);
                self.error = None;
            }
            BuySellMessage::RegionDetectionError(_error) => {
                // Graceful fallback: show provider selection, no blocking error
                self.region_detection_failed = true;
                self.error = None;
            }

            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::LastNameChanged(v) => {
                self.last_name.value = v;
                self.last_name.valid = !self.last_name.value.is_empty();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::EmailChanged(v) => {
                self.email.value = v;
                self.email.valid = self.email.value.contains('@') && self.email.value.contains('.')
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::Password1Changed(v) => {
                self.password1.value = v;
                self.password1.valid = self.is_password_valid();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::Password2Changed(v) => {
                self.password2.value = v;
                self.password2.valid = self.password2.value == self.password1.value
                    && !self.password2.value.is_empty();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::TermsToggled(b) => {
                self.terms_accepted = b;
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::SubmitRegistration => {
                tracing::info!("🔍 [REGISTRATION] Submit registration button clicked");

                if self.is_registration_valid() {
                    tracing::info!(
                        "✅ [REGISTRATION] Form validation passed, submitting registration"
                    );
                    let client = self.registration_client.clone();
                    let account_type = if self.selected_account_type
                        == Some(crate::app::view::AccountType::Individual)
                    {
                        "personal"
                    } else {
                        "business"
                    }
                    .to_string();

                    let email = self.email.value.clone();
                    let first_name = self.first_name.value.clone();
                    let last_name = self.last_name.value.clone();
                    let password = self.password1.value.clone();

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
                        self.first_name.value,
                        !self.first_name.value.is_empty()
                    );
                    tracing::warn!(
                        "   - Last name: '{}' (valid: {})",
                        self.last_name.value,
                        !self.last_name.value.is_empty()
                    );
                    tracing::warn!(
                        "   - Email: '{}' (valid: {})",
                        self.email.value,
                        self.email.value.contains('@') && self.email.value.contains('.')
                    );
                    tracing::warn!(
                        "   - Password length: {} (valid: {})",
                        self.password1.value.len(),
                        self.password1.value.len() >= 8
                    );
                    tracing::warn!(
                        "   - Passwords match: {}",
                        self.password1.value == self.password2.value
                    );
                    tracing::warn!("   - Terms accepted: {}", self.terms_accepted);
                }
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::RegistrationSuccess => {
                // Registration successful, navigate to email verification
                self.native_page = NativePage::VerifyEmail;
                self.email_verification_status = Some(false); // pending verification
                self.error = None;
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::RegistrationError(error) => {
                self.error = Some(format!("Registration failed: {}", error));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::CheckEmailVerificationStatus => {
                tracing::info!(
                    "🔍 [EMAIL_VERIFICATION] Checking email verification status for: {}",
                    self.email.value
                );
                // Set to "checking" state
                self.email_verification_status = None;
                let client = self.registration_client.clone();
                let email = self.email.value.clone();

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
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::EmailVerificationStatusChecked(verified) => {
                self.email_verification_status = Some(verified);
                if verified {
                    tracing::info!(
                        "✅ [EMAIL_VERIFICATION] Email verified, navigating to Mavapay dashboard"
                    );
                    self.native_page = NativePage::CoincubePay;
                    self.error = None;
                    // Automatically get current price when entering dashboard
                    return Task::done(Message::View(ViewMessage::BuySell(
                        BuySellMessage::MavapayGetPrice,
                    )));
                } else {
                    self.error = None;
                }
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::EmailVerificationStatusError(error) => {
                self.email_verification_status = Some(false); // fallback to pending
                self.error = Some(format!("Error checking verification status: {}", error));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::ResendVerificationEmail => {
                tracing::info!(
                    "📧 [RESEND_EMAIL] Resending verification email to: {}",
                    self.email.value
                );
                let client = self.registration_client.clone();
                let email = self.email.value.clone();

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
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::ResendEmailSuccess => {
                self.email_verification_status = Some(false); // back to pending
                self.error = Some("Verification email resent successfully!".to_string());
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::ResendEmailError(error) => {
                self.error = Some(format!("Error resending email: {}", error));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::LoginSuccess(response) => {
                tracing::info!("✅ [LOGIN] Login successful, checking email verification status");
                self.error = None;

                // Check if 2FA is required
                if response.requires_two_factor {
                    tracing::info!("⚠️ [LOGIN] 2FA required but not implemented yet");
                    self.set_error(
                        "Two-factor authentication required but not yet supported.".to_string(),
                    );
                    return Task::none();
                }

                // Check if we have user data and token
                if let (Some(user), Some(_token)) = (&response.user, &response.token) {
                    // Check if email is verified and route accordingly
                    if user.email_verified {
                        tracing::info!(
                            "✅ [LOGIN] Email verified, navigating to Mavapay dashboard"
                        );
                        self.native_page = NativePage::CoincubePay;
                        // Automatically get current price when entering dashboard
                        return Task::done(Message::View(ViewMessage::BuySell(
                            BuySellMessage::MavapayGetPrice,
                        )));
                    } else {
                        tracing::info!(
                            "⚠️ [LOGIN] Email not verified, redirecting to verification page"
                        );
                        // Store the email for verification
                        self.email.value = user.email.clone();
                        self.native_page = NativePage::VerifyEmail;
                    }
                } else {
                    tracing::error!("❌ [LOGIN] Login response missing user data or token");
                    self.set_error("Login failed: Invalid response from server".to_string());
                }
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::LoginError(err) => {
                tracing::error!("❌ [LOGIN] Login failed: {}", err);
                self.set_error(format!("Login failed: {}", err));
            }

            BuySellMessage::SourceAmountChanged(amount) => {
                self.set_source_amount(amount);
            }

            // Mavapay message handlers
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::OpenOnramper => {
                // Build Onramper widget URL and open in embedded webview
                let currency = "USD".to_string();
                let amount = if self.source_amount.value.is_empty() {
                    "50".to_string()
                } else {
                    self.source_amount.value.clone()
                };
                let wallet = self.wallet_address.value.clone();
                if let Some(url) = onramper::create_widget_url(&currency, &amount, &wallet) {
                    return Task::done(Message::View(ViewMessage::BuySell(
                        BuySellMessage::WebviewOpenUrl(url),
                    )));
                } else {
                    self.set_error("Onramper API key not configured".to_string());
                }
            }
            BuySellMessage::OpenMeld => {
                // Create Meld widget session via API and open in embedded webview
                let wallet_address = self.wallet_address.value.clone();
                let country_code = self
                    .detected_country
                    .clone()
                    .unwrap_or_else(|| "US".to_string());
                let source_amount = if self.source_amount.value.is_empty() {
                    "50".to_string()
                } else {
                    self.source_amount.value.clone()
                };
                let network = self.network;
                let client = self.meld_client.clone();
                return Task::perform(
                    async move {
                        client
                            .create_widget_session(
                                wallet_address,
                                country_code,
                                source_amount,
                                ServiceProvider::Guardarian,
                                network,
                            )
                            .await
                            .map(|resp| resp.widget_url)
                            .map_err(|e| format!("{}", e))
                    },
                    |result| match result {
                        Ok(widget_url) => Message::View(ViewMessage::BuySell(
                            BuySellMessage::WebviewOpenUrl(widget_url),
                        )),
                        Err(error) => Message::View(ViewMessage::BuySell(
                            BuySellMessage::SessionError(error),
                        )),
                    },
                );
            }

            BuySellMessage::MavapayDashboard => {
                self.native_page = NativePage::CoincubePay;
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayAmountChanged(amount) => {
                self.mavapay_amount.value = amount;
                self.mavapay_amount.valid = !self.mavapay_amount.value.is_empty()
                    && self.mavapay_amount.value.parse::<u64>().is_ok();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapaySourceCurrencyChanged(currency) => {
                self.mavapay_source_currency.value = currency;
                self.mavapay_source_currency.valid = !self.mavapay_source_currency.value.is_empty();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayTargetCurrencyChanged(currency) => {
                self.mavapay_target_currency.value = currency;
                self.mavapay_target_currency.valid = !self.mavapay_target_currency.value.is_empty();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayBankAccountNumberChanged(account) => {
                self.mavapay_bank_account_number.value = account;
                self.mavapay_bank_account_number.valid =
                    !self.mavapay_bank_account_number.value.is_empty();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayBankAccountNameChanged(name) => {
                self.mavapay_bank_account_name.value = name;
                self.mavapay_bank_account_name.valid =
                    !self.mavapay_bank_account_name.value.is_empty();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayBankCodeChanged(code) => {
                self.mavapay_bank_code.value = code;
                self.mavapay_bank_code.valid = !self.mavapay_bank_code.value.is_empty();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayBankNameChanged(name) => {
                self.mavapay_bank_name.value = name;
                self.mavapay_bank_name.valid = !self.mavapay_bank_name.value.is_empty();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayCreateQuote => {
                return self.handle_mavapay_create_quote();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayQuoteCreated(quote) => {
                self.mavapay_current_quote = Some(quote);
                self.error = None;
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayQuoteError(error) => {
                self.error = Some(format!("Quote error: {}", error));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayConfirmQuote => {
                // TODO: Implement quote confirmation
                self.error = Some("Quote confirmation not yet implemented".to_string());
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayGetPrice => {
                return self.handle_mavapay_get_price();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayPriceReceived(price) => {
                self.mavapay_current_price = Some(price);
                self.error = None;
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayPriceError(error) => {
                self.error = Some(format!("Price error: {}", error));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayGetTransactions => {
                return self.handle_mavapay_get_transactions();
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayTransactionsReceived(transactions) => {
                self.mavapay_transactions = transactions;
                self.error = None;
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayTransactionsError(error) => {
                self.error = Some(format!("Transactions error: {}", error));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayConfirmPayment(quote_id) => {
                return self.handle_mavapay_confirm_payment(quote_id);
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayPaymentConfirmed(status) => {
                self.mavapay_payment_status = Some(status);
                self.error = None;
                // Start polling for status updates
                let quote_id = self
                    .mavapay_payment_status
                    .as_ref()
                    .unwrap()
                    .quote_id
                    .clone();
                return Task::done(Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayStartPolling(quote_id),
                )));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayPaymentConfirmationError(error) => {
                self.error = Some(format!("Payment confirmation error: {}", error));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayCheckPaymentStatus(quote_id) => {
                return self.handle_mavapay_check_payment_status(quote_id);
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayPaymentStatusUpdated(status) => {
                self.mavapay_payment_status = Some(status);
                self.error = None;
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayPaymentStatusError(error) => {
                self.error = Some(format!("Payment status error: {}", error));
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayStartPolling(quote_id) => {
                return self.handle_mavapay_start_polling(quote_id);
            }
            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::MavapayStopPolling => {
                self.mavapay_polling_active = false;
            }

            #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
            BuySellMessage::CreateSession => {
                // No providers in default build; ignore or show error
                self.set_error("No provider configured in this build".into());
            }

            #[cfg(all(feature = "dev-onramp", not(feature = "dev-meld")))]
            BuySellMessage::CreateSession => {
                if self.is_form_valid() {
                    let Some(onramper_url) = onramper::create_widget_url(
                        &self.fiat_currency.value,
                        &self.source_amount.value,
                        &self.wallet_address.value,
                    ) else {
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
                } else {
                    tracing::warn!("⚠️ [BUYSELL] Cannot create session - form validation failed");
                }
            }

            #[cfg(feature = "dev-meld")]
            BuySellMessage::CreateSession => {
                if self.is_form_valid() {
                    tracing::info!(
                        "🚀 [BUYSELL] Creating new session - clearing any existing session data"
                    );

                    // init session
                    let wallet_address = self.wallet_address.value.clone();
                    let country_code = self.country_code.value.clone();
                    let source_amount = self.source_amount.value.clone();

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
                                        wallet_address,
                                        country_code,
                                        source_amount,
                                        provider,
                                        network,
                                    )
                                    .await
                                {
                                    Ok(response) => Ok(response.widget_url),
                                    Err(MeldError::Network(e)) => {
                                        Err(format!("Network error: {}", e))
                                    }
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
                                Message::View(ViewMessage::BuySell(BuySellMessage::SessionError(
                                    error,
                                )))
                            }
                        },
                    );
                } else {
                    tracing::warn!("⚠️ [BUYSELL] Cannot create session - form validation failed");
                }
            }
            BuySellMessage::SessionError(error) => {
                self.set_error(error);
            }

            // webview logic
            BuySellMessage::ViewTick(id) => {
                let action = WebviewAction::Update(id);
                return self
                    .webview
                    .get_or_insert_with(init_webview)
                    .update(action)
                    .map(map_webview_message_static);
            }
            BuySellMessage::WebviewAction(action) => {
                return self
                    .webview
                    .get_or_insert_with(init_webview)
                    .update(action)
                    .map(map_webview_message_static);
            }
            BuySellMessage::WebviewOpenUrl(url) => {
                // Load URL into Ultralight webview
                tracing::info!("🌐 [LIANA] Loading Ultralight webview with URL: {}", url);
                self.session_url = Some(url.clone());

                // Create webview with URL string and immediately update to ensure content loads
                return self
                    .webview
                    .get_or_insert_with(init_webview)
                    .update(WebviewAction::CreateView(PageType::Url(url)))
                    .map(map_webview_message_static);
            }
            BuySellMessage::WebviewCreated(id) => {
                tracing::info!("🌐 [LIANA] Activating Webview Page: {}", id);

                // set active page to selected view id
                self.active_page = Some(id);
            }
            BuySellMessage::CloseWebview => {
                self.session_url = None;

                if let (Some(webview), Some(id)) = (self.webview.as_mut(), self.active_page.take())
                {
                    tracing::info!("🌐 [LIANA] Closing webview");
                    return webview
                        .update(WebviewAction::CloseView(id))
                        .map(map_webview_message_static);
                }
            }
        };

        Task::none()
    }

    fn close(&mut self) -> Task<Message> {
        Task::done(Message::View(ViewMessage::BuySell(
            BuySellMessage::CloseWebview,
        )))
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        use std::time::Duration;

        if let Some(id) = self.active_page {
            let interval = if cfg!(debug_assertions) {
                Duration::from_millis(250)
            } else {
                Duration::from_millis(100)
            };
            return iced::time::every(interval).with(id).map(|(i, ..)| {
                Message::View(ViewMessage::BuySell(BuySellMessage::ViewTick(i)))
            });
        }

        iced::Subscription::none()
    }
}

impl BuySellPanel {
    pub fn handle_native_login(&mut self) -> Task<Message> {
        if !self.is_login_form_valid() {
            self.set_error("Please enter email and password".into());
            return Task::none();
        }

        self.error = None;
        tracing::info!(
            "🔍 [LOGIN] Attempting login for user: {}",
            self.login_username.value
        );

        let client = self.registration_client.clone();
        let email = self.login_username.value.clone();
        let password = self.login_password.value.clone();

        Task::perform(
            async move {
                match client.login(&email, &password).await {
                    Ok(response) => {
                        tracing::info!("✅ [LOGIN] Login successful for user: {}", email);
                        Message::View(ViewMessage::BuySell(BuySellMessage::LoginSuccess(response)))
                    }
                    Err(e) => {
                        tracing::error!("❌ [LOGIN] Login failed for user {}: {}", email, e);
                        Message::View(ViewMessage::BuySell(BuySellMessage::LoginError(
                            e.to_string(),
                        )))
                    }
                }
            },
            |msg| msg,
        )
    }

    #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
    fn handle_mavapay_create_quote(&self) -> Task<Message> {
        use crate::services::mavapay::{Currency, PaymentMethod, QuoteRequest};

        // Validate required bank account fields
        if self.mavapay_bank_account_number.value.is_empty() {
            return Task::done(Message::View(ViewMessage::BuySell(
                BuySellMessage::MavapayQuoteError("Bank account number is required".to_string()),
            )));
        }

        if self.mavapay_bank_account_name.value.is_empty() {
            return Task::done(Message::View(ViewMessage::BuySell(
                BuySellMessage::MavapayQuoteError("Bank account name is required".to_string()),
            )));
        }

        if self.mavapay_bank_code.value.is_empty() {
            return Task::done(Message::View(ViewMessage::BuySell(
                BuySellMessage::MavapayQuoteError("Bank code is required".to_string()),
            )));
        }

        if self.mavapay_bank_name.value.is_empty() {
            return Task::done(Message::View(ViewMessage::BuySell(
                BuySellMessage::MavapayQuoteError("Bank name is required".to_string()),
            )));
        }

        let amount = match self.mavapay_amount.value.parse::<u64>() {
            Ok(amt) => amt,
            Err(_) => {
                return Task::done(Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayQuoteError("Invalid amount".to_string()),
                )));
            }
        };

        let source_currency = match self.mavapay_source_currency.value.as_str() {
            "BTCSAT" => Currency::BitcoinSatoshi,
            "NGNKOBO" => Currency::NigerianNairaKobo,
            "ZARCENT" => Currency::SouthAfricanRandCent,
            "KESCENT" => Currency::KenyanShillingCent,
            _ => {
                return Task::done(Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayQuoteError("Invalid source currency".to_string()),
                )));
            }
        };

        let target_currency = match self.mavapay_target_currency.value.as_str() {
            "BTCSAT" => Currency::BitcoinSatoshi,
            "NGNKOBO" => Currency::NigerianNairaKobo,
            "ZARCENT" => Currency::SouthAfricanRandCent,
            "KESCENT" => Currency::KenyanShillingCent,
            _ => {
                return Task::done(Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayQuoteError("Invalid target currency".to_string()),
                )));
            }
        };

        let request = QuoteRequest {
            amount: amount.to_string(),
            source_currency,
            target_currency,
            payment_method: PaymentMethod::Lightning, // Default to Lightning
            payment_currency: target_currency,
            autopayout: true,
            customer_internal_fee: "0".to_string(),
            beneficiary: Beneficiary::Bank(BankAccount {
                account_number: self.mavapay_bank_account_number.value.clone(),
                account_name: self.mavapay_bank_account_name.value.clone(),
                bank_code: self.mavapay_bank_code.value.clone(),
                bank_name: self.mavapay_bank_name.value.clone(),
            }),
        };

        let client = self.mavapay_client.clone();
        Task::perform(
            async move { client.create_quote(request).await },
            |result| match result {
                Ok(quote) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayQuoteCreated(quote),
                )),
                Err(error) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayQuoteError(error.to_string()),
                )),
            },
        )
    }

    #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
    fn handle_mavapay_get_price(&self) -> Task<Message> {
        let client = self.mavapay_client.clone();
        Task::perform(
            async move {
                client.get_price("NGN").await // Default to Nigerian Naira
            },
            |result| match result {
                Ok(price) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayPriceReceived(price),
                )),
                Err(error) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayPriceError(error.to_string()),
                )),
            },
        )
    }

    #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
    fn handle_mavapay_get_transactions(&self) -> Task<Message> {
        let client = self.mavapay_client.clone();
        Task::perform(
            async move {
                client.get_transactions(Some(1), Some(10), None).await // Get first 10 transactions
            },
            |result| match result {
                Ok(transactions) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayTransactionsReceived(transactions),
                )),
                Err(error) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayTransactionsError(error.to_string()),
                )),
            },
        )
    }

    #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
    fn handle_mavapay_confirm_payment(&self, quote_id: String) -> Task<Message> {
        let client = self.mavapay_client.clone();
        Task::perform(
            async move { client.confirm_quote(quote_id).await },
            |result| match result {
                Ok(status) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayPaymentConfirmed(status),
                )),
                Err(error) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayPaymentConfirmationError(error.to_string()),
                )),
            },
        )
    }

    #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
    fn handle_mavapay_check_payment_status(&self, quote_id: String) -> Task<Message> {
        let client = self.mavapay_client.clone();
        Task::perform(
            async move { client.get_payment_status(&quote_id).await },
            |result| match result {
                Ok(status) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayPaymentStatusUpdated(status),
                )),
                Err(error) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayPaymentStatusError(error.to_string()),
                )),
            },
        )
    }

    #[cfg(not(any(feature = "dev-meld", feature = "dev-onramp")))]
    fn handle_mavapay_start_polling(&self, quote_id: String) -> Task<Message> {
        let client = self.mavapay_client.clone();
        Task::perform(
            async move {
                // Poll every 5 seconds for up to 20 attempts (100 seconds total)
                client.poll_transaction_status(&quote_id, 20, 5).await
            },
            |result| match result {
                Ok(status) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayPaymentStatusUpdated(status),
                )),
                Err(error) => Message::View(ViewMessage::BuySell(
                    BuySellMessage::MavapayPaymentStatusError(error.to_string()),
                )),
            },
        )
    }
}
