#[cfg(feature = "buysell")]
pub mod buysell;
#[cfg(feature = "breez")]
pub mod breez;
pub mod cache;
pub mod config;
pub mod error;
pub mod menu;
pub mod message;
pub mod settings;
pub mod state;
pub mod view;
pub mod wallet;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use iced::{clipboard, time, widget::Column, Subscription, Task};
use tokio::runtime::Handle;
use tracing::{error, info, warn};

pub use liana::miniscript::bitcoin;
use liana_ui::{component::network_banner, widget::Element};
pub use lianad::{commands::CoinStatus, config::Config as DaemonConfig};

pub use config::Config;
pub use message::Message;

use state::{
    CoinsPanel, CreateSpendPanel, Home, PsbtsPanel, ReceivePanel, State, TransactionsPanel,
};
use wallet::{sync_status, SyncStatus};

use crate::{
    app::{
        cache::{Cache, DaemonCache},
        error::Error,
        menu::Menu,
        message::FiatMessage,
        settings::WalletId,
        wallet::Wallet,
    },
    daemon::{embedded::EmbeddedDaemon, Daemon, DaemonBackend},
    dir::LianaDirectory,
    node::{bitcoind::Bitcoind, NodeType},
};

use self::state::SettingsState;

struct Panels {
    current: Menu,
    home: Home,
    coins: CoinsPanel,
    transactions: TransactionsPanel,
    psbts: PsbtsPanel,
    recovery: CreateSpendPanel,
    receive: ReceivePanel,
    create_spend: CreateSpendPanel,
    settings: SettingsState,
    #[cfg(feature = "buysell")]
    buy_sell: crate::app::view::buysell::BuySellPanel,
    #[cfg(feature = "breez")]
    activate: crate::app::view::ActivatePanel,
}

impl Panels {
    fn new(
        cache: &Cache,
        wallet: Arc<Wallet>,
        data_dir: LianaDirectory,
        daemon_backend: DaemonBackend,
        internal_bitcoind: Option<&Bitcoind>,
        config: Arc<Config>,
        restored_from_backup: bool,
    ) -> Panels {
        let show_rescan_warning = restored_from_backup
            && daemon_backend.is_lianad()
            && daemon_backend
                .node_type()
                .map(|nt| nt == NodeType::Bitcoind)
                // We don't know the node type for external lianad so assume it's bitcoind.
                .unwrap_or(true);
        Self {
            current: Menu::Home,
            home: Home::new(
                wallet.clone(),
                cache.coins(),
                sync_status(
                    daemon_backend.clone(),
                    cache.blockheight(),
                    cache.sync_progress(),
                    cache.last_poll_timestamp(),
                    cache.last_poll_at_startup,
                ),
                cache.blockheight(),
                show_rescan_warning,
            ),
            coins: CoinsPanel::new(cache.coins(), wallet.main_descriptor.first_timelock_value()),
            transactions: TransactionsPanel::new(wallet.clone()),
            psbts: PsbtsPanel::new(wallet.clone()),
            recovery: new_recovery_panel(wallet.clone(), cache),
            receive: ReceivePanel::new(data_dir.clone(), wallet.clone()),
            create_spend: CreateSpendPanel::new(
                wallet.clone(),
                cache.coins(),
                cache.blockheight() as u32,
                cache.network,
            ),
            settings: state::SettingsState::new(
                data_dir.clone(),
                wallet.clone(),
                daemon_backend,
                internal_bitcoind.is_some(),
                config.clone(),
            ),
            #[cfg(feature = "buysell")]
            buy_sell: crate::app::view::buysell::BuySellPanel::new(cache.network, wallet.clone(), data_dir.clone()),
            #[cfg(feature = "breez")]
            activate: crate::app::view::ActivatePanel::new(cache.network, wallet, data_dir),
        }
    }

    fn current(&self) -> &dyn State {
        match self.current {
            Menu::Home => &self.home,
            Menu::Receive => &self.receive,
            Menu::PSBTs => &self.psbts,
            Menu::Transactions => &self.transactions,
            Menu::TransactionPreSelected(_) => &self.transactions,
            Menu::Settings | Menu::SettingsPreSelected(_) => &self.settings,
            Menu::Coins => &self.coins,
            Menu::CreateSpendTx => &self.create_spend,
            Menu::Recovery => &self.recovery,
            Menu::RefreshCoins(_) => &self.create_spend,
            Menu::PsbtPreSelected(_) => &self.psbts,
            #[cfg(feature = "buysell")]
            Menu::BuySell => &self.buy_sell,
            #[cfg(feature = "breez")]
            Menu::Activate(_) => &self.activate,
        }
    }

