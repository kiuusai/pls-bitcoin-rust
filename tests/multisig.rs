mod multisig_mount_tests {
    use std::{assert_eq, println, vec};

    use pls_bitcoin_lib::{utils, Multisig, MultisigData};

    use bitcoin::key::Keypair;
    use bitcoin::script::Builder;
    use bitcoin::secp256k1::rand::thread_rng;
    use bitcoin::secp256k1::{Secp256k1, SecretKey};
    use bitcoin::taproot::{TapTree, TaprootBuilder};
    use bitcoin::{opcodes, Address, Network, ScriptBuf, XOnlyPublicKey};

    use rstest::rstest;

    #[rstest]
    #[case(2, 1, 1)]
    #[case(5, 2, 2)]
    #[case(2, 3, 2)]
    #[case(2, 3, 1)]
    fn it_verifies_multisig_creation(
        #[case] parts_count: usize,
        #[case] arbitrators_count: usize,
        #[case] quorum: usize,
    ) {
        let secp = Secp256k1::new();

        let rng = &mut thread_rng();

        let mut parts: Vec<Keypair> = Vec::new();

        for _ in 0..parts_count {
            let secret_key = SecretKey::new(rng);
            let keypair = Keypair::from_secret_key(&secp, &secret_key);

            println!("part public key: {}", keypair.public_key().to_string());

            parts.push(keypair);
        }

        let mut arbitrators: Vec<Keypair> = Vec::new();

        for _ in 0..arbitrators_count {
            let secret_key = SecretKey::new(rng);
            let keypair = Keypair::from_secret_key(&secp, &secret_key);

            println!(
                "arbitrator public key: {}",
                keypair.public_key().to_string()
            );

            arbitrators.push(keypair);
        }

        let secret_key = SecretKey::new(rng);
        let internal_pubkey = Keypair::from_secret_key(&secp, &secret_key).public_key();

        println!("internal pubkey: {}", internal_pubkey.to_string());

        let network = Network::Regtest;

        let multisig = Multisig::new(MultisigData {
            parts: parts
                .iter()
                .map(|part| part.x_only_public_key().0)
                .collect(),
            quorum,
            arbitrators: arbitrators
                .iter()
                .map(|arbitrator| arbitrator.x_only_public_key().0)
                .collect(),
            internal_pubkey,
            network,
        })
        .unwrap();

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

        let combinations_pubkeys: Vec<Vec<XOnlyPublicKey>> = combinations
            .iter()
            .map(|combination| {
                combination
                    .iter()
                    .map(|keypair| keypair.x_only_public_key().0)
                    .collect()
            })
            .collect();
        let multisig_combinations_pubkeys: Vec<Vec<XOnlyPublicKey>> = multisig
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
                    let pubkeys: Vec<XOnlyPublicKey> = parts
                        .clone()
                        .iter()
                        .map(|keypair| keypair.x_only_public_key().0)
                        .collect();
                    pubkeys.contains(key)
                });

                assert_eq!(script.to_asm_string(), multisig_script.leaf.to_asm_string());
                assert_eq!(
                    if multisig_combination_is_parts { 5 } else { 1 },
                    multisig_script.weight
                );

                let combination: Vec<XOnlyPublicKey> = combinations[i]
                    .clone()
                    .iter()
                    .map(|keypair| keypair.x_only_public_key().0)
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
}

mod multisig_spending_tests {
    use core::time;
    use std::collections::{HashMap, HashSet};
    use std::time::Duration;
    use std::{assert_matches, println, vec};

    use pls_bitcoin_lib::multisig::{Multisig, MultisigData, Utxo};
    use pls_bitcoin_lib::SpendingData;

