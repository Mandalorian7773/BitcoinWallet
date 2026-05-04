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
    /// The BIP39 mnemonic phrase is syntactically or semantically invalid.
    ///
    /// Common causes: wrong word count, a word not in the BIP39 word list,
    /// or an invalid checksum.
    #[error("Invalid mnemonic phrase — please check each word and its position. Detail: {0}")]
    InvalidMnemonic(String),

    /// The wallet does not hold enough confirmed satoshis to fund the send.
    ///
    /// `available` is the current confirmed balance; `required` is the
    /// minimum needed (send amount, before fees).
    #[error(
        "Insufficient confirmed balance: the wallet holds {available} sats \
         but {required} sats are required."
    )]
    InsufficientFunds { available: u64, required: u64 },

    /// The recipient address string could not be parsed or belongs to the
    /// wrong network.
    #[error("Invalid recipient address — {0}")]
    InvalidAddress(String),

    /// A network or Electrum backend error occurred.
    ///
    /// These are typically transient (server unreachable, timeout) but some
    /// indicate configuration problems (wrong Electrum URL).
    #[error("Network error — {0}")]
    NetworkError(String),

    /// The transaction could not be signed or finalized.
    ///
    /// This usually means the wallet descriptor does not control the UTXOs
    /// being spent, or the signing key is missing.
    #[error("Transaction signing failed — {0}")]
    SigningFailed(String),

    /// A transaction output is below the P2WPKH dust limit (546 sats).
    ///
    /// Bitcoin nodes will not relay dust outputs; reduce the send amount or
    /// choose a higher value so that the change output is above the limit.
    #[error(
        "Transaction output of {value} sats is below the 546-sat dust limit. \
         Increase the send amount or use send-max."
    )]
    DustOutput { value: u64 },

    /// The computed transaction fee exceeds 0.1 BTC (10,000,000 sats).
    ///
    /// This is a safety cap to prevent accidental overpayment. Lower the
    /// fee rate or ensure the UTXO set is correctly synced.
    #[error(
        "Computed fee of {fee_sats} sats exceeds the 10,000,000-sat safety ceiling. \
         Lower the fee rate or verify your UTXO set."
    )]
    FeeTooHigh { fee_sats: u64 },
}

/// Generate a new 12-word English BIP39 mnemonic.
///
/// # Errors
///
/// Returns [`WalletError::InvalidMnemonic`] if the BDK random-number
/// generator fails (extremely unlikely in practice).
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

/// Create an in-memory BIP84 wallet from a mnemonic phrase.
///
/// # Arguments
///
/// * `mnemonic` — A valid BIP39 mnemonic phrase (12 or 24 words).
/// * `network` — Bitcoin network for descriptors and address encoding.
///
/// # Errors
///
/// Returns [`WalletError::InvalidMnemonic`] if the phrase cannot be parsed
/// or the extended private key cannot be derived.
///
/// Returns [`WalletError::NetworkError`] if the BIP84 descriptor templates
/// cannot be built or the BDK wallet cannot be constructed.
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
/// * `wallet` — In-memory wallet to update.
/// * `network` — Network used to choose the Electrum endpoint.
///
/// # Errors
///
/// Returns [`WalletError::NetworkError`] if the Electrum connection fails or
/// the wallet sync request times out.
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

/// Return the wallet balance from the local BDK in-memory database.
///
/// # Arguments
///
/// * `wallet` — Wallet whose cached balance should be read.
///
/// # Errors
///
/// Returns [`WalletError::NetworkError`] if the BDK database query fails
/// (should not occur with `MemoryDatabase` under normal conditions).
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
/// * `wallet` — Wallet whose external keychain should advance.
///
/// # Errors
///
/// Returns [`WalletError::NetworkError`] if the BDK key derivation or
/// database write fails.
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
