# Liberté

Application de communication décentralisée : client de bureau Tauri, interface React et réseau Rust/libp2p.

La branche 2.4.0 est une version de stabilisation avec un protocole incompatible avec 2.3. Lire [la migration](wiki/Migration-2.4.md) avant de mettre à jour plusieurs appareils. Les résultats de CI et les essais sur deux appareils doivent être consultés avant toute utilisation sensible. Ce dépôt ne constitue pas une certification de sécurité.

## Fonctionnalités et limites

| Domaine | Implémentation et limite |
| --- | --- |
| Identité | Clés Ed25519 générées localement, sans compte central obligatoire. L'accès à la machine permet d'accéder aux clés locales. |
| Messagerie | Enveloppe chiffrée XChaCha20-Poly1305, auteur signé, contrôle du canal, de la date et des rejeux. La clé de canal reste partagée et durable. |
| Découverte | Réseau local et adresses de pairs dans les invitations. Les réseaux distants peuvent nécessiter un relais et une configuration réseau. |
| Invitations | Codes signés contenant un secret d'accès, limités dans le temps. Ils ne sont pas chiffrés pour leur destinataire et doivent être transmis confidentiellement. |
| Audio et vidéo | WebRTC dans le client, signalisation authentifiée. Permissions micro/caméra et connectivité ICE nécessaires. Aucun SFU de production n'est annoncé. |
| Fichiers et sauvegardes | Stockage local et API de relais. Les opérations serveur sont authentifiées ; une sauvegarde doit être testée par une restauration avant d'être considérée comme fiable. |
| Serveur | API HTTP, relais P2P et stockage. Utiliser un proxy HTTPS pour l'accès distant. |
| Mise à jour | Installateurs manuels. Les mises à jour automatiques exigent une paire de clés de signature configurée et les artefacts signés vérifiés par la release. |

Les adresses IP sont visibles par les pairs directs. Il n'y a pas de confidentialité persistante au niveau des messages après compromission d'une clé de canal, ni de suppression garantie chez les destinataires.

## Structure

- `liberte-shared` : identité, cryptographie et protocole.
- `liberte-net` : découverte, transport et événements P2P.
- `liberte-media` : composants média ; les flux WebRTC du client utilisent le moteur du navigateur.
- `liberte-store` : SQLite, migrations et sauvegardes.
- `liberte-client` : commandes Tauri et liaison avec le réseau.
- `liberte-server` : API et relais.
- `frontend` : React, Zustand et WebRTC.

## Développement

Prérequis : Rust **1.99.1** via rustup, Node.js **22**, Python **3.12+** pour les outils CI et les dépendances natives Tauri de la plateforme.

```bash
cd frontend
npm ci
npm test
npm run typecheck
npm run build
cd ..
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cd crates/liberte-client
npm --prefix ../../frontend run tauri -- dev
```

Si `Cargo.lock` manque sur la branche de migration, exécuter `python scripts/ci.py lockfile`, examiner le résultat et versionner le fichier. Le workflow CI peut aussi fournir cet artefact. Les releases restent bloquées tant que le lockfile n'est pas versionné. Ne pas remplacer systématiquement `--locked` par une résolution libre.

Sur Ubuntu, installer notamment `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libgtk-3-dev libglib2.0-dev libasound2-dev patchelf libssl-dev pkg-config`. Sur Windows, installer les outils de compilation C++ de Visual Studio et WebView2.

## Serveur auto-hébergé

Depuis un checkout avec son `Cargo.lock` versionné :

```bash
cd docker
cp .env.example .env
docker compose up --build -d
```

L'API est publiée sur `127.0.0.1:8080` par défaut. Le transport QUIC utilise UDP 4001. L'API HTTP n'est pas un serveur TURN et un serveur TURN ne découvre pas les pairs P2P. Le relais TURN optionnel pour WebRTC est décrit dans [TURN](wiki/TURN.md).

## Vérification et distribution

La CI exécute les tests frontend, les vérifications Rust, des tests d'authentification HTTP et le chargement/démarrage d'une image Docker réellement exportée. Elle ne remplace pas un appel entre deux appareils derrière des NAT différents, ni un audit cryptographique indépendant.

Le workflow Release attend tous ces contrôles, vérifie le tag et les versions, puis construit les installateurs. Les images Docker sont distribuées **séparément pour AMD64 et ARM64**, avec sommes SHA-256. Une release non signée ne publie pas de `latest.json`.

- [Démarrage](wiki/Getting-Started.md)
- [Configuration](wiki/Configuration.md)
- [Sécurité](SECURITY.md)
- [Migration 2.4](wiki/Migration-2.4.md)
- [Publication et signatures](wiki/Releasing.md)
- [Changelog](CHANGELOG.md)

## Licence

AGPL-3.0, voir [LICENSE](LICENSE).

