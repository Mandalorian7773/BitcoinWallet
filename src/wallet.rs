use bdk::{
    bitcoin::{Address, Network},
    blockchain::{electrum::ElectrumBlockchain, ConfigurableBlockchain, ElectrumBlockchainConfig},
    database::MemoryDatabase,
    keys::{
        bip39::{Language, Mnemonic, WordCount},
        DerivableKey, ExtendedKey, GeneratableKey, GeneratedKey,
    },
    template::{Bip84, DescriptorTemplate},
    wallet::AddressIndex,
    KeychainKind, SyncOptions, Wallet,
};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::config::electrum_url;

/// Errors returned by wallet and transaction operations.
///
/// WHY: typed variants let the CLI and tests distinguish user mistakes from
/// network/backend failures without parsing display strings.
#[derive(Debug, Error)]
pub enum WalletError {
    #[error("invalid mnemonic phrase: {0}")]
    InvalidMnemonic(String),
    #[error("insufficient confirmed balance: have {available} sats, need {required} sats")]
    InsufficientFunds { available: u64, required: u64 },
    #[error("invalid recipient address: {0}")]
    InvalidAddress(String),
    #[error("network error: {0}")]
    NetworkError(String),
    #[error("transaction signing failed: {0}")]
    SigningFailed(String),
}

/// Generate a new 12-word English BIP39 mnemonic.
///
/// # Arguments
///
/// This function does not take arguments.
///
/// # Returns
///
/// A new BDK BIP39 mnemonic on success.
///
/// # Example
///
/// ```
/// let mnemonic = bitcoin_wallet::wallet::generate_mnemonic()?;
/// assert_eq!(mnemonic.to_string().split_whitespace().count(), 12);
/// # Ok::<(), bitcoin_wallet::wallet::WalletError>(())
/// ```
pub fn generate_mnemonic() -> Result<Mnemonic, WalletError> {
    let generated: GeneratedKey<_, bdk::miniscript::Segwitv0> =
        Mnemonic::generate((WordCount::Words12, Language::English))
            .map_err(|e| WalletError::InvalidMnemonic(format!("generation failed: {e:?}")))?;
    Ok(generated.into_key())
}

/// Create an in-memory BIP84 wallet from a mnemonic.
///
/// # Arguments
///
/// * `mnemonic` - A BIP39 mnemonic phrase.
/// * `network` - Bitcoin network for descriptors and address encoding.
///
/// # Returns
///
/// A BDK wallet backed by [`MemoryDatabase`].
///
/// # Example
///
/// ```
/// use bdk::bitcoin::Network;
///
/// let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
/// let wallet = bitcoin_wallet::wallet::create_wallet(phrase, Network::Testnet)?;
/// assert_eq!(wallet.network(), Network::Testnet);
/// # Ok::<(), bitcoin_wallet::wallet::WalletError>(())
/// ```
pub fn create_wallet(
    mnemonic: &str,
    network: Network,
) -> Result<Wallet<MemoryDatabase>, WalletError> {
    // WHY: copy the phrase into Zeroizing storage before parsing so the
    // temporary secret buffer is wiped once key derivation completes.
    let mnemonic_phrase = Zeroizing::new(mnemonic.to_owned());
    let mnemonic: Mnemonic = mnemonic_phrase
        .parse::<Mnemonic>()
        .map_err(|e| WalletError::InvalidMnemonic(e.to_string()))?;
    let xkey: ExtendedKey = mnemonic
        .into_extended_key()
        .map_err(|e| WalletError::InvalidMnemonic(format!("key derivation failed: {e}")))?;
    let xprv = xkey
        .into_xprv(network)
        .ok_or_else(|| WalletError::InvalidMnemonic("xprv derivation failed".to_string()))?;

    let external = Bip84(xprv, KeychainKind::External)
        .build(network)
        .map_err(|e| WalletError::NetworkError(format!("build external descriptor: {e}")))?;
    let internal = Bip84(xprv, KeychainKind::Internal)
        .build(network)
        .map_err(|e| WalletError::NetworkError(format!("build internal descriptor: {e}")))?;

    Wallet::new(external, Some(internal), network, MemoryDatabase::default())
        .map_err(|e| WalletError::NetworkError(format!("construct BDK wallet: {e}")))
}

