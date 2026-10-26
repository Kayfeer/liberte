# Démarrer

Installer Rust 1.99.1 avec rustup, Node.js 22 et les dépendances natives Tauri. Le client macOS exige **macOS 13.3 minimum**. Le [README](../README.md) précise les moteurs WebView requis, les paquets Linux et les prérequis Windows.

```bash
git clone https://github.com/Kayfeer/liberte.git
cd liberte
cd frontend
npm ci
npm test
npm run typecheck
npm run build
cd ..
cargo test --workspace --locked
cd crates/liberte-client
node ../../frontend/node_modules/@tauri-apps/cli/tauri.js dev
```

Si `Cargo.lock` est absent pendant la migration, le générer une seule fois avec `python scripts/ci.py lockfile`, examiner le résultat et le versionner. Il ne faut pas ignorer les erreurs `--locked`.

## Premier lancement

Créer une identité puis un canal. Transmettre une invitation récente par un moyen confidentiel. Les deux appareils doivent utiliser une version de protocole compatible.

Sur un LAN, autoriser le trafic de découverte et le port de transport dans le pare-feu. Sur Internet, vérifier les adresses publiées, le NAT et le relais P2P. Pour les appels, configurer si nécessaire les serveurs ICE dans les paramètres et consulter [TURN](TURN.md).

## Vérifier un échange

1. Utiliser deux appareils ou deux profils réellement séparés.
2. Rejoindre le même canal et vérifier les deux identités.
3. Envoyer un message dans chaque sens, redémarrer et vérifier l'historique.
4. Tester l'envoi d'un fichier, puis une restauration de sauvegarde sur un profil de test.
5. Tester micro, caméra, mute, sortie d'appel et reconnexion.
6. Refaire l'essai depuis deux réseaux distincts.

Un affichage local ou un message « envoyé » ne remplace pas la réception vérifiée sur l'autre appareil.

## Connexion et messages après redémarrage

Les paramètres réseau proposent **Adresse d'un pair ou d'un relais** et **Se connecter**. Pour joindre un relais Internet, suivre les étapes de [configuration P2P](Configuration.md#connexion-p2p-et-relais-internet), puis créer l'invitation une fois l'adresse de circuit disponible. **URL du serveur auto-hébergé** configure séparément l'API HTTP.

Après redémarrage, le client restaure ses salons, leurs abonnements et les adresses explicites connues. La reconnexion ne télécharge pas les messages manqués. L'historique affiché provient de la base locale ; la sauvegarde restaure cet historique, sans interroger les autres participants.

L'envoi attend brièvement la propagation d'un abonnement après connexion. Sans pair abonné, il renvoie une erreur ; aucun message n'est conservé dans une file d'envoi différé. Une publication acceptée n'est pas un accusé de réception : si plusieurs participants sont dans le salon, un participant déconnecté peut manquer un message reçu par les autres. Il n'y a pas d'indicateur de lecture. La confirmation de réception des fichiers intervient séparément, après vérification du hash.

## Serveur

```bash
cargo build --release --locked --bin liberte-server
python scripts/ci.py smoke --binary target/release/liberte-server
```

Sous Windows, ajouter `.exe`. Pour Docker, utiliser les instructions du README et un checkout contenant le lockfile.

