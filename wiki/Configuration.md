# Configuration

## Paramètres de l'application

Les paramètres sont stockés localement dans la base SQLite, table `app_settings` (JSON).

| Paramètre               | Type     | Défaut       | Description |
|--------------------------|----------|--------------|-------------|
| `displayName`            | string?  | null         | Pseudo affiché dans le chat |
| `notificationsEnabled`   | boolean  | true         | Notifications système |
| `startMinimised`         | boolean  | false        | Démarrer minimisé |
| `audioInputDevice`       | string   | "default"    | Micro sélectionné |
| `audioOutputDevice`      | string   | "default"    | Haut-parleur sélectionné |
| `autoConnect`            | boolean  | true         | Connexion auto au réseau |
| `theme`                  | string   | "dark"       | Thème visuel (dark, light, midnight, custom) |
| `serverUrl`              | string   | ""           | URL du serveur relay (optionnel) |

## Thèmes

Liberté supporte 4 thèmes visuels :

- **Dark** (défaut) — thème sombre classique
- **Light** — thème clair
- **Midnight** — bleu profond
- **Custom** — couleurs personnalisables via l'interface

Les thèmes utilisent des variables CSS et peuvent être étendus.

## Sauvegarde automatique

La sauvegarde automatique des données (canaux, messages, clés) peut être activée dans les paramètres. L'intervalle est configurable (défaut : 30 minutes).

Les sauvegardes sont stockées localement dans le dossier de données de l'application.

## Serveur relay

Par défaut, Liberté fonctionne en **pair-à-pair pur**. Un serveur relay optionnel peut être configuré pour :

- Le NAT traversal (si les pairs ne peuvent pas se connecter directement)
- Le stockage de blobs premium
- Les appels SFU (groupe)

Configuration serveur : voir `crates/liberte-server/src/config.rs`.
