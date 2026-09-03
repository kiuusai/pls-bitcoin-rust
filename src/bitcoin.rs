use std::vec;

use bitcoin::script::Builder;
use bitcoin::secp256k1::PublicKey;
use bitcoin::taproot::{TapTree, TaprootBuilder};
use bitcoin::{opcodes, Address, KnownHrp, ScriptBuf, TapLeafHash};

pub struct MultisigScript {
    pub weight: usize,
    pub leaf: TapLeafHash,
    pub combination: Vec<PublicKey>,
}

pub struct Multisig {
    address: Address,
    multisig_scripts: Vec<MultisigScript>,
}

fn combine<T: Clone>(items: &[T], size: usize) -> Vec<Vec<T>> {
    if size == 0 {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current = Vec::with_capacity(size);

    generate_combinations(items, size, 0, &mut current, &mut result);

    result
}

fn generate_combinations<T: Clone>(
    items: &[T],
    size: usize,
    start: usize,
    current: &mut Vec<T>,
    result: &mut Vec<Vec<T>>,
) {
    if current.len() == size {
        result.push(current.clone());
        return;
    }

    for index in start..items.len() {
        current.push(items[index].clone());

        // Only consider later items, preventing reordered duplicates.
        generate_combinations(items, size, index + 1, current, result);

        // Undo the choice before trying the next item.
        current.pop();
    }
}

impl Multisig {
    pub fn new(
        parts: Vec<PublicKey>,
        arbitrators: Vec<PublicKey>,
        quorum: usize,
        internal_pubkey: PublicKey,
        network: KnownHrp,
    ) -> Multisig {
        // Create scripts arrays with each combination for each cases
        let mut keys_combination: Vec<Vec<PublicKey>> = vec![parts.clone()];

        for part in parts.clone().into_iter() {
            let mut arbitrators_combinations = combine(&arbitrators, quorum);

            for combination in arbitrators_combinations.iter_mut() {
                let mut new_combination = vec![part];
                new_combination.append(combination);

                keys_combination.push(new_combination);
            }
        }

        // Mount scripts options for multisig

        let mut scripts: Vec<ScriptBuf> = Vec::new();

        for combination in keys_combination.iter() {
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

            let script = builder.into_script();

            scripts.push(script);
        }

        // Mount taptree
        let multisig_scripts = scripts
            .iter()
            .enumerate()
            .map(|(i, script)| MultisigScript {
                weight: i,
                leaf: script.tapscript_leaf_hash(),
                combination: keys_combination.get(i).unwrap().clone(),
            }).collect();

        let builder = TaprootBuilder::with_huffman_tree(
            scripts
                .iter()
                .enumerate()
                .map(|(i, script)| (u32::try_from(i).unwrap(), script.clone())),
        )
        .unwrap();

        let taptree = TapTree::try_from(builder).unwrap();

        // Create multisig address for this one
        let (xonly_internal_pubkey, _) = internal_pubkey.x_only_public_key();
        let address = Address::p2tr(
            &bitcoin::secp256k1::Secp256k1::new(),
            xonly_internal_pubkey,
            Some(taptree.root_hash()),
            network,
        );

        return Multisig {
            address,
            multisig_scripts,
        };
    }

    pub fn get_address(self) -> Address {
        return self.address;
    }

    pub fn get_multisig_scripts(self) -> Vec<MultisigScript> {
        return self.multisig_scripts;
    }

    pub fn start_tx_spending(self) {}
}
