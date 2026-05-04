use bdk::{
    bitcoin::{
        absolute::LockTime, hashes::Hash, BlockHash, Network, OutPoint, ScriptBuf, Transaction,
        TxIn, TxOut, Txid, Witness,
    },
    blockchain::{Blockchain, Capability, GetBlockHash, GetHeight, GetTx, Progress, WalletSync},
    database::{BatchDatabase, SyncTime},
    wallet::AddressIndex,
    BlockTime, FeeRate, KeychainKind, SyncOptions,
};
use bitcoin_wallet::{wallet, wallet::WalletError};
use std::{cell::RefCell, collections::HashSet, sync::Mutex};

pub const VALID_MNEMONIC: &str =
    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

pub struct MockBlockchain {
    tx: Transaction,
    value: u64,
    broadcasts: Mutex<Vec<Txid>>,
}

impl MockBlockchain {
    pub fn new(script_pubkey: ScriptBuf, value: u64) -> Self {
        let tx = Transaction {
            version: 2,
            lock_time: LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint {
                    txid: Txid::from_byte_array([2; 32]),
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
        };

        Self {
            tx,
            value,
            broadcasts: Mutex::new(Vec::new()),
        }
    }

    #[allow(dead_code)]
    pub fn funding_txid(&self) -> Txid {
        self.tx.txid()
    }

    #[allow(dead_code)]
    pub fn broadcast_count(&self) -> usize {
        self.broadcasts
            .lock()
            .map_or(0, |broadcasts| broadcasts.len())
    }
}

impl WalletSync for MockBlockchain {
    fn wallet_setup<D: BatchDatabase>(
        &self,
        database: &RefCell<D>,
        _progress_update: Box<dyn Progress>,
    ) -> Result<(), bdk::Error> {
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

impl GetHeight for MockBlockchain {
    fn get_height(&self) -> Result<u32, bdk::Error> {
        Ok(2)
    }
}

impl GetTx for MockBlockchain {
    fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, bdk::Error> {
        Ok((*txid == self.tx.txid()).then(|| self.tx.clone()))
    }
}

impl GetBlockHash for MockBlockchain {
    fn get_block_hash(&self, _height: u64) -> Result<BlockHash, bdk::Error> {
        Ok(BlockHash::all_zeros())
    }
}

impl Blockchain for MockBlockchain {
    fn get_capabilities(&self) -> HashSet<Capability> {
        HashSet::new()
    }

    fn broadcast(&self, tx: &Transaction) -> Result<(), bdk::Error> {
        if let Ok(mut broadcasts) = self.broadcasts.lock() {
            broadcasts.push(tx.txid());
        }
        Ok(())
    }

    fn estimate_fee(&self, _target: usize) -> Result<FeeRate, bdk::Error> {
        Ok(FeeRate::from_sat_per_vb(1.0))
    }
}

pub fn funded_regtest_wallet(
    value: u64,
) -> Result<(bdk::Wallet<bdk::database::MemoryDatabase>, MockBlockchain), WalletError> {
    let wallet = wallet::create_wallet(VALID_MNEMONIC, Network::Regtest)?;
    let address = wallet
        .get_address(AddressIndex::New)
        .map_err(|e| WalletError::NetworkError(format!("derive regtest address: {e}")))?
        .address;
    let blockchain = MockBlockchain::new(address.script_pubkey(), value);
    wallet
        .sync(&blockchain, SyncOptions::default())
        .map_err(|e| WalletError::NetworkError(format!("sync mock blockchain: {e}")))?;
    Ok((wallet, blockchain))
}
