use std::vec;

use crate::utils::*;

use bitcoin::absolute::LockTime;
use bitcoin::script::Builder;
use bitcoin::secp256k1::{PublicKey, Secp256k1, XOnlyPublicKey};
use bitcoin::taproot::{LeafVersion, NodeInfo, TapTree, TaprootBuilder, TaprootSpendInfo};
use bitcoin::{
    opcodes, transaction, Address, Amount, OutPoint, Psbt, ScriptBuf, Sequence, Transaction, TxIn,
    TxOut, Witness,
};
use bitcoin::{secp256k1, Network};

/// UTXO structure to use when start spending TX spending
#[derive(Debug, Clone)]
pub struct Utxo {
    /// Blockchain reference to UTXO
    pub outpoint: OutPoint,
    /// Value in sats into the UTXO
    pub value: Amount,
}

/// Data of each multisig script
#[derive(Debug, Clone)]
pub struct MultisigScript {
    /// Weight of script sort into script tree
    pub weight: usize,
    /// The script raw data
    pub leaf: ScriptBuf,
    /// Public keys combination that matches with current script
    pub combination: Vec<PublicKey>,
}

/// Required data to create multisig
#[derive(Debug, Clone)]
pub struct MultisigData {
    /// The public key list from contractors (involved parts)
    pub parts: Vec<PublicKey>,
    /// The publc key list from contract arbitrators
    pub arbitrators: Vec<PublicKey>,
    /// Quorum of minimal necessary arbitrators to unlock funds
    pub quorum: usize,
    /// Internal public key. It's a critical data.
    /// If the private key of this one is known, all funds can be sweeped.
    /// This field exists for compatibility purposes with current on-air system.
    /// See issue [here](https://github.com/PrivateLawSociety/pls-lib/blob/5d7e963f0c64cb67afc7d61e1a6f92fa40b6cb4c/packages/pls-bitcoin/index.ts#L40)
    // TODO: Finds a way to create a verifiable one using parts and arbitrators data instead of a
    // hardcoded one
    pub internal_pubkey: PublicKey,
    /// Network to create multisig
    pub network: Network,
}

/// Multisig implementation.
#[derive(Debug, Clone)]
pub struct Multisig {
    address: Address,
    multisig_scripts: Vec<MultisigScript>,
    internal_key: XOnlyPublicKey,
    script_tree: TapTree,
    quorum: usize,
    network: Network,

    secp: Secp256k1<secp256k1::All>,
}

