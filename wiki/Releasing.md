# Construire et publier

Cette branche est le **prototype Tauri 3 / CEF**, sans publication stable ni validation native CEF acquise. Le workflow dédié est `cef-prototype.yml` ; lire [Prototype CEF](CEF-Prototype.md) pour les critères d'adoption.

Une publication est une opération distincte d'une pull request. Ne pas créer de tag du prototype avant sa qualification et l'adaptation de la chaîne de release. Les sections consacrées aux tags, signatures et archives Docker ci-dessous décrivent la chaîne historique à requalifier ; elles n'annoncent pas des paquets du prototype déjà publiés.

## Amorçage des lockfiles

La CI résout une seule fois un `Cargo.lock` manquant et le rend disponible dans l'artefact `cargo-lock`. Si les dépendances directes du frontend ne correspondent plus à `package-lock.json`, elle produit un candidat avec `npm install --package-lock-only --ignore-scripts`, dans `frontend-lock`. Cette réconciliation est également effectuée en présence d'overrides npm, car le lockfile ne conserve pas leur déclaration racine. Les builds utilisent ensuite les fichiers obtenus et les commandes strictes.

Examiner les deux artefacts, les reporter dans le dépôt et les versionner. Les logs `CODEX_BOOTSTRAP_FILE` contiennent aussi leur contenu compressé, encodé en Base64 et accompagné d'un SHA-256. Ces exports sont réservés aux lockfiles et correctifs de formatage ; ne jamais y ajouter des secrets.

Une erreur de formatage Rust produit `rustfmt-patch` sans transformer le résultat en succès. Les autres vérifications peuvent continuer pour donner un diagnostic complet, mais la gate finale reste rouge tant que toutes les vérifications n'ont pas réussi.

La release exige `Cargo.lock` versionné, un lockfile npm identique à celui du dépôt et des versions Cargo/Tauri/frontend identiques.

## Paquets de test du prototype

Le workflow prépare un paquet Debian Linux CEF et le serveur, dans `cef-prototype-linux-<commit>`, avec `SHA256SUMS` et `provenance.json`. Les builds Windows NSIS et macOS ARM64 `.app` conservent Wry ; leurs artefacts `wry-prototype-<plateforme>-<commit>` contiennent `SHA256SUMS` et `build.json`. Aucun de ces paquets n'est signé pour une distribution publique ; les mises à jour automatiques restent désactivées.

