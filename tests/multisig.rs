use std::{assert_eq, println};

use pls_bitcoin_lib::multisig::{Multisig, MultisigOptions};

use bitcoin::KnownHrp;
use bitcoin::key::Keypair;
use bitcoin::secp256k1::{Secp256k1, SecretKey};

#[test]
fn it_verifies_multisig_creation() {
    let secp = Secp256k1::new();

    let mut parts_keypairs: Vec<Keypair> = Vec::new();

    for x in 0..1 {
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

    let multisig = Multisig::new(MultisigOptions {
        parts: parts_keypairs.iter().map(|part| part.public_key()).collect(),
        quorum: 1,
        arbitrators: arbitrators.iter().map(|arbitrator| arbitrator.public_key()).collect(),
        internal_pubkey,
        network: KnownHrp::Mainnet,
    });

    assert_eq!(internal_pubkey.x_only_public_key().0, multisig.get_internal_key());

    println!("Bitcoin address: {}", multisig.get_address().to_string());
}
