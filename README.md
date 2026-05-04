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

## Security Considerations

> **This wallet is a learning tool — not production software.**

### Why Testnet Is the Default

Bitcoin Testnet coins have no monetary value. Running on Testnet by default means that configuration mistakes, bad fee rates, and accidental sends can never cost real money. Switch to `--network mainnet` only when you explicitly intend to use real funds.

### What `--confirm` Protects Against

Any mainnet send above **1 000 000 satoshis (0.01 BTC)** requires the `--confirm` flag to be passed explicitly:

```bash
cargo run -- --network mainnet send --mnemonic "..." <ADDRESS> 2000000 2.0 --confirm
```

Without `--confirm`, the CLI aborts before constructing the transaction. This prevents accidents caused by a misplaced decimal point, wrong units, or a copy-paste error in the amount field.

### Mnemonic Handling

The mnemonic phrase is copied into a [`Zeroizing`](https://docs.rs/zeroize) buffer immediately on receipt, which overwrites the secret memory when the buffer is dropped. This limits the window during which the raw phrase lives in process memory. However:

- The phrase **is** present in plaintext on the CLI argument list (visible to `ps` / process monitors) for the lifetime of the command.
- The BIP32 extended private key is held in memory for the duration of the command and is not separately zeroized.
- No swap-file or core-dump protections are applied.

Do not run this wallet on a shared or untrusted machine.

### Built-in Hardening Features

This wallet includes several proactive safety mechanisms designed to prevent common operational errors and ensure transaction validity:

- **Strict Fee Ceilings**: Calculates the effective fee (`inputs - outputs`) before signing. If the fee exceeds `10,000,000` satoshis (0.1 BTC), the transaction is immediately aborted to prevent catastrophic fee overpayments.
- **Dust Protection**: Scans all outputs before signing. If any output is below the standard `546` satoshi dust limit, the transaction is rejected, preventing network rejection and wasted effort.
- **Always-On RBF**: Opt-in Replace-By-Fee (BIP 125) is signaled on **every** transaction by default, allowing users to bump stuck transactions later.
- **Guaranteed Finalization**: Belt-and-suspenders validation ensures that every input has valid witness data or script-sig immediately after signing. Unsigned or partially signed PSBTs will never be accidentally broadcast.
- **Zero Panics**: The codebase is strictly linted (`#![deny(clippy::unwrap_used)]`, `panic`, `expect_used`, `indexing_slicing`, `arithmetic_side_effects`) to guarantee deterministic error handling over panics.
- **Adversarial Tested**: Validated against property-based fuzzing (using `proptest`) for extreme boundary values (e.g., sending exact balance amounts, `f32::MAX` fee rates, NaN fee rates) to ensure the wallet handles unexpected inputs gracefully.

### MemoryDatabase — No Encryption

UTXO data, transaction history, and derived addresses are stored in BDK's `MemoryDatabase` (an in-process hash map). This data:

- Is **not encrypted** at rest.
- Is **not persisted** to disk between runs — the wallet re-derives everything from the mnemonic on each invocation.
- Could be read by another process with sufficient OS privileges while the command is running.

### BDK Security Notes

This wallet builds on the [Bitcoin Dev Kit](https://bitcoindevkit.org). Consult the BDK project's own security documentation and responsible-disclosure policy before using any BDK-based software in a sensitive context:

👉 <https://bitcoindevkit.org>