    fn current_mut(&mut self) -> &mut dyn State {
        match self.current {
            Menu::Home => &mut self.home,
            Menu::Receive => &mut self.receive,
            Menu::PSBTs => &mut self.psbts,
            Menu::Transactions => &mut self.transactions,
            Menu::TransactionPreSelected(_) => &mut self.transactions,
            Menu::Settings | Menu::SettingsPreSelected(_) => &mut self.settings,
            Menu::Coins => &mut self.coins,
            Menu::CreateSpendTx => &mut self.create_spend,
            Menu::Recovery => &mut self.recovery,
            Menu::RefreshCoins(_) => &mut self.create_spend,
            Menu::PsbtPreSelected(_) => &mut self.psbts,
            #[cfg(feature = "buysell")]
            Menu::BuySell => &mut self.buy_sell,
            #[cfg(feature = "breez")]
            Menu::Activate(_) => &mut self.activate,
        }
    }
}

pub struct App {
    cache: Cache,
    wallet: Arc<Wallet>,
    daemon: Arc<dyn Daemon + Sync + Send>,
    internal_bitcoind: Option<Bitcoind>,

    panels: Panels,

    #[cfg(feature = "breez")]
    breez_manager: Option<std::sync::Arc<crate::app::breez::wallet::BreezWalletManager>>,
    #[cfg(feature = "breez")]
    breez_event_receiver: Option<tokio::sync::mpsc::UnboundedReceiver<crate::app::breez::events::BreezEvent>>,
}

impl App {
    pub fn new(
        cache: Cache,
        wallet: Arc<Wallet>,
        config: Config,
        daemon: Arc<dyn Daemon + Sync + Send>,
        data_dir: LianaDirectory,
        internal_bitcoind: Option<Bitcoind>,
        restored_from_backup: bool,
    ) -> (App, Task<Message>) {
        let config = Arc::new(config);
        let mut panels = Panels::new(
            &cache,
            wallet.clone(),
            data_dir.clone(),
            daemon.backend(),
            internal_bitcoind.as_ref(),
            config.clone(),
            restored_from_backup,
        );
        let cmd = panels.home.reload(daemon.clone(), wallet.clone());

        // Initialize Breez SDK if feature is enabled and hot signer is available
        #[cfg(feature = "breez")]
        let breez_init_cmd = {
            if let Some(ref signer) = wallet.signer {
                let mnemonic = crate::app::breez::init::get_mnemonic_from_signer(signer);
                let network = cache.network;
                let breez_data_dir = data_dir.path().to_path_buf();

                Task::perform(
                    async move {
                        crate::app::breez::init::initialize_breez_sdk(
                            mnemonic,
                            network,
                            breez_data_dir,
                        )
                        .await
                    },
                    |result| Message::BreezInitialized(result),
                )
            } else {
                Task::none()
            }
        };

        (
            Self {
                panels,
                cache,
                daemon,
                wallet,
                internal_bitcoind,
                #[cfg(feature = "breez")]
                breez_manager: None,
                #[cfg(feature = "breez")]
                breez_event_receiver: None,
            },
            #[cfg(feature = "breez")]
            Task::batch([cmd, breez_init_cmd]),
            #[cfg(not(feature = "breez"))]
            cmd,
        )
    }

    pub fn wallet_id(&self) -> WalletId {
        self.wallet.id()
    }

