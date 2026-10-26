# Démarrer

Cette page concerne le **prototype `codex/liberte-cef`**, avec Tauri 3 alpha. Il n'est pas publié comme version stable ; les essais natifs restent à confirmer. Pour son périmètre et ses critères d'adoption, lire [Prototype CEF](CEF-Prototype.md).

Installer Rust 1.99.1 avec rustup, Node.js 22 et Python 3.12+ pour les outils CI. Le client Windows conserve WebView2 et exige les outils C++ de Visual Studio pour la compilation. Le client macOS conserve WKWebView et exige macOS 13.3 minimum.

## Préparer Ubuntu 24.04

Le moteur Linux utilise CEF et GTK 4.14 minimum. Les dépendances ci-dessous correspondent au socle de compilation du [workflow prototype](../.github/workflows/cef-prototype.yml) ; les outils de bureau virtuel de la CI ne sont pas nécessaires sur une session graphique normale.

```bash
sudo apt-get update
sudo apt-get install --no-install-recommends \
  build-essential pkg-config libgtk-4-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev libasound2-dev libssl-dev \
  libnss3-dev libxss1 libxkbcommon-dev libgbm-dev libdrm-dev \
  libxtst-dev libxrandr-dev libx11-dev libxcomposite-dev libxcursor-dev \
  libxi-dev libxdamage-dev libxext-dev patchelf binutils perl \
  xdg-desktop-portal xdg-desktop-portal-gtk
pkg-config --atleast-version=4.14 gtk4
pkg-config --modversion gtk4
```

Les composants auxiliaires gardent certaines dépendances GTK 3 ; cela ne change pas le choix CEF/GTK 4 du moteur. Les sélecteurs de fichiers utilisent le portail XDG : conserver `xdg-desktop-portal`, un backend adapté à la session de bureau et un bus D-Bus de session accessible. L'exemple installe le backend GTK ; un bureau KDE peut utiliser son backend FileChooser. [RFD](https://github.com/PolyMeilex/rfd/blob/0.16.0/src/lib.rs) décrit aussi un repli Zenity, mais le client utilise ici les sélecteurs du portail ; ce repli n'est pas une dépendance du paquet prototype. Le [portail FileChooser](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html) doit réellement présenter une fenêtre : une bibliothèque GTK seule ne fournit pas ce service. Les appels utilisent WebRTC dans Chromium et les entrées/sorties audio du système.

La première compilation récupère la distribution CEF, puis la CLI conserve ses ressources près du binaire. Prévoir de l'espace pour ce téléchargement et les sorties Cargo. Pour les commandes Cargo exécutées directement sous Linux :

```bash
export CEF_PATH="${XDG_CACHE_HOME:-$HOME/.cache}/tauri-cef"
mkdir -p "$CEF_PATH"
```

## Construire le prototype

```bash
git clone --branch codex/liberte-cef https://github.com/Kayfeer/liberte.git
cd liberte
cd frontend
npm ci
npm test
npm run typecheck
npm run build
cd ..
cargo check --workspace --all-targets --locked
```

Sous Linux, avant d'exécuter les tests qui chargent CEF :

```bash
export LD_LIBRARY_PATH="$PWD/target/debug${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
cargo test --workspace --locked
```

Sur Windows/macOS, lancer `cargo test --workspace --locked` dans l'environnement de compilation de la plateforme. Démarrer ensuite depuis la racine du dépôt :

```bash
cd crates/liberte-client
node ../../frontend/node_modules/@tauri-apps/cli/tauri.js dev
```

Le démarrage Linux exige un sandbox Chromium opérationnel. Si la politique système bloque le sandbox du binaire de développement, utiliser le parcours du paquet Debian et son helper officiel décrit dans [Prototype CEF](CEF-Prototype.md) ; ne pas désactiver le sandbox ou les protections du système. La disponibilité et la réussite de ce paquet restent soumises aux contrôles CI.

Si `Cargo.lock` est absent pendant la migration, le générer une seule fois avec `python scripts/ci.py lockfile`, examiner le résultat et le versionner. Il ne faut pas ignorer les erreurs `--locked`.

Les versions et le comportement CEF de référence sont décrits par [Tauri 3](https://v3.tauri.app/develop/cef/). Ne pas ajouter `qa-media-devices` à un build destiné à l'utilisateur : cette option de debug réserve CDP et les sources synthétiques aux essais isolés.

## Premier lancement

Créer une identité puis un canal. Transmettre une invitation récente par un moyen confidentiel. Les deux appareils doivent utiliser une version de protocole compatible.

Sur un LAN, autoriser le trafic de découverte et le port de transport dans le pare-feu. Sur Internet, vérifier les adresses publiées, le NAT et le relais P2P. Pour les appels, configurer si nécessaire les serveurs ICE dans les paramètres et consulter [TURN](TURN.md).

## Vérifier un échange

1. Utiliser deux appareils ou deux profils réellement séparés.
2. Rejoindre le même canal et vérifier les deux identités.
3. Envoyer un message dans chaque sens, redémarrer et vérifier l'historique.
4. Depuis les boutons de l'interface, ouvrir le sélecteur de fichier, envoyer un fichier, enregistrer le fichier reçu, puis essayer l'import d'une sauvegarde sur un profil de test. Vérifier aussi l'annulation des dialogues.
5. Tester micro, caméra, mute, sortie d'appel et reconnexion ; pour le SFU optionnel, suivre aussi [LiveKit](LiveKit.md) et son test de clé incorrecte.
6. Refaire l'essai depuis deux réseaux distincts.

Un affichage local ou un message « envoyé » ne remplace pas la réception vérifiée sur l'autre appareil. Cette liste décrit les essais à effectuer ; elle n'annonce pas leur réussite dans le prototype.

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

