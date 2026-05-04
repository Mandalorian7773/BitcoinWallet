use bdk::{
    bitcoin::{psbt::PartiallySignedTransaction as Psbt, Address, Network, Txid},
    blockchain::Blockchain,
    database::MemoryDatabase,
    wallet::tx_builder::TxOrdering,
    FeeRate, SignOptions, Wallet,
};
use std::str::FromStr;

use crate::wallet::WalletError;

/// Build, sign, and broadcast a transaction.
///
/// # Arguments
///
/// * `wallet` - Synced wallet holding spendable UTXOs.
/// * `blockchain` - BDK blockchain backend used for broadcasting.
/// * `to_address` - Recipient address string.
/// * `amount_sats` - Amount to send in satoshis.
/// * `fee_rate` - Fee rate in sat/vbyte.
/// * `network` - Wallet network used for address validation.
/// * `confirm_large_mainnet_send` - Required for mainnet sends above 1,000,000 sats.
///
/// # Returns
///
/// The broadcast transaction id.
///
/// # Example
///
/// ```no_run
/// use bdk::bitcoin::Network;
///
/// # fn example<B: bdk::blockchain::Blockchain>(
/// #     wallet: &bdk::Wallet<bdk::database::MemoryDatabase>,
/// #     blockchain: &B,
/// # ) -> Result<(), bitcoin_wallet::wallet::WalletError> {
/// let txid = bitcoin_wallet::transaction::send(
///     wallet,
///     blockchain,
///     "tb1qexample000000000000000000000000000000000",
///     1_000,
///     1.0,
///     Network::Testnet,
///     false,
/// )?;
/// println!("{txid}");
/// # Ok(())
/// # }
/// ```
pub fn send<B: Blockchain>(
    wallet: &Wallet<MemoryDatabase>,
    blockchain: &B,
    to_address: &str,
    amount_sats: u64,
    fee_rate: f32,
    network: Network,
    confirm_large_mainnet_send: bool,
) -> Result<Txid, WalletError> {
    let psbt = build_signed_psbt(
        wallet,
        to_address,
        amount_sats,
        fee_rate,
        network,
        confirm_large_mainnet_send,
    )?;

    let tx = psbt.extract_tx();
    let txid = tx.txid();

    blockchain
        .broadcast(&tx)
        .map_err(|e| WalletError::NetworkError(format!("broadcast transaction {txid}: {e}")))?;

    Ok(txid)
}

