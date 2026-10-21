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
| RUST_LOG | Niveau de journalisation |
| HTTP_BIND | Variable Compose uniquement : interface hôte publiée, 127.0.0.1 par défaut |

Les noms et validations faisant autorité se trouvent dans `crates/liberte-server/src/config.rs`. Un paramètre configuré ne garantit pas que la politique désirée a été testée dans votre topologie.

## HTTPS

Le port 8080 est destiné au réseau local de la machine hôte ou à un proxy inverse. Pour accéder à l'API à distance, fournir une URL HTTPS et un certificat valide. Une signature de requête authentifie l'auteur ; elle ne chiffre pas à elle seule les en-têtes et les métadonnées HTTP.

## ICE et TURN

Le client permet de renseigner les serveurs ICE. Aucun fournisseur tiers n'est nécessaire par défaut. Un exemple auto-hébergé se trouve dans [TURN](TURN.md). Les identifiants TURN sont distincts des clés d'identité, de l'API administrateur et des clés de mise à jour.

## Sauvegardes

Conserver une copie hors de l'appareil et tester périodiquement la restauration. Une planification enregistrée dans les paramètres n'est pas une preuve d'exécution : vérifier le dernier résultat et le fichier produit. Un serveur relais n'est pas un service de sauvegarde avec garantie de conservation.

