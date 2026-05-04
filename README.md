# Bitcoin Wallet CLI

A minimal Bitcoin wallet command-line interface built in Rust using the Bitcoin Dev Kit (BDK) version 0.29.

This wallet operates on **Bitcoin Testnet** by default for safety, but can be switched to Mainnet via a flag. It uses BIP84 derivation (Native SegWit `tb1...` addresses) and connects to the Electrum network.

## Features

- Generate new BIP39 12-word mnemonics
- Restore from existing mnemonics
- Derive native SegWit receive addresses
- Check confirmed and unconfirmed balances
- Send Bitcoin with customizable fee rates
- View transaction history
- JSON output support for machine readability

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (cargo, rustc)

## Setup & Build

1. Clone or navigate to the repository:
   ```bash
   cd bitcoin-wallet
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```
   The binary will be available at `./target/release/wallet`.

   *Note: If you encounter issues compiling the `cc` crate on macOS, make sure you are using an up-to-date Rust toolchain (`rustup update`).*

## Example Usage (Testnet)

The default network is `testnet`. You can optionally pass `--network mainnet` to operate on the main network.

### 1. Generate a New Wallet

Generates a new 12-word mnemonic and shows the first receive address.
**Important:** Save this mnemonic securely! It is not saved to disk automatically.

```bash
cargo run -- generate
```

### 2. Check Balance

Queries the Electrum server for the balance of your wallet.

```bash
cargo run -- balance --mnemonic "your twelve word mnemonic phrase goes right here securely please"
```

### 3. Get a Receive Address

Generates the next unused receive address for your wallet.

```bash
cargo run -- address --mnemonic "your twelve word mnemonic phrase goes right here securely please"
```

*Note: You can get testnet coins from a faucet like [coinfaucet.eu](https://coinfaucet.eu/en/btc-testnet/) or [bitcoinfaucet.uo1.net](https://bitcoinfaucet.uo1.net/) by providing this address.*

### 4. Send Bitcoin

Sends a specific amount of satoshis to a target address.

Arguments: `--mnemonic <PHRASE> <ADDRESS> <AMOUNT_IN_SATS> <FEE_RATE_SATS_PER_VBYTE>`

```bash
cargo run -- send --mnemonic "your twelve word mnemonic phrase goes right here securely please" tb1q...recipient_address... 50000 2.5
```

### 5. View Transaction History

Lists past transactions including their ID, amount received/sent, and confirmation status.

```bash
cargo run -- history --mnemonic "your twelve word mnemonic phrase goes right here securely please"
```

## JSON Output

All commands support a `--json` flag to output the result in JSON format, which is useful for integration with other scripts.

```bash
cargo run -- --json generate
```
