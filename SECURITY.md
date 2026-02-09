# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| < 0.2   | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability in **Liberté**, please report it
responsibly. **Do not open a public GitHub issue.**

### How to report

1. **Email**: Send details to **genesistrakd@gmail.com** (or contact the
   maintainer [@Kayfeer](https://github.com/Kayfeer) directly via GitHub
   private message).
2. Include:
   - A description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### What to expect

- **Acknowledgement** within **48 hours**
- A fix or mitigation within **7 days** for critical issues
- Credit in the release notes (unless you prefer to remain anonymous)

## Security Architecture

Liberté is designed with a **zero-trust, privacy-first** approach:

| Layer            | Technology                         |
| ---------------- | ---------------------------------- |
| End-to-end encryption | XChaCha20-Poly1305 (symmetric key per channel) |
| Key exchange     | Noise Protocol (XX pattern)        |
| Identity         | Ed25519 keypair (generated locally) |
| Transport        | QUIC + TLS 1.3 via libp2p          |
| Database         | SQLite WAL (local only)            |
| Key derivation   | BLAKE3 (DB key from Ed25519 secret)|

### Design Principles

- **No accounts, no servers required** — identity is a local Ed25519 keypair
- **No telemetry** — zero data sent to any central server
- **Secret key never leaves the device** unless explicitly exported by the user
- **Channel keys** are per-channel symmetric keys shared via encrypted invite codes
- **Messages are encrypted before storage** — the database stores only ciphertext
- **Peer-to-peer by default** — optional relay servers for NAT traversal only

## Threat Model

### In scope
- Message confidentiality (E2EE)
- Identity authenticity (Ed25519 signatures)
- Forward secrecy at transport level (QUIC/Noise)
- Local data protection (encrypted messages in DB)

### Out of scope (known limitations)
- **Metadata leakage**: IP addresses are visible to direct peers
- **No perfect forward secrecy** at the message level (channel keys are long-lived)
- **Local device compromise**: If an attacker has access to the device, the unencrypted secret key in the local database is exposed
- **No message deletion guarantee**: Messages stored locally by recipients cannot be remotely deleted

## Dependencies

Security-critical dependencies are regularly audited:

- `chacha20poly1305` — RustCrypto (audited)
- `ed25519-dalek` — well-established Rust implementation
- `blake3` — official BLAKE3 team implementation
- `snow` — Noise Protocol implementation
- `libp2p` — networking stack (widely used in IPFS, Filecoin, etc.)
- `rusqlite` — SQLite bindings
