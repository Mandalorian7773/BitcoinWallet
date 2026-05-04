use bdk::bitcoin::Network;

pub const TESTNET_ELECTRUM_URL: &str = "ssl://electrum.blockstream.info:60002";
pub const MAINNET_ELECTRUM_URL: &str = "ssl://electrum.blockstream.info:50002";

pub fn electrum_url(network: Network) -> &'static str {
    match network {
        Network::Bitcoin => MAINNET_ELECTRUM_URL,
        _ => TESTNET_ELECTRUM_URL,
    }
}