impl Multisig {
    /// Creates multisig struct
    /// # Args
    /// - `data`([MultisigData]): Data to multisig generation
    /// # Returns
    /// Created multisig struct
    /// # Example
    /// ```ignore
    /// use std::vec;
    /// use secp256k1::{PublicKey};
    /// use bitcoin::{Network};
    /// use pls_bitcoin_lib::{Multisig, MultisigData};
    ///
    /// let parts = vec![
    ///     PublicKey::from_str("02b55f16363d70ae5034cc39554e8ce151254ab380bed2029cc7344807c22e6c1b"),
    ///     PublicKey::from_str("038677177e7ce4f8090f07661ac39636e4ea921bf28f7e45ba24dcf6ea56aa5f97"),
    /// ];
    ///
    /// let arbitrators = vec![PublicKey::from_str("03017f1ce0d34892be7e930c8eea77f54ce300386dea5e883bf1da60f47d64f547")];
    ///
    /// // Internal public key for constructing the multisig
    /// let internal_pubkey = PublicKey::from_str("03af0c7e8b8cf586f762ce1377a51fc6b7228a9caed4a5dcb43b180acf6824f7c9");
    ///
    /// // Minimal arbitrators signatures to unlock with one of the parts
    /// let quorum = 1;
    ///
    /// let network = Network::Regtest;
    ///
    /// let multisig = Multisig::new(MultisigData {
    ///     parts,
    ///     arbitrators,
    ///     quorum,
    ///     internal_pubkey,
    ///     network,
    /// });
    ///
    /// // Should prints "bcrt1pu0z0pwk4jn3naucadmr8gz9eh2xd3ts5shkat9vkslkms44hpavsvcdleq"
    /// println!(multisig.address().to_string());
    /// ```
    pub fn new(data: MultisigData) -> Multisig {
        let secp = Secp256k1::new();

        // Create scripts arrays with each combination for each cases
        let mut keys_combination: Vec<Vec<PublicKey>> = vec![data.parts.clone()];

        data.parts.into_iter().for_each(|part| {
            let mut arbitrators_combinations = combine(&data.arbitrators, data.quorum);

            arbitrators_combinations.iter_mut().for_each(|combination| {
                let mut new_combination = vec![part];
                new_combination.append(combination);

                keys_combination.push(new_combination);
            });
        });

        // Mount scripts options for multisig
        let (xonly_internal_pubkey, _) = data.internal_pubkey.x_only_public_key();

        let mut scripts: Vec<ScriptBuf> = Vec::new();

        keys_combination.iter().for_each(|combination| {
            let mut builder = Builder::new();

            let mut first_combination = true;

            for key in combination.iter() {
                let xonly_key = key.x_only_public_key().0;

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

        // Mount taptree
        let multisig_scripts: Vec<MultisigScript> = scripts
            .iter()
            .enumerate()
            .map(|(i, script)| MultisigScript {
                // Prioritize parts agreements
                /* TODO: Fix logic to trully prioritize shortest paths instead only parts
                agreements (Breaking change) */
                weight: (if i == 0 { 5 } else { 1 }),
                leaf: script.clone(),
                combination: keys_combination[i].clone(),
            })
            .collect();

        let builder = TaprootBuilder::with_huffman_tree(
            multisig_scripts
                .iter()
                .map(|script| (script.weight as u32, script.leaf.clone())),
        )
        .unwrap();

        let script_tree = TapTree::try_from(builder).unwrap();

        // Create multisig address for this one
        let address = Address::p2tr(
            &secp,
            xonly_internal_pubkey,
            Some(script_tree.root_hash()),
            data.network,
        );

        return Multisig {
            address,
            multisig_scripts,
            script_tree,
            internal_key: xonly_internal_pubkey,
            network: data.network,
            quorum: data.quorum,

            secp,
        };
    }

    /// # Returns
    /// Created multisig address.
    pub fn address(&self) -> Address {
        return self.address.clone();
    }

    /// # Returns
    /// A list with multisig scripts.
    /// Helps to finds the leaf to unlock the transaction.
    pub fn scripts(&self) -> Vec<MultisigScript> {
        return self.multisig_scripts.clone();
    }

    /// # Returns
    /// The created multisig [TapTree].
    pub fn script_tree(&self) -> TapTree {
        return self.script_tree.clone();
    }

    /// # Returns
    /// The x-only of internal publick key.
    pub fn internal_key(&self) -> XOnlyPublicKey {
        return self.internal_key;
    }

    /// # Returns
    /// The current network of multisig.
    pub fn network(&self) -> Network {
        return self.network;
    }

    /// # Returns
    /// The arbitrators quorum.
    /// It's the minimal arbitrators signatures required when the transaction isn't unlocked only
    /// with parts signatures.
    pub fn quorum(&self) -> usize {
        return self.quorum;
    }

    /// Starts spending of UTXO's that are into multisig address
    /// # Args
    /// - `redeem_script`([ScriptBuf]): Script to unlock UTXO's
    /// - `utxos`: ([Vec]<[Utxo]>): A list of UTXO's to unlock
    /// - `outs`: ([Vec]<[TxOut]>): A list of outputs as a destination for unlocked funds
    /// # Returns
    /// A Partial Signed Bitcoin Transaction (PSBT) that contains the given UTXO's and outputs
    /// configured to be unlocked with the given redeem script.
    /// # Usage
    /// ```ignore
    /// use std::vec;
    ///
    /// let multisig = Multisig::new(MultisigData {/* Multisig data */});
    ///
    /// // Select it as your preference
    /// let script_to_select = 0;
    /// let redeem_script: ScriptBuf = multisig.scripts()[script_to_select];
    ///
    /// let utxos: Vec<Utxo> = vec![/* UTXOS to unlock */];
    /// let outs: Vec<TxOut> = vec![/* Outputs */];
    ///
    /// let psbt = multisig.start_tx_spending(redeem_script, utxos, outs);
    /// ```
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

        let spend_info = TaprootSpendInfo::from_node_info(&self.secp, self.internal_key, node_info);

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
