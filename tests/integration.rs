mod common;

use assert_cmd::Command;
use bdk::{bitcoin::Network, wallet::AddressIndex};
use bitcoin_wallet::{transaction, wallet};
use common::{funded_regtest_wallet, VALID_MNEMONIC};

#[test]
fn test_full_flow_generate_address_check_balance() -> Result<(), Box<dyn std::error::Error>> {
    let (wallet, _blockchain) = funded_regtest_wallet(75_000)?;
    let address = wallet.get_address(AddressIndex::New)?.address;
    let balance = wallet.get_balance()?;

    assert!(address.to_string().starts_with("bcrt1"));
    assert_eq!(balance.confirmed, 75_000);
    Ok(())
}

#[test]
fn test_full_flow_send_transaction() -> Result<(), Box<dyn std::error::Error>> {
    let (wallet, blockchain) = funded_regtest_wallet(100_000)?;
    let recipient_wallet = wallet::create_wallet(
        "legal winner thank year wave sausage worth useful legal winner thank yellow",
        Network::Regtest,
    )?;
    let recipient = recipient_wallet
        .get_address(AddressIndex::New)?
        .address
        .to_string();

    let txid = transaction::send(
        &wallet,
        &blockchain,
        &recipient,
        10_000,
        1.0,
        Network::Regtest,
        false,
    )?;

    assert_ne!(txid, blockchain.funding_txid());
    assert_eq!(blockchain.broadcast_count(), 1);
    Ok(())
}

#[test]
fn test_history_shows_injected_tx() -> Result<(), Box<dyn std::error::Error>> {
    let (wallet, blockchain) = funded_regtest_wallet(42_000)?;
    let txs = wallet.list_transactions(false)?;

    assert_eq!(txs.len(), 1);
    assert_eq!(txs[0].txid, blockchain.funding_txid());
    assert_eq!(txs[0].received, 42_000);
    Ok(())
}

#[test]
fn test_cli_generate_outputs_mnemonic_and_address() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("wallet")?.arg("generate").output()?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(output.status.success());
    assert!(stdout.contains("First address: tb1"));

    let numbered_words = stdout
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed
                .split_once(". ")
                .is_some_and(|(idx, word)| idx.parse::<usize>().is_ok() && !word.is_empty())
        })
        .count();
    assert!(matches!(numbered_words, 12 | 24));
    Ok(())
}

#[test]
fn test_cli_balance_outputs_structured_data() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("wallet")?
        .args(["--offline", "balance", "--mnemonic", VALID_MNEMONIC])
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;

    assert!(output.status.success());
    assert!(stdout.to_lowercase().contains("confirmed"));
    assert!(stdout.to_lowercase().contains("unconfirmed"));
    assert!(stdout.to_lowercase().contains("total"));
    Ok(())
}

#[test]
fn test_cli_send_missing_args_shows_help() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("wallet")?
        .args(["send", "--mnemonic", VALID_MNEMONIC])
        .output()?;
    let stderr = String::from_utf8(output.stderr)?;

    assert!(!output.status.success());
    assert!(stderr.contains("Usage:") || stderr.contains("required"));
    Ok(())
}
