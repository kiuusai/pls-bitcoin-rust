mod multisig_integration_tests {
    use std::{assert_eq, println, vec};

    use bitcoin::script::Builder;
    use bitcoin::taproot::{TapTree, TaprootBuilder};
    use pls_bitcoin_lib::multisig::{Multisig, MultisigOptions};
    use pls_bitcoin_lib::utils;

    use bitcoin::key::Keypair;
    use bitcoin::secp256k1::{Secp256k1, SecretKey, PublicKey};
    use bitcoin::{opcodes, Address, KnownHrp, ScriptBuf, XOnlyPublicKey};

    #[test]
    fn it_verifies_multisig_creation() {
        let secp = Secp256k1::new();

        let mut parts_keypairs: Vec<Keypair> = Vec::new();

        for x in 0..2 {
            let mut secret_slice = [0x0; 32];
            secret_slice.fill(x);

            let secret_key = SecretKey::from_slice(&[0x01; 32]).unwrap();
            let keypair = Keypair::from_secret_key(&secp, &secret_key);

            parts_keypairs.push(keypair);
        }

        let secret_slice = [0x2; 32];
        let secret_key = SecretKey::from_slice(&secret_slice).unwrap();
        let keypair = Keypair::from_secret_key(&secp, &secret_key);

        let arbitrators: Vec<Keypair> = vec![keypair];

        let secret_slice = [0x3; 32];
        let secret_key = SecretKey::from_slice(&secret_slice).unwrap();
        let internal_pubkey = Keypair::from_secret_key(&secp, &secret_key).public_key();

        let network = KnownHrp::Regtest;
        let quorum = 1;

        let multisig = Multisig::new(MultisigOptions {
            parts: parts_keypairs
                .iter()
                .map(|part| part.public_key())
                .collect(),
            quorum,
            arbitrators: arbitrators
                .iter()
                .map(|arbitrator| arbitrator.public_key())
                .collect(),
            internal_pubkey,
            network,
        });

        assert_eq!(
            internal_pubkey.x_only_public_key().0,
            multisig.internal_key(),
        );

        let mut combinations: Vec<Vec<Keypair>> = vec![parts_keypairs.clone()];

        parts_keypairs.into_iter().for_each(|part| {
            let mut arbitrators_combinations = utils::combine(&arbitrators, quorum);

            arbitrators_combinations.iter_mut().for_each(|combination| {
                let mut new_combination = vec![part];
                new_combination.append(combination);

                combinations.push(new_combination);
            });
        });

        let mut scripts: Vec<ScriptBuf> = Vec::new();

        combinations.iter().for_each(|combination| {
            let mut builder = Builder::new();

            let mut first_combination = true;

            for key in combination.iter() {
                let xonly_key = XOnlyPublicKey::from_keypair(key).0;

                builder = builder.push_x_only_key(&xonly_key);

                builder = builder.push_opcode(if first_combination {
                    opcodes::all::OP_CHECKSIG
                } else {
                    opcodes::all::OP_CHECKSIGADD
                });

                first_combination = false;
            }

            builder = builder.push_int(combination.len() as i64);

            builder = builder.push_opcode(opcodes::all::OP_NUMEQUAL);

            let script = builder.into_script();

            scripts.push(script);
        });

        let multisig_scripts = multisig.scripts();

        assert_eq!(scripts.len(), multisig_scripts.len());

        multisig_scripts
            .iter()
            .enumerate()
            .for_each(|(i, multisig_script)| {
                let script = scripts[i].clone();

                assert_eq!(script.to_asm_string(), multisig_script.leaf.to_asm_string());
                assert_eq!(multisig_scripts.len() - i, multisig_script.weight);

                let combination: Vec<PublicKey> = combinations[i]
                    .clone()
                    .iter()
                    .map(|keypair| keypair.public_key())
                    .collect();

                assert_eq!(combination, multisig_script.combination);
            });

        let script_tree = TapTree::try_from(
            TaprootBuilder::with_huffman_tree(
                scripts
                    .iter()
                    .enumerate()
                    .map(|(i, script)| ((scripts.len() - i) as u32, script.clone())),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(script_tree, multisig.script_tree());

        let address = Address::p2tr(
            &secp,
            internal_pubkey.x_only_public_key().0,
            Some(script_tree.root_hash()),
            network,
        );

        assert_eq!(address, multisig.address());

        println!("Bitcoin address: {}", multisig.address().to_string());
    }
}
