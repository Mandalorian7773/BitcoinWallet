mod common;

use bdk::{bitcoin::Network, wallet::AddressIndex};
use bitcoin_wallet::{
    transaction,
    wallet::{self, WalletError},
};
use common::funded_regtest_wallet;
use proptest::prelude::*;

// ── Existing tests (preserved) ────────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn proptest_random_mnemonic_words_rejected(words in prop::collection::vec("[a-z]{1,12}", 12)) {
        let phrase = words.join(" ");
        let first = wallet::create_wallet(&phrase, Network::Regtest);

        if let Ok(first_wallet) = first {
            let second_wallet = wallet::create_wallet(&phrase, Network::Regtest)
                .map_err(|e| TestCaseError::fail(format!("valid mnemonic became invalid: {e}")))?;
            let first_address = wallet::get_new_address(&first_wallet)
                .map_err(|e| TestCaseError::fail(format!("derive first address: {e}")))?;
            let second_address = wallet::get_new_address(&second_wallet)
                .map_err(|e| TestCaseError::fail(format!("derive second address: {e}")))?;

            prop_assert_eq!(first_address, second_address);
        }
    }

    #[test]
    fn proptest_fee_rate_always_positive(fee_rate in any::<f32>()) {
        let (wallet, blockchain) = funded_regtest_wallet(100_000)
            .map_err(|e| TestCaseError::fail(format!("fund wallet: {e}")))?;
        let recipient_wallet = wallet::create_wallet(
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            Network::Regtest,
        ).map_err(|e| TestCaseError::fail(format!("create recipient wallet: {e}")))?;
        let recipient = recipient_wallet
            .get_address(AddressIndex::New)
            .map_err(|e| TestCaseError::fail(format!("derive recipient address: {e}")))?
            .address
            .to_string();

        let result = transaction::send(
            &wallet,
            &blockchain,
            &recipient,
            10_000,
            fee_rate,
            Network::Regtest,
            false,
        );

        if !fee_rate.is_finite() || !(1.0..=10_000.0_f32).contains(&fee_rate) {
            prop_assert!(result.is_err());
        }
    }
}