fn build_signed_psbt(
    wallet: &Wallet<MemoryDatabase>,
    to_address: &str,
    amount_sats: u64,
    fee_rate: f32,
    network: Network,
    confirm_large_mainnet_send: bool,
) -> Result<Psbt, WalletError> {
    if amount_sats == 0 {
        return Err(WalletError::InsufficientFunds {
            available: 0,
            required: 1,
        });
    }

    if !fee_rate.is_finite() || fee_rate < 1.0 {
        return Err(WalletError::NetworkError(format!(
            "fee rate {fee_rate} sat/vbyte is below the 1 sat/vbyte relay minimum"
        )));
    }
    if fee_rate > 10_000.0 {
        // WHY: absurd fee rates can overflow BDK's fee arithmetic before coin
        // selection returns a normal insufficient-funds error.
        return Err(WalletError::NetworkError(format!(
            "fee rate {fee_rate} sat/vbyte is above the supported safety limit"
        )));
    }

    if network == Network::Bitcoin && amount_sats > 1_000_000 && !confirm_large_mainnet_send {
        // WHY: a minimal CLI has no policy engine, so large mainnet sends need
        // an explicit human confirmation flag before transaction construction.
        return Err(WalletError::NetworkError(
            "mainnet sends above 1,000,000 sats require --confirm".to_string(),
        ));
    }

    let recipient = Address::from_str(to_address)
        .map_err(|e| WalletError::InvalidAddress(e.to_string()))?
        .require_network(network)
        .map_err(|e| WalletError::InvalidAddress(format!("address network mismatch: {e}")))?;

    // WHY: validating the address network before adding the script prevents
    // accidental cross-network sends that would otherwise look syntactically valid.
    let balance = wallet
        .get_balance()
        .map_err(|e| WalletError::NetworkError(format!("fetch wallet balance: {e}")))?;
    if amount_sats > balance.confirmed {
        return Err(WalletError::InsufficientFunds {
            available: balance.confirmed,
            required: amount_sats,
        });
    }

    let mut tx_builder = wallet.build_tx();
    tx_builder
        .add_recipient(recipient.script_pubkey(), amount_sats)
        .fee_rate(FeeRate::from_sat_per_vb(fee_rate))
        .ordering(TxOrdering::Shuffle);

    let (mut psbt, _details) = tx_builder
        .finish()
        .map_err(|e| WalletError::NetworkError(format!("build transaction: {e}")))?;

    let finalized = wallet
        .sign(&mut psbt, SignOptions::default())
        .map_err(|e| WalletError::SigningFailed(format!("sign transaction: {e}")))?;

    if !finalized {
        return Err(WalletError::SigningFailed(
            "wallet did not finalize all PSBT inputs".to_string(),
        ));
    }

    Ok(psbt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::create_wallet;
    use bdk::{
        bitcoin::{
            hashes::Hash, BlockHash, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Witness,
        },
        blockchain::{
            Blockchain, Capability, GetBlockHash, GetHeight, GetTx, Progress, WalletSync,
        },
        database::{BatchDatabase, SyncTime},
        wallet::AddressIndex,
        BlockTime, Error, KeychainKind,
    };
    use std::{cell::RefCell, collections::HashSet};

    const VALID_MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    fn funded_wallet(value: u64) -> Result<Wallet<MemoryDatabase>, WalletError> {
        let wallet = create_wallet(VALID_MNEMONIC, Network::Testnet)?;
        let address = wallet
            .get_address(AddressIndex::New)
            .map_err(|e| WalletError::NetworkError(format!("derive funding address: {e}")))?
            .address;
        let tx = funding_tx(address.script_pubkey(), value);
        wallet
            .sync(&FundedBlockchain { tx, value }, bdk::SyncOptions::default())
            .map_err(|e| WalletError::NetworkError(format!("sync funded wallet: {e}")))?;

        Ok(wallet)
    }

    struct FundedBlockchain {
        tx: Transaction,
        value: u64,
    }

    impl WalletSync for FundedBlockchain {
        fn wallet_setup<D: BatchDatabase>(
            &self,
            database: &RefCell<D>,
            _progress_update: Box<dyn Progress>,
        ) -> Result<(), Error> {
            let outpoint = OutPoint {
                txid: self.tx.txid(),
                vout: 0,
            };
            let mut db = database.borrow_mut();
            db.set_raw_tx(&self.tx)?;
            db.set_tx(&bdk::TransactionDetails {
                transaction: Some(self.tx.clone()),
                txid: self.tx.txid(),
                received: self.value,
                sent: 0,
                fee: None,
                confirmation_time: Some(BlockTime {
                    height: 1,
                    timestamp: 1,
                }),
            })?;
            db.set_utxo(&bdk::LocalUtxo {
                outpoint,
                txout: self.tx.output[0].clone(),
                keychain: KeychainKind::External,
                is_spent: false,
            })?;
            db.set_sync_time(SyncTime {
                block_time: BlockTime {
                    height: 2,
                    timestamp: 2,
                },
            })?;
            Ok(())
        }
    }

    impl GetHeight for FundedBlockchain {
        fn get_height(&self) -> Result<u32, Error> {
            Ok(2)
        }
    }

    impl GetTx for FundedBlockchain {
        fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
            Ok((*txid == self.tx.txid()).then(|| self.tx.clone()))
        }
    }

    impl GetBlockHash for FundedBlockchain {
        fn get_block_hash(&self, _height: u64) -> Result<BlockHash, Error> {
            Ok(BlockHash::all_zeros())
        }
    }

    impl Blockchain for FundedBlockchain {
        fn get_capabilities(&self) -> HashSet<Capability> {
            HashSet::new()
        }

        fn broadcast(&self, _tx: &Transaction) -> Result<(), Error> {
            Ok(())
        }

        fn estimate_fee(&self, _target: usize) -> Result<FeeRate, Error> {
            Ok(FeeRate::from_sat_per_vb(1.0))
        }
    }

    fn funding_tx(script_pubkey: ScriptBuf, value: u64) -> Transaction {
        Transaction {
            version: 2,
            lock_time: bdk::bitcoin::absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint {
                    txid: Txid::from_byte_array([1; 32]),
                    vout: 0,
                },
                script_sig: ScriptBuf::new(),
                sequence: bdk::bitcoin::Sequence::MAX,
                witness: Witness::default(),
            }],
            output: vec![TxOut {
                value,
                script_pubkey,
            }],
        }
    }

    #[test]
    fn test_send_rejects_zero_amount() -> Result<(), WalletError> {
        let wallet = funded_wallet(50_000)?;
        let address = wallet
            .get_address(AddressIndex::New)
            .map_err(|e| WalletError::NetworkError(format!("derive recipient address: {e}")))?;

        assert!(matches!(
            build_signed_psbt(
                &wallet,
                &address.address.to_string(),
                0,
                1.0,
                Network::Testnet,
                false
            ),
            Err(WalletError::InsufficientFunds { .. })
        ));
        Ok(())
    }

    #[test]
    fn test_send_rejects_amount_exceeding_balance() -> Result<(), WalletError> {
        let wallet = funded_wallet(10_000)?;
        let address = wallet
            .get_address(AddressIndex::New)
            .map_err(|e| WalletError::NetworkError(format!("derive recipient address: {e}")))?;
        let result = build_signed_psbt(
            &wallet,
            &address.address.to_string(),
            20_000,
            1.0,
            Network::Testnet,
            false,
        );

        assert!(matches!(
            result,
            Err(WalletError::InsufficientFunds {
                available: 10_000,
                required: 20_000
            })
        ));
        Ok(())
    }

    #[test]
    fn test_send_rejects_invalid_address() -> Result<(), WalletError> {
        let wallet = funded_wallet(50_000)?;

        assert!(matches!(
            build_signed_psbt(
                &wallet,
                "malformed-address",
                1_000,
                1.0,
                Network::Testnet,
                false
            ),
            Err(WalletError::InvalidAddress(_))
        ));
        Ok(())
    }

    #[test]
    fn test_send_rejects_fee_below_minimum() -> Result<(), WalletError> {
        let wallet = funded_wallet(50_000)?;
        let address = wallet
            .get_address(AddressIndex::New)
            .map_err(|e| WalletError::NetworkError(format!("derive recipient address: {e}")))?;

        assert!(matches!(
            build_signed_psbt(
                &wallet,
                &address.address.to_string(),
                1_000,
                0.5,
                Network::Testnet,
                false
            ),
            Err(WalletError::NetworkError(_))
        ));
        Ok(())
    }

    #[test]
    fn test_build_tx_has_change_output() -> Result<(), WalletError> {
        let wallet = funded_wallet(100_000)?;
        let recipient_wallet = create_wallet(
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            Network::Testnet,
        )?;
        let recipient = recipient_wallet
            .get_address(AddressIndex::New)
            .map_err(|e| WalletError::NetworkError(format!("derive recipient address: {e}")))?
            .address;

        let psbt = build_signed_psbt(
            &wallet,
            &recipient.to_string(),
            10_000,
            1.0,
            Network::Testnet,
            false,
        )?;

        assert_eq!(psbt.unsigned_tx.output.len(), 2);
        Ok(())
    }
}
