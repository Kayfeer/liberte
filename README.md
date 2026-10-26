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
| Audio et vidéo | WebRTC entre pairs ou SFU LiveKit auto-hébergé optionnel avec chiffrement côté client. Permissions micro/caméra et connectivité ICE nécessaires. Le SFU chiffré est refusé dans le client Linux tant que son moteur ne garantit pas les transformations E2EE. Voir les limites et essais de validation dans [LiveKit](wiki/LiveKit.md). |
| Fichiers et sauvegardes | Stockage local et API de relais. Les opérations serveur sont authentifiées ; une sauvegarde doit être testée par une restauration avant d'être considérée comme fiable. |
| Serveur | API HTTP, relais P2P et stockage. Utiliser un proxy HTTPS pour l'accès distant. |
| Mise à jour | Installateurs manuels. Les mises à jour automatiques exigent une paire de clés de signature configurée et les artefacts signés vérifiés par la release. |

Les adresses IP sont visibles par les pairs directs. Il n'y a pas de confidentialité persistante au niveau des messages après compromission d'une clé de canal, ni de suppression garantie chez les destinataires.

La messagerie fonctionne entre participants connectés. « Envoyé » signifie que GossipSub a accepté la publication, pas que chaque destinataire l'a reçue ou lue. L'historique est local : il n'y a ni boîte d'envoi hors ligne ni rattrapage automatique des messages manqués. Sans pair abonné, l'envoi renvoie une erreur. Les fichiers disposent, eux, d'une confirmation après vérification de leur contenu.

Pour utiliser un relais Internet, suivre le [parcours de configuration P2P](wiki/Configuration.md#connexion-p2p-et-relais-internet). Renseigner l'URL HTTP du serveur ne connecte pas automatiquement son relais QUIC.

## Structure

- `liberte-shared` : identité, cryptographie et protocole.
- `liberte-net` : découverte, transport et événements P2P.
- `liberte-media` : composants média ; les flux WebRTC du client utilisent le moteur du navigateur.
- `liberte-store` : SQLite, migrations et sauvegardes.
- `liberte-client` : commandes Tauri et liaison avec le réseau.
- `liberte-server` : API et relais.
- `frontend` : React, Zustand et WebRTC.

## Prérequis des clients

Le client macOS exige **macOS 13.3 minimum**, conformément à la configuration du bundle. Maintenir macOS et son moteur WebKit à jour.

L'interface utilise Tailwind CSS 4, dont les [prérequis de navigateur](https://tailwindcss.com/docs/compatibility) comprennent Safari 16.4 et Chromium 111. Sur Windows, utiliser WebView2 à jour. Sur Linux, utiliser WebKitGTK récent ; son numéro d'API `4.1` ne désigne pas la version du moteur WebKit.

Sous Linux, les appels P2P exigent aussi un moteur compilé avec WebRTC. La série [WebKitGTK 2.54 désactive ce support](https://webkitgtk.org/2026/09/16/webkitgtk-2.54-highlights.html) pendant sa transition vers LibWebRTC ; le numéro de version seul ne garantit donc pas la disponibilité des appels. Le parcours natif Ubuntu 22.04 avec WebKitGTK 2.50.4 échoue actuellement avant capture, car `RTCPeerConnection` est absent, malgré un contexte sécurisé et `getUserMedia` disponible. Les appels Linux ne sont pas validés ; les résultats natifs identifient le moteur effectivement testé et bloquent la livraison tant que ce parcours échoue.

Une compilation du paquet ne valide pas le rendu, les permissions micro/caméra ou l'E2EE dans la WebView native. Ces essais restent nécessaires sur chaque plateforme.

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
node ../../frontend/node_modules/@tauri-apps/cli/tauri.js dev
```

Si `Cargo.lock` manque sur la branche de migration, exécuter `python scripts/ci.py lockfile`, examiner le résultat et versionner le fichier. Le workflow CI peut aussi fournir cet artefact. Les releases restent bloquées tant que le lockfile n'est pas versionné. Ne pas remplacer systématiquement `--locked` par une résolution libre.

Sur Ubuntu, installer notamment `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev libgtk-3-dev libglib2.0-dev libasound2-dev patchelf libssl-dev pkg-config`. Pour les médias WebKitGTK, installer aussi `gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-libav gstreamer1.0-nice` et vérifier le support WebRTC de la distribution. Sur Windows, installer les outils de compilation C++ de Visual Studio et WebView2.

## Serveur auto-hébergé

Depuis un checkout avec son `Cargo.lock` versionné :

```bash
cd docker
cp .env.example .env
docker compose up --build -d
```

L'API est publiée sur `127.0.0.1:8080` par défaut. Le transport QUIC utilise UDP 4001. L'API HTTP n'est pas un serveur TURN et un serveur TURN ne découvre pas les pairs P2P. Le relais TURN optionnel pour WebRTC est décrit dans [TURN](wiki/TURN.md). Le SFU optionnel et sa configuration HTTPS/WSS sont décrits dans [LiveKit](wiki/LiveKit.md).

## Vérification et distribution

La CI exécute les tests frontend, les vérifications Rust, des tests d'authentification HTTP, des échanges média dans Chromium et le chargement/démarrage d'une image Docker réellement exportée. Elle lance aussi deux vrais clients Linux via WebDriver pour les invitations, les messages, les réactions, les fichiers, la reconnexion et la restauration depuis le vrai serveur. Un binaire QA distinct utilise des périphériques synthétiques pour tester les flux WebRTC natifs et leur arrêt ; il n'est pas distribué. Les résultats et captures du parcours natif sont conservés dans les artefacts `native-smoke-linux`. Elle ne remplace pas un appel entre deux appareils derrière des NAT différents, un essai des périphériques dans chaque WebView native, ni un audit cryptographique indépendant.

Les jobs desktop produisent des paquets de test Windows, Linux et macOS dans les artefacts de l'exécution GitHub Actions. Leur nom et `build.json` identifient le commit ; vérifier aussi `SHA256SUMS`. Ces paquets CI ne sont pas signés ni publiés comme release et ne proposent pas de mise à jour automatique.

Le workflow Release attend tous ces contrôles, vérifie le tag et les versions, puis construit les installateurs. Les images Docker sont distribuées **séparément pour AMD64 et ARM64**, avec sommes SHA-256. Une release non signée ne publie pas de `latest.json`.

- [Démarrage](wiki/Getting-Started.md)
- [Configuration](wiki/Configuration.md)
- [Sécurité](SECURITY.md)
- [Migration 2.4](wiki/Migration-2.4.md)
- [Publication et signatures](wiki/Releasing.md)
- [Changelog](CHANGELOG.md)

## Licence

AGPL-3.0, voir [LICENSE](LICENSE).

