# Configuration

Les paramètres du client sont locaux. Les changements de réseau, de sauvegarde et de média doivent être vérifiés par une opération réelle après enregistrement.

## Serveur

| Variable | Usage |
| --- | --- |
| HTTP_ADDR | Adresse d'écoute interne de l'API |
| LISTEN_ADDR | Multiadresse libp2p, par exemple QUIC sur UDP 4001 |
| BLOB_STORAGE_PATH | Répertoire de stockage serveur |
| INSTANCE_NAME | Nom affiché |
| PREMIUM_REQUIRED | Exiger l'autorisation premium pour les fonctions concernées |
| PAYMENT_SERVER_PUBKEY | Clé publique du signataire des tokens premium, si utilisé |
| ADMIN_TOKEN | Jeton administrateur ; vide désactive l'administration |
| REGISTRATION_OPEN | Politique d'admission des nouveaux pairs |
| MAX_PEERS | Limite configurée de pairs |
| PUBLIC_RELAY_ADDR | Multiadresse publique annoncée pour les réservations du relais Internet |
| MAX_BLOB_SIZE | Taille maximale d'un objet, 52 428 800 octets par défaut |
| MAX_USER_BYTES | Quota par propriétaire, 262 144 000 octets par défaut |
| MAX_TOTAL_BYTES | Quota total, 1 073 741 824 octets par défaut |
| MAX_OBJECTS | Nombre maximal d'objets, 10 000 par défaut |
| MAX_REPLAY_ENTRIES | Taille maximale du registre de rejeu HTTP, 10 000 par défaut |
| MAX_HTTP_CONCURRENCY | Requêtes simultanées, 8 par défaut |
| RUST_LOG | Niveau de journalisation |
| HTTP_BIND | Variable Compose uniquement : interface hôte publiée, 127.0.0.1 par défaut |

Les noms et validations faisant autorité se trouvent dans `crates/liberte-server/src/config.rs`. Un paramètre configuré ne garantit pas que la politique désirée a été testée dans votre topologie.

Une configuration invalide empêche le démarrage. Un jeton administrateur non vide doit contenir au moins 32 caractères. `MAX_PEERS` accepte 1 à 4 096, pas zéro ; `MAX_HTTP_CONCURRENCY` accepte 1 à 64.

## HTTPS

Le port 8080 est destiné au réseau local de la machine hôte ou à un proxy inverse. Pour accéder à l'API à distance, fournir une URL HTTPS et un certificat valide. Une signature de requête authentifie l'auteur ; elle ne chiffre pas à elle seule les en-têtes et les métadonnées HTTP.

## ICE et TURN

Le client permet de renseigner les serveurs ICE. Aucun fournisseur tiers n'est nécessaire par défaut. Un exemple auto-hébergé se trouve dans [TURN](TURN.md). Les identifiants TURN sont distincts des clés d'identité, de l'API administrateur et des clés de mise à jour.

## Sauvegardes

Conserver une copie hors de l'appareil et tester périodiquement la restauration. Une planification enregistrée dans les paramètres n'est pas une preuve d'exécution : vérifier le dernier résultat et le fichier produit. Un serveur relais n'est pas un service de sauvegarde avec garantie de conservation.

Les exports portables sont chiffrés avec XChaCha20-Poly1305 et une clé dérivée d'une phrase secrète d'au moins 12 caractères par Argon2id. Conserver cette phrase séparément. Les sauvegardes automatiques sont liées à la clé d'identité : une sauvegarde distante seule ne suffit pas après perte complète de l'appareil. Les profils exportés n'incluent pas les octets des pièces jointes ; sauvegarder aussi les fichiers nécessaires.

L'import de formats bruts historiques nécessite une option explicite. Ne pas supprimer une ancienne sauvegarde avant validation de la restauration du nouveau format.