    pub fn title(&self) -> &str {
        if let Some(alias) = &self.wallet.alias {
            if !alias.is_empty() {
                return alias;
            }
        }

        "Coincube Vault Wallet"
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    pub fn wallet(&self) -> &Wallet {
        &self.wallet
    }

    #[cfg(feature = "breez")]
    fn get_activate_panel_mut(&mut self) -> Option<&mut view::ActivatePanel> {
        if matches!(self.panels.current, Menu::Activate(_)) {
            Some(&mut self.panels.activate)
        } else {
            None
        }
    }

    fn set_current_panel(&mut self, menu: Menu) -> Task<Message> {
        self.panels.current_mut().interrupt();

        // Handle Activate submenu - update the active panel
        #[cfg(feature = "breez")]
        if let menu::Menu::Activate(submenu) = &menu {
            use crate::app::view::activate::ActivateSubPanel;
            self.panels.activate.active_panel = match submenu {
                menu::ActivateMenu::Main => ActivateSubPanel::Main,
                menu::ActivateMenu::Send => ActivateSubPanel::Send,
                menu::ActivateMenu::Receive => ActivateSubPanel::Receive,
                menu::ActivateMenu::History => ActivateSubPanel::History,
            };
            
            // Check if we need to sync panel state with app-level breez_manager
            #[cfg(feature = "breez")]
            if self.breez_manager.is_some() && self.panels.activate.breez_manager.is_none() {
                // App has breez_manager but panel doesn't - sync them
                log::info!("🔄 Syncing breez_manager to Activate panel");
                if let Some(ref manager) = self.breez_manager {
                    self.panels.activate.breez_manager = Some((**manager).clone());
                }
                self.panels.activate.lightning_wallet_state = view::activate::LightningWalletState::Ready;
            }
            
            // Only trigger SDK initialization if:
            // 1. Panel state is Initializing (wallet exists but not connected)
            // 2. SDK is not already initialized at app level
            // 3. Panel doesn't have breez_manager yet
            #[cfg(feature = "breez")]
            if self.breez_manager.is_none() && self.panels.activate.should_initialize_sdk() {
                match self.panels.activate.get_init_params() {
                    Ok(Some((mnemonic, network, breez_data_dir))) => {
                        log::info!("🚀 Triggering SDK initialization for existing wallet (first time)");
                        return Task::perform(
                            async move {
                                crate::app::breez::init::initialize_breez_sdk(
                                    mnemonic,
                                    network,
                                    breez_data_dir,
                                )
                                .await
                            },
                            Message::BreezInitialized,
                        );
                    }
                    Ok(None) => {
                        log::debug!("SDK initialization not needed");
                    }
                    Err(error_msg) => {
                        // Wallet corrupted - set state back to NotCreated with error
                        log::error!("🔴 Lightning wallet corrupted, resetting to NotCreated state");
                        self.panels.activate.lightning_wallet_state = view::activate::LightningWalletState::NotCreated;
                        self.panels.activate.error = Some(error_msg);
                    }
                }
            } else if self.breez_manager.is_some() {
                log::debug!("✅ SDK already initialized, reusing existing connection");
            }
        }

        match &menu {
            menu::Menu::TransactionPreSelected(txid) => {
                if let Ok(Some(tx)) = Handle::current().block_on(async {
                    self.daemon
                        .get_history_txs(&[*txid])
                        .await
                        .map(|txs| txs.first().cloned())
                }) {
                    self.panels.transactions.preselect(tx);
                    self.panels.current = menu;
                    return Task::none();
                };
            }
            menu::Menu::PsbtPreSelected(txid) => {
                // Get preselected spend from DB in case it's not yet in the cache.
                // We only need this single spend as we will go straight to its view and not show the PSBTs list.
                // In case of any error loading the spend or if it doesn't exist, load PSBTs list in usual way.
                if let Ok(Some(spend_tx)) = Handle::current().block_on(async {
                    self.daemon
                        .list_spend_transactions(Some(&[*txid]))
                        .await
                        .map(|txs| txs.first().cloned())
                }) {
                    self.panels.psbts.preselect(spend_tx);
                    self.panels.current = menu;
                    return Task::none();
                };
            }
            menu::Menu::SettingsPreSelected(setting) => {
                self.panels.current = menu.clone();
                return self.panels.current_mut().update(
                    self.daemon.clone(),
                    &self.cache,
                    Message::View(view::Message::Settings(match setting {
                        &menu::SettingsOption::Node => view::SettingsMessage::EditBitcoindSettings,
                    })),
                );
            }
            menu::Menu::RefreshCoins(preselected) => {
                self.panels.create_spend = CreateSpendPanel::new_self_send(
                    self.wallet.clone(),
                    self.cache.coins(),
                    self.cache.blockheight() as u32,
                    preselected,
                    self.cache.network,
                );
            }
            menu::Menu::CreateSpendTx => {
                // redo the process of spending only if user want to start a new one.
                if !self.panels.create_spend.keep_state() {
                    self.panels.create_spend = CreateSpendPanel::new(
                        self.wallet.clone(),
                        self.cache.coins(),
                        self.cache.blockheight() as u32,
                        self.cache.network,
                    );
                }
            }
            menu::Menu::Recovery => {
                if !self.panels.recovery.keep_state() {
                    self.panels.recovery = new_recovery_panel(self.wallet.clone(), &self.cache);
                }
            }
            _ => {}
        };

        self.panels.current = menu;
        self.panels
            .current_mut()
            .reload(self.daemon.clone(), self.wallet.clone())
    }

    pub fn subscription(&self) -> Subscription<Message> {
        #[allow(unused_mut)]
        let mut subscriptions = vec![
            time::every(Duration::from_secs(
                match sync_status(
                    self.daemon.backend(),
                    self.cache.blockheight(),
                    self.cache.sync_progress(),
                    self.cache.last_poll_timestamp(),
                    self.cache.last_poll_at_startup,
                ) {
                    SyncStatus::BlockchainSync(_) => 5, // Only applies to local backends
                    SyncStatus::WalletFullScan
                        if self.daemon.backend() == DaemonBackend::RemoteBackend =>
                    {
                        10
                    } // If remote backend, don't ping too often
                    SyncStatus::WalletFullScan | SyncStatus::LatestWalletSync => 3,
                    SyncStatus::Synced => {
                        if self.daemon.backend() == DaemonBackend::RemoteBackend {
                            // Remote backend has no rescan feature. For a synced wallet,
                            // cache refresh is only used to warn user about recovery availability.
                            120
                        } else {
                            // For the rescan feature, we refresh more often in order
                            // to give user an up-to-date view of the rescan progress.
                            10
                        }
                    }
                },
            ))
            .map(|_| Message::Tick),
            self.panels.current().subscription(),
        ];

        // Breez event subscription is handled internally by the SDK

        Subscription::batch(subscriptions)
    }

    pub fn stop(&mut self) {
        info!("Close requested");
        if self.daemon.backend().is_embedded() {
            if let Err(e) = Handle::current().block_on(async { self.daemon.stop().await }) {
                error!("{}", e);
            } else {
                info!("Internal daemon stopped");
            }
            if let Some(bitcoind) = self.internal_bitcoind.take() {
                bitcoind.stop();
            }
        }
    }

    pub fn on_tick(&mut self) -> Task<Message> {
        let tick = std::time::Instant::now();
        let mut tasks =
            vec![self
                .panels
                .current_mut()
                .update(self.daemon.clone(), &self.cache, Message::Tick)];

        // Check if we need to update the daemon cache.
        let duration = Duration::from_secs(
            match sync_status(
                self.daemon.backend(),
                self.cache.blockheight(),
                self.cache.sync_progress(),
                self.cache.last_poll_timestamp(),
                self.cache.last_poll_at_startup,
            ) {
                SyncStatus::BlockchainSync(_) => 5, // Only applies to local backends
                SyncStatus::WalletFullScan
                    if self.daemon.backend() == DaemonBackend::RemoteBackend =>
                {
                    10
                } // If remote backend, don't ping too often
                SyncStatus::WalletFullScan | SyncStatus::LatestWalletSync => 3,
                SyncStatus::Synced => {
                    if self.daemon.backend() == DaemonBackend::RemoteBackend {
                        // Remote backend has no rescan feature. For a synced wallet,
                        // cache refresh is only used to warn user about recovery availability.
                        120
                    } else {
                        // For the rescan feature, we refresh more often in order
                        // to give user an up-to-date view of the rescan progress.
                        10
                    }
                }
            },
        );
        if self.cache.daemon_cache.last_tick + duration <= tick {
            tracing::debug!("Updating daemon cache");

            // We have to update here the last_tick to prevent that during a burst of events
            // there is a race condition with the Task and too much tasks are triggered.
            self.cache.daemon_cache.last_tick = tick;

            let daemon = self.daemon.clone();
            let datadir_path = self.cache.datadir_path.clone();
            let network = self.cache.network;
            tasks.push(Task::perform(
                async move {
                    // we check every 10 second if the daemon poller is alive
                    // or if the access token is not expired.
                    daemon.is_alive(&datadir_path, network).await?;

                    let info = daemon.get_info().await?;
                    let coins = cache::coins_to_cache(daemon).await?;
                    Ok(DaemonCache {
                        blockheight: info.block_height,
                        coins: coins.coins,
                        rescan_progress: info.rescan_progress,
                        sync_progress: info.sync,
                        last_poll_timestamp: info.last_poll_timestamp,
                        last_tick: tick,
                    })
                },
                Message::UpdateDaemonCache,
            ));
        }
        Task::batch(tasks)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            #[cfg(feature = "breez")]
            Message::BreezInitialized(result) => {
                match result {
                    Ok((manager, event_receiver)) => {
                        tracing::info!("✅ Breez SDK initialized successfully");
                        self.breez_manager = Some(manager.clone());
                        self.breez_event_receiver = Some(event_receiver);

                        // Pass manager to ActivatePanel and set state to Ready
                        if let Some(activate_panel) = self.get_activate_panel_mut() {
                            activate_panel.breez_manager = Some((*manager).clone());
                            // Update state to Ready now that SDK is connected
                            activate_panel.lightning_wallet_state = view::activate::LightningWalletState::Ready;
                            tracing::info!("Lightning wallet state updated to Ready");
                            // Trigger initial balance fetch
                            return activate_panel.refresh_balance();
                        }
                    }
                    Err(e) => {
                        tracing::error!("❌ Failed to initialize Breez SDK: {}", e);
                        // Update ActivatePanel with error
                        if let Some(activate_panel) = self.get_activate_panel_mut() {
                            // Set back to NotCreated so user can try again
                            activate_panel.lightning_wallet_state = view::activate::LightningWalletState::NotCreated;
                            activate_panel.error = Some(format!("Breez SDK initialization failed: {}. Please check your API key and try again.", e));
                        }
                    }
                }
                return Task::none();
            }
            #[cfg(feature = "breez")]
            Message::BreezEvent(event) => {
                tracing::debug!("Breez event: {:?}", event);
                // Handle Breez events (balance updates, payment status, etc.)
                if let Some(activate_panel) = self.get_activate_panel_mut() {
                    // Trigger balance refresh on relevant events
                    match event {
                        crate::app::breez::events::BreezEvent::PaymentSucceeded { .. }
                        | crate::app::breez::events::BreezEvent::BalanceUpdated
                        | crate::app::breez::events::BreezEvent::SyncComplete => {
                            return activate_panel.refresh_balance();
                        }
                        _ => {}
                    }
                }
                return Task::none();
            }
            Message::Fiat(FiatMessage::GetPriceResult(fiat_price)) => {
                if self.wallet.fiat_price_is_relevant(&fiat_price)
                    // make sure we only update if the price is newer than the cached one
                    && !self.cache.fiat_price.as_ref().is_some_and(|cached| {
                        cached.source() == fiat_price.source()
                            && cached.currency() == fiat_price.currency()
                            && cached.requested_at() >= fiat_price.requested_at()
                    })
                {
                    self.cache.fiat_price = Some(fiat_price);
                    Task::perform(async {}, |_| Message::CacheUpdated)
                } else {
                    Task::none()
                }
            }
            Message::UpdateDaemonCache(res) => {
                match res {
                    Ok(daemon_cache) => {
                        self.cache.daemon_cache = daemon_cache;
                        return Task::perform(async {}, |_| Message::CacheUpdated);
                    }
                    Err(e) => tracing::error!("Failed to update daemon cache: {}", e),
                }
                Task::none()
            }
            Message::CacheUpdated => {
                // These are the panels to update with the cache.
                let mut panels = [
                    (&mut self.panels.home as &mut dyn State, Menu::Home),
                    (&mut self.panels.settings as &mut dyn State, Menu::Settings),
                ];
                let daemon = self.daemon.clone();
                let current = &self.panels.current;
                let cache = self.cache.clone();
                let commands: Vec<_> = panels
                    .iter_mut()
                    .map(|(panel, menu)| {
                        panel.update(
                            daemon.clone(),
                            &cache,
                            Message::UpdatePanelCache(current == menu),
                        )
                    })
                    .collect();
                Task::batch(commands)
            }
            Message::LoadDaemonConfig(cfg) => {
                let res = self.load_daemon_config(self.cache.datadir_path.clone(), *cfg);
                self.update(Message::DaemonConfigLoaded(res))
            }
            Message::WalletUpdated(Ok(wallet)) => {
                self.wallet = wallet.clone();
                self.panels.current_mut().update(
                    self.daemon.clone(),
                    &self.cache,
                    Message::WalletUpdated(Ok(wallet)),
                )
            }
            #[cfg(feature = "breez")]
            Message::View(view::Message::ToggleActivateMenu) => {
                // Smart context-aware toggle:
                // - If in submenu (Send/Receive/History) → Go to Main (don't collapse)
                // - If in Main → Toggle collapse/expand
                
                let current_menu = &self.panels.current;
                
                match current_menu {
                    // User is in a submenu - navigate to Main instead of collapsing
                    Menu::Activate(menu::ActivateMenu::Send) 
                    | Menu::Activate(menu::ActivateMenu::Receive) 
                    | Menu::Activate(menu::ActivateMenu::History) => {
                        log::debug!("In Activate submenu, navigating to Main");
                        let menu = Menu::Activate(menu::ActivateMenu::Main);
                        return Task::batch([
                            self.panels.current_mut().close(),
                            self.set_current_panel(menu),
                        ]);
                    }
                    
                    // User is in Main or outside Activate - toggle expansion
                    Menu::Activate(menu::ActivateMenu::Main) => {
                        log::debug!("In Main, toggling menu collapse");
                        self.cache.activate_expanded = !self.cache.activate_expanded;
                        
                        // If expanding, navigate to Main panel
                        if self.cache.activate_expanded {
                            let menu = Menu::Activate(menu::ActivateMenu::Main);
                            return Task::batch([
                                self.panels.current_mut().close(),
                                self.set_current_panel(menu),
                            ]);
                        }
                        Task::none()
                    }
                    
                    // User is outside Activate menu - expand and go to Main
                    _ => {
                        log::debug!("Outside Activate, expanding menu");
                        self.cache.activate_expanded = true;
                        let menu = Menu::Activate(menu::ActivateMenu::Main);
                        return Task::batch([
                            self.panels.current_mut().close(),
                            self.set_current_panel(menu),
                        ]);
                    }
                }
            }
            Message::View(view::Message::Menu(menu)) => Task::batch([
                self.panels.current_mut().close(),
                self.set_current_panel(menu),
            ]),
            Message::View(view::Message::OpenUrl(url)) => {
                if let Err(e) = open::that_detached(&url) {
                    tracing::error!("Error opening '{}': {}", url, e);
                }
                Task::none()
            }
            Message::View(view::Message::Clipboard(text)) => clipboard::write(text),

            // TODO: Move to panel.state
            _ => self
                .panels
                .current_mut()
                .update(self.daemon.clone(), &self.cache, message),
        }
    }