Ces paquets permettent les essais sur machine, sans créer de release ni de tag. Vérifier le résultat de l'exécution CI entière, le commit et les empreintes avant de les utiliser : la présence d'un artefact ne suffit pas à qualifier le client. Les builds Windows/macOS ne démontrent pas le fonctionnement natif de leur interface ou de leurs médias. Les prérequis Linux sont indiqués dans [Démarrer](Getting-Started.md#préparer-ubuntu-2404).

Les contrôles frontend, Rust, dépendances et serveur restent nécessaires. Les diagnostics natifs Linux sont conservés dans `cef-native-diagnostics-<commit>`. Les résultats du navigateur de test, lorsqu'ils sont verts, ne constituent pas une preuve du fonctionnement du binaire CEF.

## Validation native Linux à obtenir

Le paquet de distribution et le binaire QA sont construits séparément. Les parcours natifs du prototype utilisent un binaire debug avec `qa-media-devices` et le frontend embarqué. Playwright se connecte par CDP au processus CEF de cette application ; il ne lance pas un navigateur de remplacement. Les profils sont isolés et les commandes IPC, le transport P2P et le serveur sont réels.

Le parcours d'intégration doit vérifier une invitation avant tout redémarrage, les messages dans les deux sens, une réaction, un fichier de plusieurs blocs avec SHA-256 et accusé de réception, la reconnexion, puis une sauvegarde et une restauration depuis le serveur. Les rapports distinguent les actions d'interface des commandes IPC de production. Les chemins de fichiers transmis directement à ces commandes ne valident pas les fenêtres d'ouverture et d'enregistrement du bureau : celles-ci exigent un essai séparé avec un portail XDG fonctionnel.

Une interruption de transport peut relancer au plus deux fois une lecture explicitement autorisée ; les envois, clics et autres mutations ne sont jamais rejoués automatiquement. Une erreur de l'application reste un échec.

Les scénarios médias emploient des périphériques synthétiques Chromium, la vraie signalisation Rust et de vrais flux WebRTC. Les mesures attendues portent sur l'énergie audio reçue, les images vidéo décodées, le mute, la caméra et le raccrochage. CDP observe les objets `MediaStreamTrack` effectivement créés et leur arrêt ; cette observation ne mesure pas l'état matériel du microphone ou de la caméra.

Le scénario [SFU E2EE](LiveKit.md) doit également démontrer la réception avec la bonne clé, puis le refus d'une clé incorrecte. Pour ce second cas, une troisième application CEF QA héberge un participant de test distinct : elle doit continuer à émettre du média chiffré pendant que le client normal signale l'erreur et arrête ses captures, sans décoder le média adverse.

CDP et les sources synthétiques sont réservés au debug ; la compilation release refuse `qa-media-devices`. Les paquets distribués n'incluent pas ce binaire QA. Les rapports doivent conserver le commit, le SHA-256 du binaire et les limites de la preuve : périphériques physiques, consentement interactif, sélecteurs de fichiers du bureau et réseaux NAT distincts restent à essayer séparément. La présence de ces scénarios dans le dépôt ne prouve pas encore leur réussite.

## Chaîne de release historique à requalifier

1. Créer un nouveau tag complet correspondant à la version, par exemple `v2.4.0`. Ne pas déplacer un ancien tag.
2. Le workflow réutilise la CI sur le même commit.
3. Il construit les clients Windows/Linux, les serveurs Windows/Linux x64/Linux ARM64 et les images Docker AMD64/ARM64.
4. Les binaires serveur et les images Docker rechargées sont démarrés et soumis aux smoke tests HTTP.
5. Les signatures des bundles de mise à jour, si activées, sont vérifiées cryptographiquement avec Minisign.
6. Un seul job publie les fichiers après réussite de tous les jobs requis, avec `SHA256SUMS`.

Aucun job de compilation ne publie directement dans une release. Un échec interrompt la publication finale.

## Signature des mises à jour après qualification

Configurer sur le dépôt :

- secret `TAURI_SIGNING_PRIVATE_KEY` : clé privée Tauri ;
- secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` si cette clé est protégée ;
- variable `TAURI_UPDATER_PUBLIC_KEY` : contenu Base64 de la clé publique Tauri.

La clé publique est intégrée à la configuration de compilation. La clé privée reste dans l'environnement du build, sans être écrite dans les artefacts.

Si seule une des deux clés est configurée, le build échoue. Si aucune clé n'est configurée, les installateurs sont construits avec les mises à jour automatiques désactivées et aucun `latest.json` n'est publié. Ne pas présenter une release non signée comme disposant d'un updater utilisable.

Une clé de signature d'updater ne remplace pas une signature de code Windows Authenticode. Conserver la clé privée et son mot de passe hors du dépôt, avec une sauvegarde maîtrisée.

Référence de la chaîne historique : [Tauri 2 updater](https://v2.tauri.app/plugin/updater/), [Tauri action](https://github.com/tauri-apps/tauri-action).

## Archives Docker de la chaîne de release

La chaîne de release prévoit deux archives distinctes :

- `liberte-server-docker-linux-amd64.tar.gz`
- `liberte-server-docker-linux-arm64.tar.gz`

Choisir l'architecture de la machine, vérifier `SHA256SUMS`, puis :

```bash
gzip --decompress --stdout liberte-server-docker-linux-amd64.tar.gz > image.tar
docker load --input image.tar
```

Avant publication, chaque image doit avoir été exportée par `docker save`, retirée du cache local, rechargée puis démarrée dans son job de build. Le workflow ne compresse pas une sortie vide de build pour fabriquer un artefact de succès.
