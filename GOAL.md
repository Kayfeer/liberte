# Objectif de livraison Liberté 2.4

Terminer une application pleinement fonctionnelle sur `codex/complete-liberte`, PR #2. Ce document fixe les critères de livraison ; il ne prétend pas activer le mode natif `/goal` de Codex.

## Critères de fin

- Identité, profils, canaux et invitations utilisables.
- Messagerie authentifiée et chiffrée entre deux pairs, historique et erreurs visibles.
- Fichiers : consentement, intégrité, confirmation de réception, sauvegarde et nettoyage.
- Audio/vidéo WebRTC réels, périphériques, mute/caméra et libération des captures.
- SFU LiveKit optionnel avec E2EE obligatoire et refus des clés incompatibles.
- Sauvegardes locales, portables et distantes chiffrées, restauration validée.
- API serveur authentifiée, propriétaires isolés, quotas et état persistant.
- Interface vérifiée, sans commande factice ni succès simulé.
- Tests, compilation, formatage, Clippy, audits et smoke Docker validés sur le même commit.
- Paquets Windows, Linux et macOS construits sur ce commit et conservés en artefacts CI.
- Documentation de configuration, migration et limites conforme au comportement livré.

## Conditions de validation

Les tests distinguent les échanges réseau/média réels des tests d'interface avec IPC simulés. Un build ne prouve pas l'utilisation des périphériques physiques sur chaque plateforme. Les essais restants sont signalés.

Serveur distant, HTTPS/WSS, TURN et LiveKit nécessitent une configuration opérateur. Aucune clé de signature inventée, aucun merge, déploiement public ou release officielle sans instruction correspondante.

## État au 26 octobre 2026

Commit examiné : `e364ddcac3626b62dcc19d9aa394ae2ea48fd0ff`.
Source : https://github.com/Kayfeer/liberte/actions/runs/39698753266

Frontend, outils de livraison, Docker et paquets Windows/Linux/macOS passent. Audit Rust, qualité Rust et tests navigateur restent en échec. L'objectif reste ouvert jusqu'à correction et vérification.
