mod multisig_errors_handling {
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use bitcoin::{Network, ScriptBuf};
    use indexmap::IndexSet;
    use pls_bitcoin_lib::{Multisig, MultisigData, MultisigError, SpendingData, SpendingError};
    use rstest::{fixture, rstest};

    fn key(byte: u8) -> PublicKey {
        let secp = Secp256k1::new();
        let secret = SecretKey::from_slice(&[byte; 32]).unwrap();
        PublicKey::from_secret_key(&secp, &secret)
    }

    #[fixture]
    fn multisig_data() -> MultisigData {
        MultisigData {
            parts: IndexSet::from([key(1).x_only_public_key().0]),
            arbitrators: IndexSet::from([key(2).x_only_public_key().0]),
            quorum: 1,
            internal_pubkey: key(3),
            network: Network::Regtest,
        }
    }

    #[fixture]
    fn multisig_data_with_zero_quorum() -> MultisigData {
        let mut data = multisig_data();
        data.quorum = 0;
        data
    }

    #[fixture]
    fn multisig_data_with_arbitrator_also_in_parts() -> MultisigData {
        let part = key(6).x_only_public_key().0;
        MultisigData {
            parts: IndexSet::from([part]),
            arbitrators: IndexSet::from([part]),
            quorum: 1,
            internal_pubkey: key(7),
            network: Network::Regtest,
        }
    }

    #[rstest]
    fn rejects_zero_quorum(multisig_data_with_zero_quorum: MultisigData) {
        assert!(matches!(
            Multisig::new(multisig_data_with_zero_quorum),
            Err(MultisigError::QuorumZero)
        ));
    }

    #[rstest]
    fn rejects_arbitrator_xonly_key_that_is_also_a_part(
        multisig_data_with_arbitrator_also_in_parts: MultisigData,
    ) {
        assert!(matches!(
            Multisig::new(multisig_data_with_arbitrator_also_in_parts),
            Err(MultisigError::ArbitratorIsPart(_))
        ));
    }

    #[rstest]
    fn returns_error_when_spending_script_is_not_in_taproot_tree(multisig_data: MultisigData) {
        let multisig = Multisig::new(multisig_data).unwrap();
        let result = multisig.start_tx_spending(SpendingData {
            redeem_script: ScriptBuf::new(),
            utxos: vec![],
            outs: vec![],
            lock_time: None,
        });

        assert_eq!(result, Err(SpendingError::ScriptNotFound));
    }
}
