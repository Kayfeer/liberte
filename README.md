# Liberté

Cette branche contient le [prototype isolé Tauri 3 / Chromium pour Linux](wiki/CEF-Prototype.md). Ses prérequis, limites et critères de validation sont décrits dans ce document. La migration reste à qualifier avant publication.

Application de communication décentralisée : client de bureau Tauri, interface React et réseau Rust/libp2p.

La branche 2.4.0 est une version de stabilisation avec un protocole incompatible avec 2.3. Lire [la migration](wiki/Migration-2.4.md) avant de mettre à jour plusieurs appareils. Les résultats de CI et les essais sur deux appareils doivent être consultés avant toute utilisation sensible. Ce dépôt ne constitue pas une certification de sécurité.

## Fonctionnalités et limites

| Domaine | Implémentation et limite |
| --- | --- |
| Identité | Clés Ed25519 générées localement, sans compte central obligatoire. L'accès à la machine permet d'accéder aux clés locales. |
| Messagerie | Enveloppe chiffrée XChaCha20-Poly1305, auteur signé, contrôle du canal, de la date et des rejeux. La clé de canal reste partagée et durable. |
| Découverte | Réseau local et adresses de pairs dans les invitations. Les réseaux distants peuvent nécessiter un relais et une configuration réseau. |
| Invitations | Codes signés contenant un secret d'accès, limités dans le temps. Ils ne sont pas chiffrés pour leur destinataire et doivent être transmis confidentiellement. |
| Audio et vidéo | WebRTC entre pairs ou SFU LiveKit auto-hébergé optionnel avec chiffrement côté client. Permissions micro/caméra et connectivité ICE nécessaires. Le prototype CEF réouvre le parcours SFU Linux sous réserve de validation native du chiffrement ; les contrôles E2EE restent obligatoires. Voir les limites et essais de validation dans [LiveKit](wiki/LiveKit.md). |
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

| Plateforme | Moteur de cette branche et prérequis |
| --- | --- |
| Linux, prototype | **Ubuntu 24.04**, GTK **4.14 minimum**, Chromium embarqué via CEF. Les paquets de compilation et le démarrage sont détaillés dans [Démarrer](wiki/Getting-Started.md#préparer-ubuntu-2404). |
| Windows | Wry avec WebView2 à jour ; outils de compilation C++ de Visual Studio pour construire le client. |
| macOS | Wry avec WKWebView ; **macOS 13.3 minimum**, conformément au bundle. Maintenir le système à jour. |

L'interface utilise Tailwind CSS 4 et requiert un moteur récent. Le client Linux de cette branche utilise CEF ; les anciens prérequis WebKitGTK/GStreamer de la base Tauri 2 ne décrivent plus son moteur média. Le diagnostic ayant motivé ce changement reste dans [Prototype CEF](wiki/CEF-Prototype.md).

La migration, les permissions micro/caméra et les appels restent à qualifier dans les binaires natifs. Une compilation ou un scénario navigateur séparé ne constitue pas cette validation.

## Développement

Prérequis : Rust **1.99.1** via rustup, Node.js **22**, Python **3.12+** pour les outils CI et les dépendances natives de la plateforme. Les versions Tauri 3 alpha, du runtime et des plugins sont fixées dans les manifestes et lockfiles ; utiliser la CLI du projet. Sous Linux, préparer les paquets, `CEF_PATH` et le chemin des bibliothèques selon [Démarrer](wiki/Getting-Started.md) avant les commandes Cargo.

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

## Serveur auto-hébergé

Depuis un checkout avec son `Cargo.lock` versionné :

```bash
cd docker
cp .env.example .env
docker compose up --build -d
```

L'API est publiée sur `127.0.0.1:8080` par défaut. Le transport QUIC utilise UDP 4001. L'API HTTP n'est pas un serveur TURN et un serveur TURN ne découvre pas les pairs P2P. Le relais TURN optionnel pour WebRTC est décrit dans [TURN](wiki/TURN.md). Le SFU optionnel et sa configuration HTTPS/WSS sont décrits dans [LiveKit](wiki/LiveKit.md).

## Vérification et distribution

Le workflow dédié [CEF prototype](.github/workflows/cef-prototype.yml) prévoit les contrôles frontend/Rust, les audits de dépendances, le serveur réel et les parcours Linux natifs. Le pilote CDP se connecte au binaire CEF de l'application : les invitations, messages, fichiers, sauvegardes et redémarrages utilisent les commandes Rust. Les appels P2P et SFU sont des contrôles distincts ; leur préparation ne signifie pas qu'ils ont réussi.

Un binaire QA debug distinct active CDP et les périphériques synthétiques. Les mesures visent l'audio reçu, les images décodées, le refus d'une clé SFU incorrecte et l'arrêt des pistes observées. Il n'est pas distribué et ne valide ni les périphériques physiques ni les dialogues de permission interactifs.

Les rapports et captures sont prévus dans `cef-native-diagnostics-<commit>`. Le paquet Linux expérimental utilise `cef-prototype-linux-<commit>` avec `SHA256SUMS` et `provenance.json`. Les paquets Wry Windows/macOS utilisent `wry-prototype-<plateforme>-<commit>` et `build.json` ; leur compilation ne teste pas les appels natifs de ces systèmes. Vérifier la gate complète de l'exécution, pas seulement la présence d'un artefact.

Cette branche n'est pas une release : paquets non signés, mises à jour automatiques désactivées pour ces builds et adoption suspendue aux [preuves requises](wiki/CEF-Prototype.md). Les procédures historiques de distribution restent documentées dans [Publication et signatures](wiki/Releasing.md), sans valider la migration.

- [Démarrage](wiki/Getting-Started.md)
- [Configuration](wiki/Configuration.md)
- [Sécurité](SECURITY.md)
- [Migration 2.4](wiki/Migration-2.4.md)
- [Publication et signatures](wiki/Releasing.md)
- [Changelog](CHANGELOG.md)

## Licence

AGPL-3.0, voir [LICENSE](LICENSE).

