mod common;

use bdk::{bitcoin::Network, wallet::AddressIndex};
use bitcoin_wallet::{transaction, wallet};
use common::funded_regtest_wallet;
use proptest::prelude::*;

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

        if !fee_rate.is_finite() || !(1.0..=10_000.0).contains(&fee_rate) {
            prop_assert!(result.is_err());
        }
    }
}
