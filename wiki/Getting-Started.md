# Getting Started

## Prérequis

- **Rust** 1.82+ (via `rustup`)
- **Node.js** 20+ et **pnpm** (ou npm)
- **Tauri v2 CLI** : `cargo install tauri-cli --version "^2"`
- Sur Linux : `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `librsvg2-dev`

## Installation

```bash
# Cloner le dépôt
git clone https://github.com/Kayfeer/liberte.git
cd liberte

# Installer les dépendances frontend
cd frontend
pnpm install  # ou npm install
cd ..

# Lancer en mode développement
cargo tauri dev
```

## Build de production

```bash
cargo tauri build
```

L'installeur se trouvera dans `target/release/bundle/`.

## Serveur (optionnel)

```bash
# Build du serveur relay/SFU
cargo build --release -p liberte-server

# Lancer
./target/release/liberte-server
```

Ou via Docker :

```bash
cd docker
docker compose up -d
```

## Premier lancement

1. L'app affiche la page d'accueil **Welcome**
2. Choisissez un **pseudo** (optionnel, modifiable dans Paramètres)
3. Cliquez **Créer mon identité** — une clé Ed25519 est générée localement
4. Créez un canal ou rejoignez-en un via un code d'invitation
5. Partagez votre clé publique ou générez un lien d'invitation