    pub fn load_daemon_config(
        &mut self,
        datadir_path: LianaDirectory,
        cfg: DaemonConfig,
    ) -> Result<(), Error> {
        Handle::current().block_on(async { self.daemon.stop().await })?;
        let network = cfg.bitcoin_config.network;
        let daemon = EmbeddedDaemon::start(cfg)?;
        self.daemon = Arc::new(daemon);
        let mut daemon_config_path = datadir_path
            .network_directory(network)
            .lianad_data_directory(&self.wallet.id())
            .path()
            .to_path_buf();
        daemon_config_path.push("daemon.toml");

        let content =
            toml::to_string(&self.daemon.config()).map_err(|e| Error::Config(e.to_string()))?;

        OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(daemon_config_path)
            .map_err(|e| Error::Config(e.to_string()))?
            .write_all(content.as_bytes())
            .map_err(|e| {
                warn!("failed to write to file: {:?}", e);
                Error::Config(e.to_string())
            })
    }

    pub fn view(&self) -> Element<'_, Message> {
        let view = self.panels.current().view(&self.cache);

        if self.cache.network != bitcoin::Network::Bitcoin {
            Column::with_children([
                network_banner(self.cache.network).into(),
                view.map(Message::View),
            ])
            .into()
        } else {
            view.map(Message::View)
        }
    }

    pub fn datadir_path(&self) -> &LianaDirectory {
        &self.cache.datadir_path
    }
}

fn new_recovery_panel(wallet: Arc<Wallet>, cache: &Cache) -> CreateSpendPanel {
    CreateSpendPanel::new_recovery(
        wallet,
        cache.coins(),
        cache.blockheight() as u32,
        cache.network,
    )
}

