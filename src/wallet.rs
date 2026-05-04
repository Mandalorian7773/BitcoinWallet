use anyhow::{Context, Result};
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

use crate::config::electrum_url;

pub fn generate_mnemonic() -> Result<Mnemonic> {
    let generated: GeneratedKey<_, bdk::miniscript::Segwitv0> =
        Mnemonic::generate((WordCount::Words12, Language::English))
            .map_err(|e| anyhow::anyhow!("Mnemonic generation failed: {:?}", e))?;
    Ok(generated.into_key())
}

pub fn create_wallet(mnemonic: &str, network: Network) -> Result<Wallet<MemoryDatabase>> {
    let mnemonic: Mnemonic = mnemonic.parse().context("Invalid mnemonic phrase")?;
    let xkey: ExtendedKey = mnemonic
        .into_extended_key()
        .context("Failed to derive extended key from mnemonic")?;
    let xprv = xkey
        .into_xprv(network)
        .context("Failed to derive xprv — wrong network?")?;

    let external = Bip84(xprv, KeychainKind::External)
        .build(network)
        .context("Failed to build external descriptor")?;
    let internal = Bip84(xprv, KeychainKind::Internal)
        .build(network)
        .context("Failed to build internal descriptor")?;

    Wallet::new(external, Some(internal), network, MemoryDatabase::default())
        .context("Failed to construct BDK wallet")
}

pub fn sync_wallet(wallet: &Wallet<MemoryDatabase>, network: Network) -> Result<ElectrumBlockchain> {
    let url = electrum_url(network);
    let blockchain = ElectrumBlockchain::from_config(&ElectrumBlockchainConfig {
        url: url.to_string(),
        socks5: None,
        retry: 3,
        timeout: Some(15),
        stop_gap: 10,
        validate_domain: true,
    })
    .context("Failed to connect to Electrum server")?;

    wallet
        .sync(&blockchain, SyncOptions::default())
        .context("Wallet sync failed")?;

    Ok(blockchain)
}

pub fn get_balance(wallet: &Wallet<MemoryDatabase>) -> Result<bdk::Balance> {
    wallet.get_balance().context("Failed to retrieve balance")
}

pub fn get_new_address(wallet: &Wallet<MemoryDatabase>) -> Result<Address> {
    wallet
        .get_address(AddressIndex::New)
        .map(|info| info.address)
        .context("Failed to derive new address")
}
