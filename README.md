# pls-bitcoin-lib

Welcome to Private Law Society (PLS) Bitcoin lib!

This repository contains the Rust code for multisig contracts generation for Private Law Society project.
It consists in a multisig generator for contracts.

## DISCLAIMER

That's a beta lib.
Understand that it's under progressive development and it can have compatibility issues with future protocol versions.
Major updates means protocol incompatibility with older versions.
Middle ones means possible incompatible structs and/or functions definitions or perhaps new developed features.
Also, as any beta project it may susceptible to failues.
We test it rigorously and we have an active community that help us to resolve a lot of issues, but IT REMAINS AS A BETA SOFTWARE.
Use it as your own risk.

By using this lib you understand that we (Private Law Society) aren't responsible for any funds losses.

## Help us contributing (First steps to develop)

This project uses some technologies to help developers reduces the friction when startin development.
Every tool used here are optional. But all help you develop faster. So consider using them.

See a resume of environment helpers:
- [Devcontainers](#devcontainers)
- [ASDF](#asdf)
- [Nigiri (inside Devcontainers)](#nigiri-inside-devcontainers)


Also, see some topics to work with this project:
- [Executing automated tests](#executing-automated-tests)
- [Preview docs locally](#preview-docs-locally)

### Devcontainers

This lib was developed using [Devcontainers](https://containers.dev/).
It means you can simply up the project devcontainer and get hands dirty.

For a vscode instance just install devcontainers extension and start the work.
If you is a NeoVIM rat you can use [`devcontainers-cli`](https://github.com/devcontainers/cli) to up containers and start work with it directly from terminal.

#### Why devcontainers?

There are some reasons to use Devcontainers here.
Such as:
- Preconfigured and replicable environment for every contributer.
- No "works on my machine"
- Patternized environment for e2e tests, build and publish

#### Installing `devcontainers-cli` with [NPM](https://github.com/npm/cli)

With NPM package manager, install `devcontainers-cli`:
```bash
npm install -g @devcontainers/cli
```

#### Starting devcontainers

Up project devcontainers.

```bash
devcontainer up
```
It should create devcontainers and up it into Docker instances.

Then enter in development container:
```bash
devcontainer exec bash # Or your prefered shell instance such as ZSH or FISH
```

To exit, just enter `exit` or press `Ctrl+D`.

`IMPORTANT`: You should have Docker and Docker Compose installed on your machine.

### ASDF

This lib contains a [`.tool-versions`](./.tool-versions) file.
It's a version descriptor for [ASDF](https://github.com/asdf-vm/asdf) version manager.
It means you can use ASDF to download the exactly Rust version that was used for develop it.
ASDF is a very flexible version manager.
It works managing versions for a lot of language sets.
So, you can work with some projects each one containing their own `.tool-versions` ensuring that the correct version for that language has being used.
Consider using it for your own personal projects.

#### Installing ASDF

The easiest way to install ASDF is with Golang:

```bash
go install github.com/asdf-vm/asdf/cmd/asdf@v0.20.0
```

#### Configuring to current project

Just follow the steps below:

Install rust plugin:
```bash
asdf plugin add rust

```

In project root install tools: 
```bash
asdf install
```

### Nigiri (inside Devcontainers)

[Nigiri](https://github.com/vulpemventures/nigiri) are being used as a helper to e2e tests.
It's installed in devcontainers and it's necessary to run e2e tests correctly.

#### Using Nigiri

Inside devcontainer you can start Nigiri by just doing:
```bash
nigiri start
```

To prune Nigiri data you can stop it with `--delete` flag:
```bash
nigiri stop --delete
```

### Executing automated tests

`DISCLAIMER`: You need nigiri instance started to run automated tests.
See [Nigiri (inside Devcontainers)](#nigiri-inside-devcontainers) for more information.

To execute automated tests just execute this command in root folder:
```bash
cargo test
```

If you want to see the `print` or `println` macros outputs, execute the following command:
```bash
cargo test -- --show-output
```

### Preview docs locally

Some times you should need to preview generated docs for this lib.
So, you can do it running:
```bash
cargo doc
```

It will generates a file named `target/doc/pls_bitcoin_lib/index.html` that you can open on your browser.
That's a html documentation for library.

You can also open it directly running the same command with `--open` flag:
```bash
cargo doc --open
```

## Multisig address generator (Example)

See this basic example:
```rust
use std::vec;
use secp256k1::{PublicKey};
use bitcoin::{Network};
use pls_bitcoin_lib::{Multisig, MultisigData};

let parts = vec![
    PublicKey::from_str("02b55f16363d70ae5034cc39554e8ce151254ab380bed2029cc7344807c22e6c1b"),
    PublicKey::from_str("038677177e7ce4f8090f07661ac39636e4ea921bf28f7e45ba24dcf6ea56aa5f97"),
];

let arbitrators = vec![PublicKey::from_str("03017f1ce0d34892be7e930c8eea77f54ce300386dea5e883bf1da60f47d64f547")];

// Internal public key for constructing the multisig
let internal_pubkey = PublicKey::from_str("03af0c7e8b8cf586f762ce1377a51fc6b7228a9caed4a5dcb43b180acf6824f7c9");

// Minimal arbitrators signatures to unlock with one of the parts
let quorum = 1;

let network = Network::Regtest;

let multisig = Multisig::new(MultisigData {
    parts,
    arbitrators,
    quorum,
    internal_pubkey,
    network,
});

// Should prints "bcrt1pu0z0pwk4jn3naucadmr8gz9eh2xd3ts5shkat9vkslkms44hpavsvcdleq"
println!(multisig.address().to_string());
```

