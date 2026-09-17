#[suitest::suite(multisig_e2e_tests)]
#[suitest::suite_cfg(sequential = false)]
mod multisig_e2e_tests {
    use std::{assert_eq, env, println, vec};

    use pls_bitcoin_lib::multisig::{Multisig, MultisigData, Utxo};
    use pls_bitcoin_lib::{utils, SpendingData};

    use bitcoin::key::rand::thread_rng;
    use bitcoin::key::Keypair;
    use bitcoin::script::Builder;
    use bitcoin::secp256k1::{Message, PublicKey, Secp256k1, SecretKey};
    use bitcoin::sighash::{Prevouts, SighashCache};
    use bitcoin::taproot::{self, LeafVersion, TapTree, TaprootBuilder};
    use bitcoin::{
        opcodes, Address, Amount, Network, OutPoint, PrivateKey, ScriptBuf, TapLeafHash,
        TapSighashType, TxOut, Witness, XOnlyPublicKey,
    };
    use bitcoincore_rpc::json::EstimateMode;
    use bitcoincore_rpc::{Auth, Client, RpcApi};
    use dotenv::dotenv;
    use suitest::before_all;

    #[derive(Debug, Clone)]
    struct TestConfig {
        node_url: String,
        user: String,
        pass: String,
    }

    #[before_all]
    fn config() -> (TestConfig,) {
        // It loads the dotenv and ignore errors if file doesn't exists
        let _ = dotenv();

        let node_url = env::var("RPC_NODE_URL").unwrap_or(String::from("http://0.0.0.0:18443"));
        let user = env::var("RPC_USER").unwrap_or(String::from("admin1"));
        let pass = env::var("RPC_PASSWORD").unwrap_or(String::from("123"));

        (TestConfig {
            // Use nigiri to make it works instantly
            node_url,
            user,
            pass,
        },)
    }

    #[test]
    fn it_verifies_multisig_creation(_config: TestConfig) {
        let secp = Secp256k1::new();

        let rng = &mut thread_rng();

        let mut parts: Vec<Keypair> = Vec::new();

        for _ in 0..2 {
            let secret_key = SecretKey::new(rng);
            let keypair = Keypair::from_secret_key(&secp, &secret_key);

            println!("part public key: {}", keypair.public_key().to_string());

            parts.push(keypair);
        }

        let secret_key = SecretKey::new(rng);
        let keypair = Keypair::from_secret_key(&secp, &secret_key);

        println!(
            "arbitrator public key: {}",
            keypair.public_key().to_string()
        );

        let arbitrators: Vec<Keypair> = vec![keypair];

        let secret_key = SecretKey::new(rng);
        let internal_pubkey = Keypair::from_secret_key(&secp, &secret_key).public_key();

        println!("internal pubkey: {}", internal_pubkey.to_string());

        let network = Network::Regtest;
        let quorum = 1;

        let multisig = Multisig::new(MultisigData {
            parts: parts.iter().map(|part| part.public_key()).collect(),
            quorum,
            arbitrators: arbitrators
                .iter()
                .map(|arbitrator| arbitrator.public_key())
                .collect(),
            internal_pubkey,
            network,
        });

        assert_eq!(network, multisig.network());

        assert_eq!(quorum, multisig.quorum());

        assert_eq!(
            internal_pubkey.x_only_public_key().0,
            multisig.internal_key(),
        );

        let mut combinations: Vec<Vec<Keypair>> = vec![parts.clone()];

        parts.clone().into_iter().for_each(|part| {
            let mut arbitrators_combinations = utils::combine(&arbitrators, quorum);

            arbitrators_combinations.iter_mut().for_each(|combination| {
                let mut new_combination = vec![part];
                new_combination.append(combination);

                combinations.push(new_combination);
            });
        });

        let combinations_pubkeys: Vec<Vec<PublicKey>> = combinations
            .iter()
            .map(|combination| {
                combination
                    .iter()
                    .map(|keypair| PublicKey::from_secret_key(&secp, &keypair.secret_key()))
                    .collect()
            })
            .collect();
        let multisig_combinations_pubkeys: Vec<Vec<PublicKey>> = multisig
            .scripts()
            .iter()
            .map(|script| script.combination.clone())
            .collect();

        assert_eq!(combinations_pubkeys, multisig_combinations_pubkeys);

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

                let multisig_combination_is_parts = multisig_script.combination.iter().all(|key| {
                    let pubkeys = parts
                        .clone()
                        .iter()
                        .map(|keypair| keypair.public_key())
                        .collect::<Vec<PublicKey>>();
                    pubkeys.contains(key)
                });

                assert_eq!(script.to_asm_string(), multisig_script.leaf.to_asm_string());
                assert_eq!(
                    if multisig_combination_is_parts { 5 } else { 1 },
                    multisig_script.weight
                );

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
                    .map(|(i, script)| ((if i == 0 { 5 } else { 1 }), script.clone())),
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(script_tree.root_hash(), multisig.script_tree().root_hash());

        let address = Address::p2tr(
            &secp,
            internal_pubkey.x_only_public_key().0,
            Some(script_tree.root_hash()),
            network,
        );

        assert_eq!(address, multisig.address());

        println!("Bitcoin address: {}", multisig.address().to_string());
    }

