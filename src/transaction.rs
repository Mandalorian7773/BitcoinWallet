use anyhow::{bail, Context, Result};
use bdk::{
    bitcoin::{Address, Network, Txid},
    blockchain::{Blockchain, ElectrumBlockchain},
    database::MemoryDatabase,
    wallet::tx_builder::TxOrdering,
    FeeRate, SignOptions, Wallet,
};
use std::str::FromStr;

pub fn send(
    wallet: &Wallet<MemoryDatabase>,
    blockchain: &ElectrumBlockchain,
    to_address: &str,
    amount_sats: u64,
    fee_rate: f32,
    network: Network,
) -> Result<Txid> {
    if fee_rate < 1.0 {
        eprintln!("Warning: fee rate {fee_rate} sat/vbyte is below 1 sat/vbyte and may not be relayed.");
    }

    let balance = wallet.get_balance().context("Failed to fetch balance")?;
    if amount_sats > balance.confirmed {
        bail!(
            "Insufficient confirmed balance: have {} sats, want to send {} sats",
            balance.confirmed,
            amount_sats
        );
    }

    let recipient = Address::from_str(to_address)
        .context("Invalid recipient address")?
        .require_network(network)
        .context("Address is for the wrong network")?;

    let mut tx_builder = wallet.build_tx();
    tx_builder
        .add_recipient(recipient.script_pubkey(), amount_sats)
        .fee_rate(FeeRate::from_sat_per_vb(fee_rate))
        .ordering(TxOrdering::Shuffle);

    let (mut psbt, _details) = tx_builder.finish().context("Failed to build transaction")?;

    wallet
        .sign(&mut psbt, SignOptions::default())
        .context("Failed to sign transaction")?;

    let tx = psbt.extract_tx();
    let txid = tx.txid();

    blockchain
        .broadcast(&tx)
        .context("Failed to broadcast transaction")?;

    Ok(txid)
}
