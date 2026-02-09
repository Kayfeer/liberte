# Liberté

**Alternative souveraine et décentralisée à Discord.**

Communication chiffrée de bout en bout, peer-to-peer, sans serveur central obligatoire.

[English version below](#english)

---

## Présentation

Liberté est une application de communication P2P qui ne dépend d'aucune infrastructure centralisée. Les messages, appels audio/vidéo et fichiers transitent directement entre les utilisateurs via un réseau chiffré. Aucun serveur ne peut lire vos conversations.

### Fonctionnalités

- **Messagerie E2EE** — Chiffrement XChaCha20-Poly1305 avec dérivation de clés BLAKE3
- **Identité Ed25519** — Pas de compte, pas d'email. Votre clé publique est votre identité
- **Réseau P2P** — libp2p avec transport QUIC, GossipSub, Kademlia, Relay v2 et DCUtR
- **DNS sécurisé** — Résolution DNS-over-HTTPS uniquement (Cloudflare/Google), bypass du DNS système
- **Appels audio/vidéo** — WebRTC avec chiffrement E2EE des frames (insertable streams)
- **Transfert de fichiers** — Direct en P2P, ou via relais chiffré (premium)
- **Self-hosted** — Hébergez votre propre noeud relais/SFU avec Docker
- **Premium (0.99€/mois)** — Relais SFU, stockage de blobs chiffrés, support du projet

### Stack technique

| Composant | Technologie |
|-----------|------------|
| Backend | Rust, libp2p, webrtc-rs, axum |
| Client | Tauri v2 |
| Frontend | React 19, Zustand, Tailwind CSS |
| Crypto | XChaCha20-Poly1305, Ed25519, Noise_XX, BLAKE3 |
| Base de données | SQLite (chiffrement applicatif) |
| Réseau | QUIC, GossipSub, Kademlia, DNS-over-HTTPS |

## Architecture

```
crates/
├── liberte-shared    # Types partagés, crypto, identité, filtrage CSAM
├── liberte-net       # Réseau libp2p, swarm, découverte, relais
├── liberte-media     # Audio/vidéo WebRTC, SFU client, insertable streams
├── liberte-store     # SQLite, CRUD, migrations
├── liberte-client    # Application Tauri (point d'entrée desktop)
└── liberte-server    # Serveur relais + SFU + blob store (Docker)

frontend/             # React + Vite + Tailwind
```

## Build

### Prérequis

- Rust 1.75+
- Node.js 20+
- pnpm (ou npm/yarn)

### Développement

```bash
# Vérifier que tout compile
cargo check --workspace

# Lancer les tests
cargo test --workspace

# Lancer le client Tauri en dev
cd frontend && pnpm install && cd ..
cargo tauri dev
```

### Serveur self-hosted

```bash
cd docker
cp .env.example .env
# Éditer .env selon vos besoins
docker-compose up -d
```

## Licence

AGPL-3.0 — voir [LICENSE](LICENSE)

---

<a id="english"></a>

# Liberté (English)

**Sovereign, decentralized alternative to Discord.**

End-to-end encrypted communication, peer-to-peer, no mandatory central server.

## Overview

Liberté is a P2P communication app that doesn't rely on any centralized infrastructure. Messages, audio/video calls, and files travel directly between users through an encrypted network. No server can read your conversations.

### Features

- **E2EE messaging** — XChaCha20-Poly1305 encryption with BLAKE3 key derivation
- **Ed25519 identity** — No account, no email. Your public key is your identity
- **P2P network** — libp2p with QUIC transport, GossipSub, Kademlia, Relay v2, and DCUtR
- **Secure DNS** — DNS-over-HTTPS only (Cloudflare/Google), bypasses system DNS
- **Audio/video calls** — WebRTC with E2EE frame encryption (insertable streams)
- **File transfer** — Direct P2P, or via encrypted relay (premium)
- **Self-hosted** — Run your own relay/SFU node with Docker
- **Premium ($0.99/month)** — SFU relay, encrypted blob storage, project support

### Tech stack

| Component | Technology |
|-----------|-----------|
| Backend | Rust, libp2p, webrtc-rs, axum |
| Client | Tauri v2 |
| Frontend | React 19, Zustand, Tailwind CSS |
| Crypto | XChaCha20-Poly1305, Ed25519, Noise_XX, BLAKE3 |
| Database | SQLite (application-layer encryption) |
| Network | QUIC, GossipSub, Kademlia, DNS-over-HTTPS |

## Architecture

```
crates/
├── liberte-shared    # Shared types, crypto, identity, CSAM filtering
├── liberte-net       # libp2p networking, swarm, discovery, relay
├── liberte-media     # WebRTC audio/video, SFU client, insertable streams
├── liberte-store     # SQLite, CRUD, migrations
├── liberte-client    # Tauri desktop app (entry point)
└── liberte-server    # Relay server + SFU + blob store (Docker)

frontend/             # React + Vite + Tailwind
```

## Build

### Requirements

- Rust 1.75+
- Node.js 20+
- pnpm (or npm/yarn)

### Development

```bash
# Check that everything compiles
cargo check --workspace

# Run tests
cargo test --workspace

# Run the Tauri client in dev mode
cd frontend && pnpm install && cd ..
cargo tauri dev
```

### Self-hosted server

```bash
cd docker
cp .env.example .env
# Edit .env to your needs
docker-compose up -d
```

## License

AGPL-3.0 — see [LICENSE](LICENSE)
