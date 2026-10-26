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
- Client Linux lancé réellement en CI : création d'identité et de canal, puis persistance après redémarrage.
- Tests, compilation, formatage, Clippy, audits et smoke Docker validés sur le même commit.
- Paquets Windows, Linux et macOS construits sur ce commit et conservés en artefacts CI.
- Documentation de configuration, migration et limites conforme au comportement livré.

## Conditions de validation

Les tests distinguent les échanges réseau/média réels des tests d'interface avec IPC simulés. Un build ne prouve pas l'utilisation des périphériques physiques sur chaque plateforme. Les essais restants sont signalés.

Serveur distant, HTTPS/WSS, TURN et LiveKit nécessitent une configuration opérateur. Aucune clé de signature inventée, aucun merge, déploiement public ou release officielle sans instruction correspondante.

## Suivi et preuve de livraison

La [PR #2](https://github.com/Kayfeer/liberte/pull/2) regroupe l'implémentation et le bilan de validation. Les [exécutions CI de la branche](https://github.com/Kayfeer/liberte/actions?query=branch%3Acodex%2Fcomplete-liberte) fournissent les résultats et les installateurs, identifiés par commit et SHA-256.

La livraison est validée lorsque tous les contrôles du dernier commit passent, y compris le lancement du client Linux. Les vérifications sur des périphériques physiques et des réseaux distincts restent explicitement séparées des preuves automatisées. Le document [Démarrer](wiki/Getting-Started.md) décrit leur parcours.
