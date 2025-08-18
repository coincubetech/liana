#[cfg(feature = "dev-meld")]
use iced::Task;
#[cfg(feature = "dev-meld")]
use std::sync::Arc;

#[cfg(feature = "dev-meld")]
use liana_ui::widget::Element;

#[cfg(feature = "dev-meld")]
use crate::{
    app::{
        buysell::{ServiceProvider, meld::{MeldClient, MeldError}},
        cache::Cache,
        message::Message,
        state::State,
        view::{meld_buysell::{meld_buysell_view, MeldBuySellPanel}, MeldBuySellMessage, Message as ViewMessage},
        wallet::Wallet,
    },
    daemon::Daemon,
};

#[cfg(feature = "dev-meld")]
impl Default for MeldBuySellPanel {
    fn default() -> Self {
        Self::new(liana::miniscript::bitcoin::Network::Bitcoin)
    }
}

#[cfg(feature = "dev-meld")]
impl State for MeldBuySellPanel {
    fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, ViewMessage> {
        crate::app::view::dashboard(
            &crate::app::menu::Menu::BuyAndSell,
            cache,
            None, // Error handling will be done within the meld view itself
            meld_buysell_view(self),
        )
    }

    fn update(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _cache: &Cache,
        message: Message,
    ) -> Task<Message> {
        match message {
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::ShowModal)) => {
                // No longer needed - form is always visible
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::CloseModal)) => {
                // No longer needed - form is always visible
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::WalletAddressChanged(address))) => {
                self.set_wallet_address(address);
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::CountryCodeChanged(code))) => {
                self.set_country_code(code);
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::SourceAmountChanged(amount))) => {
                self.set_source_amount(amount);
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::ProviderSelected(provider))) => {
                self.set_selected_provider(provider);
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::CreateSession)) => {
                if self.is_form_valid() {
                    self.start_session();
                    let wallet_address = self.wallet_address.value.clone();
                    let country_code = self.country_code.value.clone();
                    let source_amount = self.source_amount.value.clone();
                    let provider = self.selected_provider;

                    Task::perform(
                        create_meld_session(wallet_address, country_code, source_amount, provider, self.network),
                        |result| match result {
                            Ok(widget_url) => Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::SessionCreated(widget_url))),
                            Err(error) => Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::SessionError(error))),
                        }
                    )
                } else {
                    Task::none()
                }
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::SessionCreated(widget_url))) => {
                self.session_created(widget_url);
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::OpenWidget(widget_url))) => {
                // Log the URL we're trying to open
                tracing::info!("Attempting to open widget URL: {}", widget_url);

                // Use embedded webview if available, otherwise fallback to external browser
                #[cfg(feature = "webview")]
                {
                    tracing::info!("Opening widget URL in embedded webview");
                    return Task::done(Message::View(ViewMessage::OpenWebview(widget_url)));
                }

                #[cfg(not(feature = "webview"))]
                {
                    // Fallback to external browser
                    let mut success = false;

                    // Method 1: Try open::that_detached first (non-blocking)
                    match open::that_detached(&widget_url) {
                        Ok(_) => {
                            tracing::info!("Successfully opened widget URL with detached method");
                            success = true;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to open browser with detached method: {}", e);
                        }
                    }

                    // Method 2: Try WSL-specific commands first, then Linux commands
                    if !success {
                        // WSL-specific commands (these work better in WSL)
                        let wsl_commands = [
                            ("cmd.exe", vec!["/c", "start", &widget_url]),
                            ("powershell.exe", vec!["-c", "Start-Process", &widget_url]),
                            ("explorer.exe", vec![&widget_url]),
                        ];

                        // Try WSL commands first
                        for (cmd, args) in &wsl_commands {
                            match std::process::Command::new(cmd).args(args).spawn() {
                                Ok(_) => {
                                    tracing::info!("Successfully opened widget URL with WSL command: {}", cmd);
                                    success = true;
                                    break;
                                }
                                Err(_) => {
                                    tracing::debug!("WSL command {} not available", cmd);
                                }
                            }
                        }

                        // If WSL commands failed, try Linux commands
                        if !success {
                            let linux_commands = [
                                ("xdg-open", vec![&widget_url]),
                                ("firefox", vec![&widget_url]),
                                ("google-chrome", vec![&widget_url]),
                                ("chromium", vec![&widget_url]),
                                ("sensible-browser", vec![&widget_url]),
                            ];

                            for (cmd, args) in &linux_commands {
                                match std::process::Command::new(cmd).args(args).spawn() {
                                    Ok(_) => {
                                        tracing::info!("Successfully opened widget URL with Linux command: {}", cmd);
                                        success = true;
                                        break;
                                    }
                                    Err(_) => {
                                        tracing::debug!("Linux command {} not available", cmd);
                                    }
                                }
                            }
                        }
                    }

                    if !success {
                        tracing::error!("All browser opening methods failed");
                        self.set_error("Could not open browser automatically. Please copy the URL manually and paste it into your browser.".to_string());
                    }

                    Task::none()
                }
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::CopyUrl(widget_url))) => {
                tracing::info!("Attempting to copy URL to clipboard: {}", widget_url);

                let mut success = false;

                // Try WSL clipboard commands first
                let powershell_cmd = format!("Set-Clipboard '{}'", widget_url);
                let wsl_clipboard_commands = [
                    ("clip.exe", vec![]),  // Windows clipboard via WSL
                    ("powershell.exe", vec!["-c", &powershell_cmd]),
                ];

                for (cmd, args) in wsl_clipboard_commands {
                    if cmd == "clip.exe" {
                        // For clip.exe, we need to pipe the input
                        match std::process::Command::new(cmd)
                            .stdin(std::process::Stdio::piped())
                            .spawn() {
                            Ok(mut child) => {
                                if let Some(stdin) = child.stdin.take() {
                                    use std::io::Write;
                                    if let Ok(mut stdin) = std::io::BufWriter::new(stdin).into_inner() {
                                        if stdin.write_all(widget_url.as_bytes()).is_ok() {
                                            drop(stdin);
                                            if child.wait().is_ok() {
                                                tracing::info!("Successfully copied URL to clipboard with {}", cmd);
                                                success = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                tracing::debug!("WSL clipboard command {} not available", cmd);
                            }
                        }
                    } else {
                        match std::process::Command::new(cmd).args(&args).output() {
                            Ok(_) => {
                                tracing::info!("Successfully copied URL to clipboard with {}", cmd);
                                success = true;
                                break;
                            }
                            Err(_) => {
                                tracing::debug!("WSL clipboard command {} not available", cmd);
                            }
                        }
                    }
                }

                // Try Linux clipboard commands if WSL commands failed
                if !success {
                    let linux_clipboard_commands = [
                        ("xclip", vec!["-selection", "clipboard"]),
                        ("xsel", vec!["--clipboard", "--input"]),
                        ("pbcopy", vec![]),  // macOS
                    ];

                    for (cmd, args) in &linux_clipboard_commands {
                        match std::process::Command::new(cmd)
                            .args(args)
                            .stdin(std::process::Stdio::piped())
                            .spawn() {
                            Ok(mut child) => {
                                if let Some(stdin) = child.stdin.take() {
                                    use std::io::Write;
                                    if let Ok(mut stdin) = std::io::BufWriter::new(stdin).into_inner() {
                                        if stdin.write_all(widget_url.as_bytes()).is_ok() {
                                            drop(stdin);
                                            if child.wait().is_ok() {
                                                tracing::info!("Successfully copied URL to clipboard with {}", cmd);
                                                success = true;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                tracing::debug!("Linux clipboard command {} not available", cmd);
                            }
                        }
                    }
                }

                if !success {
                    tracing::warn!("Could not copy to clipboard automatically. URL logged for manual copying.");
                    self.set_error("Could not copy to clipboard automatically. Please copy the URL manually from the display above.".to_string());
                }

                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::SessionError(error))) => {
                self.set_error(error);
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::UrlCopied)) => {
                // Show success message for copied URL
                tracing::info!("URL copied to clipboard successfully");
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::CopyError)) => {
                self.set_error("Failed to copy URL to clipboard. Please copy manually.".to_string());
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::ResetForm)) => {
                self.error = None;
                self.widget_url = None;
                self.widget_session_created = None;
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::GoBackToForm)) => {
                // Reset to form state - clear widget URL and error
                self.widget_url = None;
                self.error = None;
                self.widget_session_created = None;
                Task::none()
            }
            Message::View(ViewMessage::MeldBuySell(MeldBuySellMessage::OpenWidgetInNewWindow(widget_url))) => {
                // Open in a new window/browser tab - similar to OpenWidget but explicitly for new window
                tracing::info!("Attempting to open widget URL in new window: {}", widget_url);

                let mut success = false;

                // Method 1: Try open::that_detached first (non-blocking)
                match open::that_detached(&widget_url) {
                    Ok(_) => {
                        tracing::info!("Successfully opened widget URL in new window with detached method");
                        success = true;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to open browser with detached method: {}", e);
                    }
                }

                // Method 2: Try WSL-specific commands first, then Linux commands
                if !success {
                    // WSL-specific commands (these work better in WSL)
                    let wsl_commands = [
                        ("cmd.exe", vec!["/c", "start", &widget_url]),
                        ("powershell.exe", vec!["-c", "Start-Process", &widget_url]),
                        ("explorer.exe", vec![&widget_url]),
                    ];

                    // Try WSL commands first
                    for (cmd, args) in &wsl_commands {
                        match std::process::Command::new(cmd).args(args).spawn() {
                            Ok(_) => {
                                tracing::info!("Successfully opened widget URL in new window with WSL command: {}", cmd);
                                success = true;
                                break;
                            }
                            Err(_) => {
                                tracing::debug!("WSL command {} not available", cmd);
                            }
                        }
                    }

                    // If WSL commands failed, try Linux commands
                    if !success {
                        let linux_commands = [
                            ("xdg-open", vec![&widget_url]),
                            ("firefox", vec![&widget_url]),
                            ("google-chrome", vec![&widget_url]),
                            ("chromium", vec![&widget_url]),
                            ("sensible-browser", vec![&widget_url]),
                        ];

                        for (cmd, args) in &linux_commands {
                            match std::process::Command::new(cmd).args(args).spawn() {
                                Ok(_) => {
                                    tracing::info!("Successfully opened widget URL in new window with Linux command: {}", cmd);
                                    success = true;
                                    break;
                                }
                                Err(_) => {
                                    tracing::debug!("Linux command {} not available", cmd);
                                }
                            }
                        }
                    }
                }

                if !success {
                    tracing::error!("All browser opening methods failed for new window");
                    self.set_error("Could not open browser automatically. Please copy the URL manually and paste it into your browser.".to_string());
                }

                Task::none()
            }
            _ => Task::none(),
        }
    }

    fn reload(
        &mut self,
        _daemon: Arc<dyn Daemon + Sync + Send>,
        _wallet: Arc<Wallet>,
    ) -> Task<Message> {
        Task::none()
    }
}

#[cfg(feature = "dev-meld")]
async fn create_meld_session(
    wallet_address: String,
    country_code: String,
    source_amount: String,
    provider: ServiceProvider,
    network: liana::miniscript::bitcoin::Network,
) -> Result<String, String> {
    let client = MeldClient::new();
    
    match client.create_widget_session(
        wallet_address,
        country_code,
        source_amount,
        provider,
        network,
    ).await {
        Ok(response) => Ok(response.widget_url),
        Err(MeldError::Network(e)) => Err(format!("Network error: {}", e)),
        Err(MeldError::Serialization(e)) => Err(format!("Data error: {}", e)),
        Err(MeldError::Api(e)) => Err(format!("API error: {}", e)),
    }
}
