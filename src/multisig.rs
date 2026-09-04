use std::vec;

use crate::utils::*;

use bitcoin::absolute::LockTime;
use bitcoin::script::Builder;
use bitcoin::secp256k1;
use bitcoin::secp256k1::{Secp256k1, PublicKey, XOnlyPublicKey};
use bitcoin::taproot::{LeafVersion, NodeInfo, TapTree, TaprootBuilder, TaprootSpendInfo};
use bitcoin::{
    opcodes, transaction, Address, Amount, KnownHrp, OutPoint, Psbt, ScriptBuf, Sequence,
    Transaction, TxIn, TxOut, Witness,
};

#[derive(Clone)]
pub struct Utxo {
    pub outpoint: OutPoint,
    pub value: Amount,
}

#[derive(Clone)]
pub struct MultisigScript {
    pub weight: usize,
    pub leaf: ScriptBuf,
    pub combination: Vec<PublicKey>,
}

pub struct MultisigOptions {
    pub parts: Vec<PublicKey>,
    pub arbitrators: Vec<PublicKey>,
    pub quorum: usize,
    pub internal_pubkey: PublicKey,
    pub network: KnownHrp,
}

#[derive(Clone)]
pub struct Multisig {
    address: Address,
    multisig_scripts: Vec<MultisigScript>,
    internal_key: XOnlyPublicKey,
    script_tree: TapTree,

    secp: Secp256k1<secp256k1::All>,
}

impl Multisig {
    pub fn new(opts: MultisigOptions) -> Multisig {
        let secp = Secp256k1::new();

        // Create scripts arrays with each combination for each cases
        let mut keys_combination: Vec<Vec<PublicKey>> = vec![opts.parts.clone()];

        opts.parts.into_iter().for_each(|part| {
            let mut arbitrators_combinations = combine(&opts.arbitrators, opts.quorum);

            arbitrators_combinations.iter_mut().for_each(|combination| {
                let mut new_combination = vec![part];
                new_combination.append(combination);

                keys_combination.push(new_combination);
            });
        });

        // Mount scripts options for multisig
        let (xonly_internal_pubkey, _) = opts.internal_pubkey.x_only_public_key();

        let mut scripts: Vec<ScriptBuf> = Vec::new();

        keys_combination.iter().for_each(|combination| {
            let mut builder = Builder::new();

            let mut combination_iter = combination.iter();

            let first_key = combination_iter.next().unwrap();

            builder = builder.push_key(&bitcoin::PublicKey::new(*first_key));

            let mut first_combination = true;

            for key in combination_iter {
                builder = builder.push_opcode(if first_combination {
                    opcodes::all::OP_CHECKSIG
                } else {
                    opcodes::all::OP_CHECKSIGADD
                });

                builder = builder.push_key(&bitcoin::PublicKey::new(*key));

                first_combination = false;
            }

            let mut script = builder.into_script();

            script = script.to_p2tr(&secp, xonly_internal_pubkey);

            scripts.push(script);
        });

        // Mount taptree
        let multisig_scripts = scripts
            .iter()
            .enumerate()
            .map(|(i, script)| MultisigScript {
                weight: i,
                leaf: script.clone(),
                combination: keys_combination.get(i).unwrap().clone(),
            })
            .collect();

        let builder = TaprootBuilder::with_huffman_tree(
            scripts
                .iter()
                .enumerate()
                .map(|(i, script)| (u32::try_from(i).unwrap(), script.clone())),
        )
        .unwrap();

        let script_tree = TapTree::try_from(builder).unwrap();

        // Create multisig address for this one
        let address = Address::p2tr(
            &bitcoin::secp256k1::Secp256k1::new(),
            xonly_internal_pubkey,
            Some(script_tree.root_hash()),
            opts.network,
        );

        return Multisig {
            address,
            multisig_scripts,
            script_tree,
            internal_key: xonly_internal_pubkey,

            secp,
        };
    }

    pub fn get_address(&self) -> Address {
        return self.address.clone();
    }

    pub fn get_multisig_scripts(&self) -> Vec<MultisigScript> {
        return self.multisig_scripts.clone();
    }

    pub fn get_script_tree(&self) -> TapTree {
        return self.script_tree.clone();
    }

    pub fn get_internal_key(&self) -> XOnlyPublicKey {
        return self.internal_key;
    }

    pub fn start_tx_spending(
        &self,
        redeem_script: ScriptBuf,
        utxos: Vec<Utxo>,
        outs: Vec<TxOut>,
    ) -> Psbt {
        let unsigned_tx = Transaction {
            version: transaction::Version::TWO,
            input: utxos
                .iter()
                .map(|utxo| TxIn {
                    previous_output: utxo.outpoint.clone(),
                    script_sig: ScriptBuf::new(),
                    sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
                    witness: Witness::new(),
                })
                .collect(),
            output: outs,
            lock_time: LockTime::ZERO,
        };

        let mut psbt = Psbt::from_unsigned_tx(unsigned_tx).unwrap();

        let node_info = NodeInfo::from(self.script_tree.clone());

        let spend_info = TaprootSpendInfo::from_node_info(
            &self.secp,
            self.internal_key,
            node_info,
        );

        let control_block = spend_info
            .control_block(&(redeem_script.clone(), LeafVersion::TapScript))
            .unwrap();

        utxos.iter().enumerate().for_each(|(i, utxo)| {
            let input = &mut psbt.inputs[i];

            input.witness_utxo = Some(TxOut {
                value: utxo.value,
                script_pubkey: self.address.script_pubkey(),
            });
            input.tap_scripts.insert(
                control_block.clone(),
                (redeem_script.clone(), LeafVersion::TapScript),
            );
        });

        return psbt;
    }
}
