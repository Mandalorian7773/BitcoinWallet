use bdk::bitcoin::Network;
use clap::ValueEnum;

pub const TESTNET_ELECTRUM_URL: &str = "ssl://electrum.blockstream.info:60002";
pub const MAINNET_ELECTRUM_URL: &str = "ssl://electrum.blockstream.info:50002";

/// User-facing network selector for CLI arguments.
///
/// # Arguments
///
/// No runtime arguments are required; clap constructs this from `--network`.
///
/// # Returns
///
/// Converts into the matching BDK [`Network`].
///
/// # Example
///
/// ```
/// use bdk::bitcoin::Network;
/// use bitcoin_wallet::config::NetworkArg;
///
/// assert_eq!(Network::from(NetworkArg::Testnet), Network::Testnet);
/// ```
#[derive(Clone, Copy, ValueEnum, Debug, Default, Eq, PartialEq)]
pub enum NetworkArg {
    // WHY: testnet is the safe default for a teaching wallet because it
    // prevents accidental mainnet broadcasts while experimenting.
    #[default]
    Testnet,
    Mainnet,
    Regtest,
}

impl From<NetworkArg> for Network {
    fn from(n: NetworkArg) -> Self {
        match n {
            NetworkArg::Testnet => Network::Testnet,
            NetworkArg::Mainnet => Network::Bitcoin,
            NetworkArg::Regtest => Network::Regtest,
        }
    }
}

/// Return the Electrum endpoint for a Bitcoin network.
///
/// # Arguments
///
/// * `network` - BDK network used by the wallet.
///
/// # Returns
///
/// A static Electrum URL for mainnet or testnet-compatible networks.
///
/// # Example
///
/// ```
/// use bdk::bitcoin::Network;
/// use bitcoin_wallet::config::electrum_url;
///
/// assert_eq!(electrum_url(Network::Testnet), "ssl://electrum.blockstream.info:60002");
/// ```
#[must_use]
pub fn electrum_url(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => MAINNET_ELECTRUM_URL,
        _ => TESTNET_ELECTRUM_URL,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_flag_testnet() {
        assert_eq!(Network::from(NetworkArg::Testnet), Network::Testnet);
    }

    #[test]
    fn test_network_flag_mainnet() {
        assert_eq!(Network::from(NetworkArg::Mainnet), Network::Bitcoin);
    }

    #[test]
    fn test_default_network_is_testnet() {
        assert_eq!(Network::from(NetworkArg::default()), Network::Testnet);
    }
}
