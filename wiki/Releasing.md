# Construire et publier

Une publication est une opération distincte d'une pull request. Ne pas créer de tag tant que les résultats CI et les essais nécessaires n'ont pas été examinés.

## Amorçage des lockfiles

La CI résout une seule fois un `Cargo.lock` manquant et le rend disponible dans l'artefact `cargo-lock`. Si les dépendances directes du frontend ne correspondent plus à `package-lock.json`, elle produit un candidat avec `npm install --package-lock-only --ignore-scripts`, dans `frontend-lock`. Les builds utilisent ensuite les fichiers obtenus et les commandes strictes.

Examiner les deux artefacts, les reporter dans le dépôt et les versionner. Les logs `CODEX_BOOTSTRAP_FILE` contiennent aussi leur contenu compressé, encodé en Base64 et accompagné d'un SHA-256. Ces exports sont réservés aux lockfiles et correctifs de formatage ; ne jamais y ajouter des secrets.

Une erreur de formatage Rust produit `rustfmt-patch` sans transformer le résultat en succès. Les autres vérifications peuvent continuer pour donner un diagnostic complet, mais la gate finale reste rouge tant que toutes les vérifications n'ont pas réussi.

La release exige `Cargo.lock` versionné, un lockfile npm identique à celui du dépôt et des versions Cargo/Tauri/frontend identiques.

## Chaîne de release

1. Créer un nouveau tag complet correspondant à la version, par exemple `v2.4.0`. Ne pas déplacer un ancien tag.
2. Le workflow réutilise la CI sur le même commit.
3. Il construit les clients Windows/Linux, les serveurs Windows/Linux x64/Linux ARM64 et les images Docker AMD64/ARM64.
4. Les binaires serveur et les images Docker rechargées sont démarrés et soumis aux smoke tests HTTP.
5. Les signatures des bundles de mise à jour, si activées, sont vérifiées cryptographiquement avec Minisign.
6. Un seul job publie les fichiers après réussite de tous les jobs requis, avec `SHA256SUMS`.

Aucun job de compilation ne publie directement dans une release. Un échec interrompt la publication finale.

## Signature des mises à jour

Configurer sur le dépôt :

- secret `TAURI_SIGNING_PRIVATE_KEY` : clé privée Tauri ;
- secret `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` si cette clé est protégée ;
- variable `TAURI_UPDATER_PUBLIC_KEY` : contenu Base64 de la clé publique Tauri.

La clé publique est intégrée à la configuration de compilation. La clé privée reste dans l'environnement du build, sans être écrite dans les artefacts.

Si seule une des deux clés est configurée, le build échoue. Si aucune clé n'est configurée, les installateurs sont construits avec les mises à jour automatiques désactivées et aucun `latest.json` n'est publié. Ne pas présenter une release non signée comme disposant d'un updater utilisable.

Une clé de signature d'updater ne remplace pas une signature de code Windows Authenticode. Conserver la clé privée et son mot de passe hors du dépôt, avec une sauvegarde maîtrisée.

Documentation officielle : [Tauri updater](https://v2.tauri.app/plugin/updater/), [Tauri action](https://github.com/tauri-apps/tauri-action).

## Docker

Deux archives distinctes sont publiées :

- `liberte-server-docker-linux-amd64.tar.gz`
- `liberte-server-docker-linux-arm64.tar.gz`

Choisir l'architecture de la machine, vérifier `SHA256SUMS`, puis :

```bash
gzip --decompress --stdout liberte-server-docker-linux-amd64.tar.gz > image.tar
docker load --input image.tar
```

Chaque image a été exportée par `docker save`, retirée du cache local, rechargée puis démarrée dans son job de build. Le workflow ne compresse pas une sortie vide de build pour fabriquer un artefact de succès.
