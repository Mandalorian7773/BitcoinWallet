mod config;
mod transaction;
mod wallet;

use anyhow::{Context, Result};
use bdk::bitcoin::Network;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

#[derive(Clone, ValueEnum, Debug)]
enum NetworkArg {
    Testnet,
    Mainnet,
}

impl From<NetworkArg> for Network {
    fn from(n: NetworkArg) -> Self {
        match n {
            NetworkArg::Testnet => Network::Testnet,
            NetworkArg::Mainnet => Network::Bitcoin,
        }
    }
}

#[derive(Parser)]
#[command(name = "wallet", about = "Minimal Bitcoin wallet CLI (BDK 0.29)", version)]
struct Cli {
    #[arg(long, value_enum, default_value = "testnet")]
    network: NetworkArg,

    #[arg(long, help = "Output results as JSON")]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Generate,
    Restore {
        mnemonic: Vec<String>,
    },
    Address {
        #[arg(long)]
        mnemonic: String,
    },
    Balance {
        #[arg(long)]
        mnemonic: String,
    },
    Send {
        #[arg(long)]
        mnemonic: String,
        address: String,
        sats: u64,
        fee_sat_per_vbyte: f32,
    },
    History {
        #[arg(long)]
        mnemonic: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let network: Network = cli.network.into();

    match cli.command {
        Commands::Generate => cmd_generate(network, cli.json),

        Commands::Restore { mnemonic } => {
            let phrase = mnemonic.join(" ");
            cmd_restore(&phrase, network, cli.json)
        }

        Commands::Address { mnemonic } => {
            let w = wallet::create_wallet(&mnemonic, network)?;
            let addr = wallet::get_new_address(&w)?;
            if cli.json {
                println!("{}", json!({ "address": addr.to_string() }));
            } else {
                println!("Address: {addr}");
            }
            Ok(())
        }

        Commands::Balance { mnemonic } => {
            let w = wallet::create_wallet(&mnemonic, network)?;
            let _chain = wallet::sync_wallet(&w, network)?;
            let bal = wallet::get_balance(&w)?;
            if cli.json {
                println!(
                    "{}",
                    json!({
                        "confirmed_sats": bal.confirmed,
                        "unconfirmed_sats": bal.untrusted_pending + bal.trusted_pending,
                        "total_sats": bal.confirmed + bal.untrusted_pending + bal.trusted_pending,
                    })
                );
            } else {
                let unconfirmed = bal.untrusted_pending + bal.trusted_pending;
                println!("Confirmed   : {} sats", bal.confirmed);
                println!("Unconfirmed : {} sats", unconfirmed);
                println!("Total       : {} sats", bal.confirmed + unconfirmed);
            }
            Ok(())
        }

        Commands::Send {
            mnemonic,
            address,
            sats,
            fee_sat_per_vbyte,
        } => {
            let w = wallet::create_wallet(&mnemonic, network)?;
            let chain = wallet::sync_wallet(&w, network)?;
            let txid =
                transaction::send(&w, &chain, &address, sats, fee_sat_per_vbyte, network)?;
            if cli.json {
                println!("{}", json!({ "txid": txid.to_string() }));
            } else {
                println!("Broadcast successful!");
                println!("txid: {txid}");
            }
            Ok(())
        }

        Commands::History { mnemonic } => {
            let w = wallet::create_wallet(&mnemonic, network)?;
            let _chain = wallet::sync_wallet(&w, network)?;
            let txs = w.list_transactions(false).context("Failed to list transactions")?;
            if txs.is_empty() {
                println!("No transactions found.");
                return Ok(());
            }
            if cli.json {
                let entries: Vec<_> = txs
                    .iter()
                    .map(|tx| {
                        let net = tx.received as i64 - tx.sent as i64;
                        json!({
                            "txid": tx.txid.to_string(),
                            "received_sats": tx.received,
                            "sent_sats": tx.sent,
                            "net_sats": net,
                            "confirmation_height": tx.confirmation_time.as_ref().map(|c| c.height),
                            "confirmed": tx.confirmation_time.is_some(),
                        })
                    })
                    .collect();
                println!("{}", json!(entries));
            } else {
                println!("{:<64}  {:>12}  {:>12}  {:>10}  {}", "TXID", "RECEIVED", "SENT", "NET", "STATUS");
                println!("{}", "-".repeat(120));
                for tx in &txs {
                    let net = tx.received as i64 - tx.sent as i64;
                    let status = if tx.confirmation_time.is_some() {
                        "confirmed"
                    } else {
                        "pending"
                    };
                    println!(
                        "{:<64}  {:>12}  {:>12}  {:>+10}  {}",
                        tx.txid, tx.received, tx.sent, net, status
                    );
                }
            }
            Ok(())
        }
    }
}

fn cmd_generate(network: Network, as_json: bool) -> Result<()> {
    let mnemonic = wallet::generate_mnemonic()?;
    let phrase = mnemonic.to_string();
    let w = wallet::create_wallet(&phrase, network)?;
    let addr = wallet::get_new_address(&w)?;
    if as_json {
        println!(
            "{}",
            json!({
                "mnemonic": phrase,
                "first_address": addr.to_string(),
            })
        );
    } else {
        println!("=== New Wallet ===");
        println!("Mnemonic     : {phrase}");
        println!("First address: {addr}");
        println!();
        println!("IMPORTANT: Back up your mnemonic. It is the only way to recover your funds.");
    }
    Ok(())
}

fn cmd_restore(phrase: &str, network: Network, as_json: bool) -> Result<()> {
    let w = wallet::create_wallet(phrase, network)?;
    let addr = wallet::get_new_address(&w)?;
    if as_json {
        println!("{}", json!({ "first_address": addr.to_string() }));
    } else {
        println!("Wallet restored successfully.");
        println!("First address: {addr}");
    }
    Ok(())
}