    #[test]
    fn it_spends_multisig_values(config: TestConfig) {
        let secp = Secp256k1::new();
        let rng = &mut thread_rng();

        let client = Client::new(
            &config.node_url,
            Auth::UserPass(config.user.clone(), config.pass.clone()),
        )
        .unwrap();

        let mut parts_keypairs: Vec<Keypair> = Vec::new();

        for _ in 0..2 {
            let secret_key = SecretKey::new(rng);
            let keypair = Keypair::from_secret_key(&secp, &secret_key);

            parts_keypairs.push(keypair);
        }

        let secret_key = SecretKey::new(rng);
        let keypair = Keypair::from_secret_key(&secp, &secret_key);

        let arbitrators: Vec<Keypair> = vec![keypair];

        let secret_key = SecretKey::new(rng);
        let internal_pubkey = Keypair::from_secret_key(&secp, &secret_key).public_key();

        let network = Network::Regtest;
        let quorum = 1;

        let multisig = Multisig::new(MultisigData {
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

        // Needed to generate spendable amounts
        if client.get_block_count().unwrap() < 100 {
            let ghost_address = {
                let secret_key = PrivateKey::generate(network);
                let public_key = secret_key.public_key(&secp);

                Address::p2pkh(public_key.pubkey_hash(), network)
            };

            client.generate_to_address(101, &ghost_address).unwrap();
        }

        let rpc_address = client
            .get_new_address(None, None)
            .unwrap()
            .require_network(network)
            .unwrap();

        client.generate_to_address(1, &rpc_address).unwrap();

        let txid = client
            .send_to_address(
                &multisig.address(),
                client.get_balance(Some(0), None).unwrap(),
                Some("send to multisig"),
                None,
                Some(true),
                None,
                None,
                Some(EstimateMode::Economical),
            )
            .unwrap();

        let tx = client.get_raw_transaction(&txid, None).unwrap();

        let (output_index, output) = tx
            .output
            .iter()
            .enumerate()
            .find(|(_, output)| {
                let address = Address::from_script(&output.script_pubkey, network).unwrap();

                return address == multisig.address();
            })
            .unwrap();

        let output_address = Address::from_script(&output.script_pubkey, network).unwrap();

        assert_eq!(output_address, multisig.address());

        println!("sent {} btc to {} address", output.value, output_address);

        let redeem_script = multisig
            .scripts()
            .into_iter()
            .find(|script| {
                let found_keys: Vec<PublicKey> = script
                    .combination
                    .clone()
                    .into_iter()
                    .filter(|key| {
                        parts_keypairs
                            .iter()
                            .find(|part| part.public_key().eq(key))
                            .is_some()
                    })
                    .collect();

                return found_keys.len() == parts_keypairs.len();
            })
            .unwrap();

        let utxos = vec![Utxo {
            value: output.value,
            outpoint: OutPoint::new(txid, output_index as u32),
        }];

        let redeemer_private_key =
            bitcoin::PrivateKey::new(parts_keypairs[0].secret_key(), network);
        let redeemer = redeemer_private_key.public_key(&secp);

        let redeemer_address = Address::p2pkh(redeemer.pubkey_hash(), network);

        let outs = vec![TxOut {
            value: output.value - Amount::from_sat(200),
            script_pubkey: redeemer_address.script_pubkey(),
        }];

        let mut psbt = multisig.start_tx_spending(SpendingData {
            redeem_script: redeem_script.leaf.clone(),
            outs: outs.clone(),
            utxos: utxos.clone(),
        });

        let unsigned_tx = psbt.unsigned_tx.clone();
        let mut sighash_cache = SighashCache::new(unsigned_tx);

        let leaf_hash = TapLeafHash::from_script(
            redeem_script.leaf.clone().as_script(),
            LeafVersion::TapScript,
        );

        let prevouts: Vec<TxOut> = utxos
            .clone()
            .iter()
            .map(|utxo| TxOut {
                value: utxo.value,
                script_pubkey: multisig.address().script_pubkey(),
            })
            .collect();

        for i in 0..psbt.inputs.len() {
            let sighash = sighash_cache
                .taproot_script_spend_signature_hash(
                    i,
                    &Prevouts::All(&prevouts),
                    leaf_hash,
                    TapSighashType::Default,
                )
                .unwrap();
            let message = Message::from(sighash);

            for keypair in parts_keypairs.iter() {
                let signature = secp.sign_schnorr(&message, keypair);

                let final_signature = taproot::Signature {
                    signature,
                    sighash_type: TapSighashType::Default,
                };

                let (xonly_key, _) = keypair.x_only_public_key();

                psbt.inputs[i]
                    .tap_script_sigs
                    .insert((xonly_key, leaf_hash), final_signature);
            }
        }

        let is_completed = parts_keypairs.iter().all(|keypair| {
            let (xonly, _) = keypair.x_only_public_key();

            psbt.inputs
                .clone()
                .iter()
                .all(|input| input.tap_script_sigs.contains_key(&(xonly, leaf_hash)))
        });

        assert!(is_completed);

        psbt.inputs.iter_mut().for_each(|input| {
            let mut witness = Witness::new();

            for keypair in parts_keypairs.iter().rev() {
                let (xonly, _) = keypair.x_only_public_key();
                let sig = input
                    .tap_script_sigs
                    .get(&(xonly, leaf_hash))
                    .unwrap()
                    .clone();
                witness.push(sig.to_vec());
            }

            let control_block = input.tap_scripts.iter().next().unwrap().0;

            witness.push(redeem_script.leaf.clone().to_bytes());
            witness.push(control_block.serialize());

            input.final_script_witness = Some(witness);
            input.tap_script_sigs.clear();
            input.tap_scripts.clear();
        });

        let tx = psbt.extract_tx().unwrap();

        client.send_raw_transaction(&tx).unwrap();

        let output = tx
            .output
            .iter()
            .find(|output| {
                let address = Address::from_script(&output.script_pubkey, network).unwrap();

                address == redeemer_address
            })
            .unwrap();

        println!(
            "sent {} btc from {} multisig address to {}",
            output.value,
            multisig.address(),
            redeemer_address,
        );
    }
}
