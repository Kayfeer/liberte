# Architecture

## Vue d'ensemble

Liberté est organisé en **6 crates Rust** + un frontend **React/TypeScript** embarqué via **Tauri v2**.

```
┌──────────────────────────────────────────────────┐
│                   Frontend                       │
│         React 19 + TypeScript 5 + Vite 6         │
│              Tailwind CSS + Zustand              │
└─────────────────────┬────────────────────────────┘
                      │ Tauri IPC (invoke)
┌─────────────────────▼────────────────────────────┐
│              liberte-client (Tauri)               │
│  Commands · Events · State management             │
├───────────────┬──────────────┬────────────────────┤
│ liberte-net   │ liberte-media│ liberte-store      │
│ (libp2p)      │ (WebRTC)     │ (SQLite)           │
├───────────────┴──────────────┴────────────────────┤
│              liberte-shared                       │
│  Crypto · Protocol · Types · Identity             │
└───────────────────────────────────────────────────┘

┌───────────────────────────────────────────────────┐
│           liberte-server (optionnel)              │
│        Axum 0.7 · Relay · SFU · Blob store       │
└───────────────────────────────────────────────────┘
```

## Crates

### liberte-shared
Types partagés, protocole wire (serde bincode), primitives crypto (XChaCha20-Poly1305, BLAKE3, Ed25519), gestion d'identité.

### liberte-net
Stack réseau basé sur **libp2p** : transport QUIC, protocole de découverte mDNS/DHT, relay pour NAT traversal, pubsub GossipSub pour les messages de canal.

### liberte-media
Gestion audio/vidéo : WebRTC peer-to-peer (mesh) et SFU (Selective Forwarding Unit) pour les appels de groupe.

### liberte-store
Couche de persistance SQLite (rusqlite, WAL mode) : utilisateurs, canaux, messages chiffrés, clés de canal, blobs. Migrations automatiques.

### liberte-client
Application Tauri v2 : commandes IPC, gestion d'état, événements temps réel, plugins (shell, dialog, fs, notification, updater, process).

### liberte-server
Serveur optionnel (Axum 0.7) : relay libp2p, SFU WebRTC, blob store, rate limiting, gestion premium.

## Frontend

| Techno          | Version  |
|-----------------|----------|
| React           | 19       |
| TypeScript      | 5        |
| Vite            | 6        |
| Tailwind CSS    | 3        |
| Zustand         | 5        |
| Lucide Icons    | latest   |

### Stores Zustand
- `identityStore` — identité crypto (clé publique, pseudo)
- `messageStore` — canaux, messages, clés de canal
- `networkStore` — peers connectés, mode de connexion
- `navigationStore` — navigation interne
- `mediaStore` — appels audio/vidéo
- `themeStore` — thèmes (dark, light, midnight, custom)
- `backupStore` — sauvegarde automatique

## Chiffrement

```
Utilisateur A                          Utilisateur B
     │                                       │
     │  Ed25519 keypair                      │  Ed25519 keypair
     │         │                             │         │
     │  Noise XX handshake ──────────────────┤         │
     │         │                             │         │
     │  Channel key (XChaCha20-Poly1305) ────┤         │
     │         │                             │         │
     │  encrypt(channel_key, message) ───────┤         │
     │                                       │  decrypt(channel_key, ciphertext)
```