// ── Dimension 1: Adversarial proptest cases ───────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// Sending amount == confirmed balance is always an error (fees consume
    /// some sats on top of the send amount) and must never panic.
    #[test]
    fn proptest_send_amount_at_exact_balance(balance in 10_000u64..500_000u64) {
        let (wallet, blockchain) = funded_regtest_wallet(balance)
            .map_err(|e| TestCaseError::fail(format!("fund wallet: {e}")))?;
        let recipient_wallet = wallet::create_wallet(
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            Network::Regtest,
        ).map_err(|e| TestCaseError::fail(format!("create recipient: {e}")))?;
        let recipient = recipient_wallet
            .get_address(AddressIndex::New)
            .map_err(|e| TestCaseError::fail(format!("derive recipient address: {e}")))?
            .address
            .to_string();

        // Sending exactly the balance should always fail because fees must
        // come from the wallet and there are no additional UTXOs.
        let result = transaction::send(
            &wallet,
            &blockchain,
            &recipient,
            balance,   // amount == total UTXO value
            1.0,
            Network::Regtest,
            false,
        );

        // Must always be an error — never a panic (test itself panicking = failure).
        prop_assert!(result.is_err(), "expected Err, got Ok");
    }

    /// Sending balance + 1 satoshi must always return InsufficientFunds and
    /// never panic.
    #[test]
    fn proptest_send_amount_one_above_balance(balance in 1_000u64..500_000u64) {
        let (wallet, blockchain) = funded_regtest_wallet(balance)
            .map_err(|e| TestCaseError::fail(format!("fund wallet: {e}")))?;
        let recipient_wallet = wallet::create_wallet(
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            Network::Regtest,
        ).map_err(|e| TestCaseError::fail(format!("create recipient: {e}")))?;
        let recipient = recipient_wallet
            .get_address(AddressIndex::New)
            .map_err(|e| TestCaseError::fail(format!("derive recipient address: {e}")))?
            .address
            .to_string();

        let result = transaction::send(
            &wallet,
            &blockchain,
            &recipient,
            balance.saturating_add(1),
            1.0,
            Network::Regtest,
            false,
        );

        prop_assert!(
            matches!(result, Err(WalletError::InsufficientFunds { .. })),
            "expected InsufficientFunds, got: {result:?}"
        );
    }

    /// f32::MAX as a fee rate must return Err, never panic or overflow.
    #[test]
    fn proptest_fee_rate_at_f32_max(_dummy in 0u8..1u8) {
        let (wallet, blockchain) = funded_regtest_wallet(100_000)
            .map_err(|e| TestCaseError::fail(format!("fund wallet: {e}")))?;
        let recipient_wallet = wallet::create_wallet(
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            Network::Regtest,
        ).map_err(|e| TestCaseError::fail(format!("create recipient: {e}")))?;
        let recipient = recipient_wallet
            .get_address(AddressIndex::New)
            .map_err(|e| TestCaseError::fail(format!("derive recipient: {e}")))?
            .address
            .to_string();

        let result = transaction::send(
            &wallet,
            &blockchain,
            &recipient,
            10_000,
            f32::MAX,
            Network::Regtest,
            false,
        );

        prop_assert!(result.is_err(), "f32::MAX fee rate must be rejected, got Ok");
    }

    /// f32::NAN as a fee rate must return Err before reaching BDK, never panic.
    #[test]
    fn proptest_fee_rate_nan(_dummy in 0u8..1u8) {
        let (wallet, blockchain) = funded_regtest_wallet(100_000)
            .map_err(|e| TestCaseError::fail(format!("fund wallet: {e}")))?;
        let recipient_wallet = wallet::create_wallet(
            "legal winner thank year wave sausage worth useful legal winner thank yellow",
            Network::Regtest,
        ).map_err(|e| TestCaseError::fail(format!("create recipient: {e}")))?;
        let recipient = recipient_wallet
            .get_address(AddressIndex::New)
            .map_err(|e| TestCaseError::fail(format!("derive recipient: {e}")))?
            .address
            .to_string();

        let result = transaction::send(
            &wallet,
            &blockchain,
            &recipient,
            10_000,
            f32::NAN,
            Network::Regtest,
            false,
        );

        prop_assert!(result.is_err(), "NaN fee rate must be rejected, got Ok");
        // Specifically: the guard fires before BDK touches the value.
        prop_assert!(
            matches!(result, Err(WalletError::NetworkError(_))),
            "expected NetworkError for NaN rate, got: {result:?}"
        );
    }

    /// An arbitrary ASCII string as an address must return InvalidAddress,
    /// never panic.
    #[test]
    fn proptest_arbitrary_recipient_string(addr in "[ -~]{0,200}") {
        let (wallet, blockchain) = funded_regtest_wallet(100_000)
            .map_err(|e| TestCaseError::fail(format!("fund wallet: {e}")))?;

        let result = transaction::send(
            &wallet,
            &blockchain,
            &addr,
            10_000,
            1.0,
            Network::Regtest,
            false,
        );

        // The address is almost certainly invalid; if it somehow parses for the
        // wrong network that is also caught.  In either case: must be Err,
        // never Ok, never panic.
        prop_assert!(result.is_err(), "arbitrary address must be rejected, got Ok for: {addr:?}");
    }

    /// Mnemonics with wrong word counts (1–11, 13–23, 25–50) must all return
    /// `InvalidMnemonic`, never panic.
    #[test]
    fn proptest_mnemonic_wrong_word_count(
        n in prop_oneof![
            1usize..12usize,
            13usize..24usize,
            25usize..51usize,
        ]
    ) {
        // Build a phrase with `n` known-good BIP39 words but the wrong count.
        let base = "abandon ";
        let phrase = base.repeat(n).trim().to_string();

        let result = wallet::create_wallet(&phrase, Network::Regtest);

        prop_assert!(
            matches!(result, Err(WalletError::InvalidMnemonic(_))),
            "expected InvalidMnemonic for {n}-word phrase, got: {result:?}"
        );
    }

    /// For any valid BIP39 mnemonic, deriving the first address twice must
    /// always produce identical results (deterministic key derivation).
    #[test]
    fn proptest_mnemonic_valid_entropy_always_deterministic(words in prop::collection::vec("[a-z]{1,12}", 12)) {
        let phrase = words.join(" ");

        // Only run the determinism check if the phrase happens to be valid.
        if let Ok(w1) = wallet::create_wallet(&phrase, Network::Regtest) {
            let w2 = wallet::create_wallet(&phrase, Network::Regtest)
                .map_err(|e| TestCaseError::fail(format!("second create_wallet failed: {e}")))?;

            let addr1 = wallet::get_new_address(&w1)
                .map_err(|e| TestCaseError::fail(format!("derive addr 1: {e}")))?;
            let addr2 = wallet::get_new_address(&w2)
                .map_err(|e| TestCaseError::fail(format!("derive addr 2: {e}")))?;

            prop_assert_eq!(
                addr1.to_string(),
                addr2.to_string(),
                "address derivation is not deterministic for phrase: {:?}",
                phrase
            );
        }
    }
}