    use bitcoin::absolute::LockTime;
    use bitcoin::consensus::encode::serialize_hex;
    use bitcoin::key::Keypair;
    use bitcoin::secp256k1::rand::thread_rng;
    use bitcoin::secp256k1::{Message, Secp256k1, SecretKey, XOnlyPublicKey};
    use bitcoin::sighash::{Prevouts, SighashCache};
    use bitcoin::taproot::{self, LeafVersion};
    use bitcoin::{
        Address, Amount, Network, OutPoint, PrivateKey, TapLeafHash, TapSighashType, TxOut, Txid,
        Witness,
    };
    use nigiri_rs::fixtures::Fixture;
    use nigiri_rs::{Bitcoin, BitcoinUtxo, NigiriClient, NigiriError};
    use rstest::rstest;

    #[rstest]
    #[case(2, 1, 1)]
    #[case(5, 2, 2)]
    #[case(2, 3, 2)]
    #[case(2, 3, 1)]
    #[tokio::test]
    async fn it_spends_multisig_values(
        #[case] parts_count: usize,
        #[case] arbitrators_count: usize,
        #[case] quorum: usize,
        #[values(0, 1, 5)] blocks_to_lock: usize,
    ) {
        let fixture = Fixture::<Bitcoin>::start().await.unwrap();
        let client: NigiriClient<Bitcoin> = fixture.client().clone();

        let secp = Secp256k1::new();
        let rng = &mut thread_rng();

        let mut parts: Vec<Keypair> = Vec::new();

        for _ in 0..parts_count {
            let secret_key = SecretKey::new(rng);
            let keypair = Keypair::from_secret_key(&secp, &secret_key);

            parts.push(keypair);
        }

        let mut arbitrators: Vec<Keypair> = Vec::new();

        for _ in 0..arbitrators_count {
            let secret_key = SecretKey::new(rng);
            let keypair = Keypair::from_secret_key(&secp, &secret_key);

            arbitrators.push(keypair);
        }

        let secret_key = SecretKey::new(rng);
        let internal_pubkey = Keypair::from_secret_key(&secp, &secret_key).public_key();

        let network = Network::Regtest;

        let multisig = Multisig::new(MultisigData {
            parts: parts
                .iter()
                .map(|part| part.x_only_public_key().0)
                .collect(),
            quorum,
            arbitrators: arbitrators
                .iter()
                .map(|arbitrator| arbitrator.x_only_public_key().0)
                .collect(),
            internal_pubkey,
            network,
        })
        .unwrap();

        let ghost_address = {
            let secret_key = PrivateKey::new(SecretKey::new(rng), network);
            let public_key = secret_key.public_key(&secp);

            Address::p2pkh(public_key.pubkey_hash(), network)
        };

        let mut all_keypairs: HashMap<XOnlyPublicKey, Keypair> = HashMap::new();

        parts.clone().into_iter().for_each(|part| {
            all_keypairs.insert(part.x_only_public_key().0, part);
        });

        arbitrators.clone().into_iter().for_each(|arbitrator| {
            all_keypairs.insert(arbitrator.x_only_public_key().0, arbitrator);
        });

        for (i, redeem_script) in multisig.scripts().iter().enumerate() {
            println!("script {} being tested", i);

            println!("sending 1 BTC to multisig address");

            let txid = client
                .faucet(&multisig.address().to_string(), Some(Amount::ONE_BTC))
                .await
                .unwrap();

            println!("faucet tx: {}", txid);

            client
                .wait_for_confirmation(&txid, Duration::MAX)
                .await
                .unwrap();

            tokio::time::sleep(time::Duration::from_millis(100)).await;

            let outputs = client
                .get_utxos(&multisig.address().to_string())
                .await
                .unwrap();

            // Ensures that outputs are not repeated
            // Necessary to avoid API errors
            let mut vouts: HashSet<(u32, Txid)> = HashSet::new();
            let outputs: Vec<BitcoinUtxo> = outputs
                .into_iter()
                .filter(|out| {
                    let exists = vouts.contains(&(out.vout, out.txid));

                    if exists {
                        return false;
                    }

                    vouts.insert((out.vout, out.txid));
                    return true;
                })
                .collect();

            println!("outputs: {:?}", outputs);

            let balance = outputs
                .iter()
                .map(|out| out.value)
                .reduce(|acc, e| acc + e)
                .unwrap();

            println!("sent {} btc to {} address", balance, multisig.address());

            let utxos: Vec<Utxo> = outputs
                .iter()
                .map(|out| Utxo {
                    value: out.value,
                    outpoint: OutPoint {
                        txid,
                        vout: out.vout,
                    },
                })
                .collect();

            let redeemer_private_key = bitcoin::PrivateKey::new(parts[0].secret_key(), network);
            let redeemer = redeemer_private_key.public_key(&secp);

            let redeemer_address = Address::p2pkh(redeemer.pubkey_hash(), network);

            let fee = Amount::from_sat(1000);

            let outs = vec![TxOut {
                value: balance - fee,
                script_pubkey: redeemer_address.script_pubkey(),
            }];

            let lock_time: Option<LockTime> = if blocks_to_lock > 0 {
                let current_block = client.block_height().await.unwrap();

                Some(LockTime::from_height(current_block as u32 + blocks_to_lock as u32).unwrap())
            } else {
                None
            };

            let mut psbt = multisig
                .start_tx_spending(SpendingData {
                    redeem_script: redeem_script.leaf.clone(),
                    outs: outs.clone(),
                    utxos: utxos.clone(),
                    lock_time,
                })
                .unwrap();

            println!("unspent transaction vsize: {}", psbt.unsigned_tx.vsize());

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

                for public_key in redeem_script.combination.iter() {
                    let keypair = all_keypairs.get(public_key).unwrap();

                    let signature = secp.sign_schnorr(&message, keypair);

                    let final_signature = taproot::Signature {
                        signature,
                        sighash_type: TapSighashType::Default,
                    };

                    psbt.inputs[i]
                        .tap_script_sigs
                        .insert((*public_key, leaf_hash), final_signature);
                }
            }

            let is_completed = redeem_script.combination.iter().all(|public_key| {
                psbt.inputs.clone().iter().all(|input| {
                    input
                        .tap_script_sigs
                        .contains_key(&(*public_key, leaf_hash))
                })
            });

            assert!(is_completed);

            psbt.inputs.iter_mut().for_each(|input| {
                let mut witness = Witness::new();

                for public_key in redeem_script.combination.iter().rev() {
                    let sig = input
                        .tap_script_sigs
                        .get(&(*public_key, leaf_hash))
                        .unwrap()
                        .clone();
                    witness.push(sig.to_vec());
                }

                let control_block = input
                    .tap_scripts
                    .iter()
                    .find(|tap_script| {
                        let (_, (leaf, _)) = tap_script;
                        redeem_script.leaf == leaf.clone()
                    })
                    .unwrap()
                    .0;

                witness.push(redeem_script.leaf.clone().to_bytes());
                witness.push(control_block.serialize());

                input.final_script_witness = Some(witness);
                input.tap_script_sigs.clear();
                input.tap_scripts.clear();
            });

            let tx = psbt.extract_tx().unwrap();

            println!("final tx vsize: {}", tx.vsize());

            if blocks_to_lock > 0 {
                println!("needs {} mined blocks to spend", blocks_to_lock);

                // Ensures that broadcast transaction fails until satisfies locktime
                for _ in 0..blocks_to_lock {
                    let res = client.broadcast_tx(&serialize_hex(&tx)).await;

                    assert!(res.is_err(), "res is: {:?}", res);

                    let res_err = res.unwrap_err();

                    // Asserts that transaction was not processed because of the locktime
                    assert_matches!(
                        res_err,
                        NigiriError::RpcFailed {
                            code: -26,
                            ref method,
                            ref message,
                        } if method == "sendrawtransaction" && message == "non-final"
                    );

                    client
                        .generate_to_address(1 as u64, &ghost_address.to_string())
                        .await
                        .unwrap();
                }
            }

            client.broadcast_tx(&serialize_hex(&tx)).await.unwrap();

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

        fixture.shutdown().await.unwrap();
        println!("test finished");
    }
}
