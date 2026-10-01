mod multisig_old_compatibility_tests {
    use std::str::FromStr;
    use std::vec;

    use indexmap::IndexSet;
    use pls_bitcoin_lib::{Multisig, MultisigData};

    use bitcoin::secp256k1::{PublicKey, XOnlyPublicKey};
    use rstest::rstest;

    #[rstest]
    fn it_verifies_multisig_creation_matches_old_contracts() {
        let parts_keys_str = vec![
            "bd1bb9ff4f64cd53b57362287949a3f3668cd3b96d876f4dbede888515b552b1",
            "639faf46fba3cf2baf6828b2a80210759de54a20d07693d90eaad3c3dd413652",
        ];

        let arbitrator_pubkey = XOnlyPublicKey::from_str(
            "1d425dce155b582ff5229fafbfec01321a1d8bcd565ed18840679167ca685694",
        )
        .unwrap();

        let parts_keys: IndexSet<bitcoin::secp256k1::XOnlyPublicKey> = parts_keys_str
            .iter()
            .map(|key_str| {
                let pubkey = XOnlyPublicKey::from_str(key_str).unwrap();

                pubkey
            })
            .collect();

        let internal_pubkey = PublicKey::from_str(
            "0250929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0",
        )
        .unwrap();

        let multisig = Multisig::new(MultisigData {
            parts: parts_keys,
            arbitrators: [arbitrator_pubkey].into(),
            quorum: 1,
            internal_pubkey: internal_pubkey,
            network: bitcoin::Network::Testnet,
        })
        .unwrap();

        assert_eq!(
            multisig.address().to_string(),
            "tb1pwzpkqfnsxgj34gxxk2yjn7523jyx52yghdftshsyqej4xpxtllyskvcelt"
        );
    }
}
