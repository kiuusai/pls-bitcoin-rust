mod multisig_errors_handling {
    use bitcoin::secp256k1::rand::thread_rng;
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey, XOnlyPublicKey};
    use bitcoin::{Network, ScriptBuf};
    use indexmap::IndexSet;
    use pls_bitcoin_lib::{Multisig, MultisigData, MultisigError, SpendingData, SpendingError};
    use rstest::rstest;

    struct MultisigConfig {
        quorum: usize,
        share_part_and_arbitrator: bool,
        shared_key: Option<XOnlyPublicKey>,
        expected_error: MultisigErrorKind,
    }

    enum MultisigErrorKind {
        MultisigError(MultisigError),
        SpendingError(SpendingError),
    }

    fn key() -> PublicKey {
        let secp = Secp256k1::new();
        let secret = SecretKey::new(&mut thread_rng());
        PublicKey::from_secret_key(&secp, &secret)
    }

    fn x_only_key() -> XOnlyPublicKey {
        key().x_only_public_key().0
    }

    impl MultisigConfig {
        fn into_data(&self) -> MultisigData {
            let part = if self.share_part_and_arbitrator {
                self.shared_key
                    .expect("overlap case provides its shared key")
            } else {
                x_only_key()
            };
            let arbitrator = if self.share_part_and_arbitrator {
                part
            } else {
                x_only_key()
            };

            MultisigData {
                parts: IndexSet::from([part]),
                arbitrators: IndexSet::from([arbitrator]),
                quorum: self.quorum,
                internal_pubkey: key(),
                network: Network::Regtest,
            }
        }
    }

    fn arbitrator_is_part_config() -> MultisigConfig {
        let shared_key = x_only_key();
        MultisigConfig {
            quorum: 1,
            share_part_and_arbitrator: true,
            shared_key: Some(shared_key),
            expected_error: MultisigErrorKind::MultisigError(MultisigError::ArbitratorIsPart(
                shared_key,
            )),
        }
    }

    #[rstest]
    #[case::zero_quorum(MultisigConfig {
        quorum: 0,
        share_part_and_arbitrator: false,
        shared_key: None,
        expected_error: MultisigErrorKind::MultisigError(MultisigError::QuorumZero),
    })]
    #[case::quorum_exceeds_arbitrators(MultisigConfig {
        quorum: 2,
        share_part_and_arbitrator: false,
        shared_key: None,
        expected_error: MultisigErrorKind::MultisigError(MultisigError::QuorumGreaterThanArbitratorsLength),
    })]
    #[case::arbitrator_is_part(arbitrator_is_part_config())]
    #[case::spending_script_not_in_tree(MultisigConfig {
        quorum: 1,
        share_part_and_arbitrator: false,
        shared_key: None,
        expected_error: MultisigErrorKind::SpendingError(SpendingError::ScriptNotFound),
    })]
    fn rejects_with_given_errors(#[case] config: MultisigConfig) {
        let data = config.into_data();

        match config.expected_error {
            MultisigErrorKind::MultisigError(err) => {
                assert_eq!(Multisig::new(data).expect_err("Expected error"), err);
            }
            MultisigErrorKind::SpendingError(err) => {
                let multisig = Multisig::new(data).unwrap();
                let result = multisig.start_tx_spending(SpendingData {
                    redeem_script: ScriptBuf::new(),
                    utxos: vec![],
                    outs: vec![],
                    lock_time: None,
                });
                assert_eq!(result, Err(err));
            }
        }
    }
}
