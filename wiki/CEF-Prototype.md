# Prototype du moteur Linux CEF

Cette branche `codex/liberte-cef` est une migration isolée, non une version publiée. Elle vise à rétablir les appels Linux sans simuler les échanges et sans retirer le chiffrement. La branche `codex/complete-liberte` conserve la base Tauri 2.

## Problème démontré

Sur la base `5545513`, la CI `40336424290` a fait fonctionner deux applications Linux et le vrai serveur : invitations, messages bidirectionnels, réaction, fichier de 1 048 596 octets avec hash et ACK, reprise après redémarrage, synchronisation et restauration distante dans un profil neuf. Le commit de fusion testé possède le même arbre que cette base.

Le contrôle média a ensuite mesuré `isSecureContext=true`, `getUserMedia=function`, `AudioContext=function`, mais `RTCPeerConnection=undefined` dans WebKitGTK 2.50.4. Les scénarios Chromium séparés ne compensent pas cet échec natif. Les sources GStreamer ne permettent pas non plus de garantir les transformations E2EE du SFU.

## Migration proposée

- Tauri `3.0.0-alpha.5`, build `3.0.0-alpha.4`, API JS `3.0.0-alpha.3` et CLI `3.0.0-alpha.5`.
- Linux : runtime CEF `3.0.0-alpha.6`, Chromium embarqué, GTK 4.14 minimum ; runner Ubuntu 24.04.
- Windows et macOS : runtime Wry `3.0.0-alpha.5` et moteur système.
- Plugins v3 migrés ensemble. Le plugin updater est conservé sous `vendor/tauri-plugin-updater`, avec sa provenance officielle et le correctif ZIP 9 documentés dans `UPSTREAM.md`.
- Sandbox Chromium exigée. Le helper installé par le paquet Debian sert au test ; aucun assouplissement des protections du système ni option `--no-sandbox`.
- Style Chrome pour le consentement, navigation limitée aux origines de l'application et refus des fenêtres externes.
- CDP et périphériques synthétiques réservés au binaire QA de débogage. Le binaire de distribution est construit séparément.

## Preuves requises avant adoption

La compilation ne suffit pas. Les mêmes parcours de données doivent passer avec le véritable binaire CEF et les commandes Rust. Les appels doivent produire de l'énergie audio et des images vidéo décodées dans les deux sens, puis valider mute, reprise caméra et fin des pistes réellement créées. L'observation CDP des objets `MediaStreamTrack` ne constitue pas un essai des périphériques physiques.

Un appel SFU natif doit encore traverser le vrai endpoint Rust `/media/token`, le Worker E2EE et LiveKit ; le refus d'une clé incorrecte doit être démontré. Le chiffrement obligatoire et les refus de publications non chiffrées sont conservés.

Le prototype mesure aussi le CPU au repos : un défaut amont signalé dans l'issue Tauri 16189 peut occuper un cœur. Son absence n'est pas présumée. Audits, signatures de l'updater, locks, packaging et comportement Windows/macOS doivent être revérifiés avant de retenir cette migration.

Tauri 3 reste une préversion. Aucune validation physique, aucun déploiement, aucune signature officielle et aucune complétude de l'application ne sont déclarés par ce document.

## Sources

- [Tauri CEF](https://v3.tauri.app/develop/cef/)
- [Tag source Tauri 3 alpha 5](https://github.com/tauri-apps/tauri/tree/tauri-v3.0.0-alpha.5)
- [Test CEF officiel](https://github.com/tauri-apps/tauri/blob/tauri-v3.0.0-alpha.5/.github/workflows/test-cef-e2e.yml)
- [Consommation CPU signalée](https://github.com/tauri-apps/tauri/issues/16189)
- [Résultat natif de la base](https://github.com/Kayfeer/liberte/actions/runs/40336424290)