/// Synchronize a wallet with the configured Electrum backend.
///
/// # Arguments
///
/// * `wallet` - In-memory wallet to update.
/// * `network` - Network used to choose the Electrum endpoint.
///
/// # Returns
///
/// The connected Electrum backend, ready for broadcasting.
///
/// # Example
///
/// ```no_run
/// use bdk::bitcoin::Network;
///
/// let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
/// let wallet = bitcoin_wallet::wallet::create_wallet(phrase, Network::Testnet)?;
/// let _backend = bitcoin_wallet::wallet::sync_wallet(&wallet, Network::Testnet)?;
/// # Ok::<(), bitcoin_wallet::wallet::WalletError>(())
/// ```
pub fn sync_wallet(
    wallet: &Wallet<MemoryDatabase>,
    network: Network,
) -> Result<ElectrumBlockchain, WalletError> {
    let url = electrum_url(network);
    let blockchain = ElectrumBlockchain::from_config(&ElectrumBlockchainConfig {
        url: url.to_string(),
        socks5: None,
        retry: 3,
        timeout: Some(15),
        stop_gap: 10,
        validate_domain: true,
    })
    .map_err(|e| WalletError::NetworkError(format!("connect to Electrum server {url}: {e}")))?;

    wallet
        .sync(&blockchain, SyncOptions::default())
        .map_err(|e| WalletError::NetworkError(format!("sync wallet from Electrum: {e}")))?;

    Ok(blockchain)
}

/// Return the wallet balance from the local BDK database.
///
/// # Arguments
///
/// * `wallet` - Wallet whose cached balance should be read.
///
/// # Returns
///
/// BDK balance buckets for confirmed, pending, and immature sats.
///
/// # Example
///
/// ```
/// use bdk::bitcoin::Network;
///
/// let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
/// let wallet = bitcoin_wallet::wallet::create_wallet(phrase, Network::Testnet)?;
/// let balance = bitcoin_wallet::wallet::get_balance(&wallet)?;
/// assert_eq!(balance.confirmed, 0);
/// # Ok::<(), bitcoin_wallet::wallet::WalletError>(())
/// ```
pub fn get_balance(wallet: &Wallet<MemoryDatabase>) -> Result<bdk::Balance, WalletError> {
    wallet
        .get_balance()
        .map_err(|e| WalletError::NetworkError(format!("retrieve wallet balance: {e}")))
}

/// Derive and persist the next external receive address.
///
/// # Arguments
///
/// * `wallet` - Wallet whose external keychain should advance.
///
/// # Returns
///
/// A network-encoded BIP84 receive address.
///
/// # Example
///
/// ```
/// use bdk::bitcoin::Network;
///
/// let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
/// let wallet = bitcoin_wallet::wallet::create_wallet(phrase, Network::Testnet)?;
/// let address = bitcoin_wallet::wallet::get_new_address(&wallet)?;
/// assert!(address.to_string().starts_with("tb1"));
/// # Ok::<(), bitcoin_wallet::wallet::WalletError>(())
/// ```
pub fn get_new_address(wallet: &Wallet<MemoryDatabase>) -> Result<Address, WalletError> {
    wallet
        .get_address(AddressIndex::New)
        .map(|info| info.address)
        .map_err(|e| WalletError::NetworkError(format!("derive new address: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    #[test]
    fn test_create_wallet_from_valid_mnemonic() {
        assert!(create_wallet(VALID_MNEMONIC, Network::Testnet).is_ok());
    }

    #[test]
    fn test_create_wallet_from_invalid_mnemonic() {
        let result = create_wallet("not a valid mnemonic phrase", Network::Testnet);
        assert!(matches!(result, Err(WalletError::InvalidMnemonic(_))));
    }

    #[test]
    fn test_create_wallet_deterministic() -> Result<(), WalletError> {
        let first = create_wallet(VALID_MNEMONIC, Network::Testnet)?;
        let second = create_wallet(VALID_MNEMONIC, Network::Testnet)?;

        assert_eq!(get_new_address(&first)?, get_new_address(&second)?);
        Ok(())
    }

    #[test]
    fn test_get_new_address_increments() -> Result<(), WalletError> {
        let wallet = create_wallet(VALID_MNEMONIC, Network::Testnet)?;

        assert_ne!(get_new_address(&wallet)?, get_new_address(&wallet)?);
        Ok(())
    }

    #[test]
    fn test_get_new_address_is_valid_bech32() -> Result<(), WalletError> {
        let wallet = create_wallet(VALID_MNEMONIC, Network::Testnet)?;

        assert!(get_new_address(&wallet)?.to_string().starts_with("tb1"));
        Ok(())
    }
}
