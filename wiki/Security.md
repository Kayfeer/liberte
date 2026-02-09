# Security

Voir le fichier [SECURITY.md](../SECURITY.md) à la racine du dépôt pour la politique complète.

## Résumé

| Composant         | Technologie                              |
|-------------------|------------------------------------------|
| Chiffrement E2EE  | XChaCha20-Poly1305                       |
| Échange de clés   | Noise Protocol (XX)                      |
| Identité          | Ed25519 (clé locale)                     |
| Transport         | QUIC + TLS 1.3 (libp2p)                 |
| Dérivation de clé | BLAKE3                                   |
| Base de données   | SQLite WAL (données chiffrées au repos)  |

## Signaler une vulnérabilité

**Ne pas ouvrir d'issue publique.** Contactez le mainteneur via message privé GitHub ou par email (voir SECURITY.md).

## Bonnes pratiques utilisateur

- **Sauvegardez votre profil** régulièrement (Paramètres → Exporter le profil)
- **N'exportez jamais votre clé privée** sur un canal non sécurisé
- **Activez la sauvegarde automatique** dans les paramètres
- Gardez l'application à jour pour bénéficier des correctifs de sécurité
