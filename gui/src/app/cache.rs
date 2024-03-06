use crate::daemon::model::Coin;
use liana::miniscript::bitcoin::Network;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Cache {
    pub datadir_path: PathBuf,
    pub network: Network,
    pub blockheight: i32,
    pub coins: Vec<Coin>,
    pub rescan_progress: Option<f64>,
}

/// only used for tests.
impl std::default::Default for Cache {
    fn default() -> Self {
        Self {
            datadir_path: std::path::PathBuf::new(),
            network: Network::Bitcoin,
            blockheight: 0,
            coins: Vec::new(),
            rescan_progress: None,
        }
    }
}
