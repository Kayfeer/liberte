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
| MAX_HTTP_CONCURRENCY | Requêtes simultanées, 2 par défaut |
| LIVEKIT_URL | URL publique WSS du SFU optionnel ; WS accepté uniquement sur loopback en développement |
| LIVEKIT_API_KEY | Identifiant API du SFU, configuré avec son URL et son secret |
| LIVEKIT_API_SECRET | Secret API LiveKit, conservé uniquement côté serveur |
| RUST_LOG | Niveau de journalisation |
| HTTP_BIND | Variable Compose uniquement : interface hôte publiée, 127.0.0.1 par défaut |

Les noms et validations faisant autorité se trouvent dans `crates/liberte-server/src/config.rs`. Un paramètre configuré ne garantit pas que la politique désirée a été testée dans votre topologie.

Une configuration invalide empêche le démarrage. Un jeton administrateur non vide doit contenir au moins 32 caractères. `MAX_PEERS` accepte 1 à 4 096, pas zéro ; `MAX_HTTP_CONCURRENCY` accepte 1 à 64.

## Connexion P2P et relais Internet

Le client découvre les pairs sur le LAN. Sur Internet, fournir une adresse joignable directement ou utiliser un relais P2P. L'API HTTP, le relais QUIC et TURN remplissent des rôles distincts : l'URL du serveur de sauvegarde ne suffit pas à connecter les pairs.

1. Sur le serveur, ouvrir UDP 4001 et configurer `PUBLIC_RELAY_ADDR` avec l'adresse publique réelle. Exemple à adapter : `PUBLIC_RELAY_ADDR=/ip4/203.0.113.10/udp/4001/quic-v1`. L'adresse `203.0.113.10` sert uniquement à la documentation. Le serveur peut écouter sur `LISTEN_ADDR=/ip4/0.0.0.0/udp/4001/quic-v1`, mais une adresse non spécifiée n'est pas une adresse publique.
2. Démarrer le serveur et relever `peer_id` dans la ligne `Starting relay server` de ses journaux (`docker compose logs liberte-relay`). Conserver le volume de données : `relay-identity.key` maintient cet identifiant après redémarrage.
3. Dans les paramètres réseau de chaque client, saisir sous **Adresse d'un pair ou d'un relais** une adresse complète telle que `/dns4/relay.votre-domaine.example/udp/4001/quic-v1/p2p/<PEER_ID>`, puis cliquer **Se connecter**. Remplacer le domaine et `<PEER_ID>` ; ne pas utiliser de crochets dans l'adresse réelle. Une adresse `/ip4/...` fonctionne également.
4. Attendre l'apparition d'une adresse contenant `/p2p-circuit` dans **Vos adresses à partager**. La connexion au serveur et la réservation d'un circuit sont deux étapes distinctes. Sans cette adresse, vérifier l'annonce publique, UDP 4001 et les journaux avant de générer une invitation pour un correspondant derrière un autre NAT.
5. Créer le canal puis une invitation récente. Les adresses de circuit sont prioritaires dans l'invitation. Après l'adhésion du correspondant, vérifier un message reçu dans chaque sens. Le statut réseau indique « relayé » dès qu'au moins une connexion utilise un circuit, même si le lien vers le serveur relais est direct.

Les adresses explicites des invitations et des connexions manuelles sont enregistrées localement (128 pairs, 8 adresses par pair). Elles sont retentées au démarrage et après une déconnexion, avec un délai progressif plafonné à 60 secondes. Elles ne sont pas exportées dans les sauvegardes. Si les adresses IP ou ports ont changé, utiliser une nouvelle invitation ou reconnecter l'adresse du relais. Plusieurs clients sur une machine peuvent coexister : si le port par défaut 4001 est occupé, le second choisit un port disponible et partage ce port réel.

### Bootstrap optionnel au démarrage

La connexion manuelle ci-dessus suffit ; aucun bootstrap public n'est fourni par défaut. Pour un déploiement administré, copier `assets/bootstrap_peers.toml`, ajouter des lignes `addr = "..."` avec les identifiants des pairs, puis définir `LIBERTE_BOOTSTRAP_PEERS` sur le chemin absolu de ce fichier **dans l'environnement du processus client** avant son lancement. Un exemple commenté est fourni ; le fichier du dépôt n'est pas chargé automatiquement.

Les formes `/dns`, `/dns4` et `/dns6` sont résolues par DNS-over-HTTPS, sans repli sur le DNS système. `/dnsaddr` n'est pas pris en charge. La résolution du bootstrap est bornée à 8 adresses résolues simultanément et 6 secondes au total (5 secondes par adresse). Une adresse non résolue est signalée dans les journaux et n'empêche pas le démarrage local ; la retenter ensuite avec **Se connecter**, ou redémarrer après correction. Les adresses résolues et les adresses IP sont ensuite reconnectées avec backoff. Le nom DNS est relu à chaque démarrage depuis le fichier bootstrap ; les adresses du cache de connexion manuelle sont les IP résolues au moment de leur enregistrement.

## HTTPS

Le port 8080 est destiné au réseau local de la machine hôte ou à un proxy inverse. Pour accéder à l'API à distance, fournir une URL HTTPS et un certificat valide. Une signature de requête authentifie l'auteur ; elle ne chiffre pas à elle seule les en-têtes et les métadonnées HTTP.

## ICE et TURN

Le client permet de renseigner les serveurs ICE. Aucun fournisseur tiers n'est nécessaire par défaut. Un exemple auto-hébergé se trouve dans [TURN](TURN.md). Les identifiants TURN sont distincts des clés d'identité, de l'API administrateur et des clés de mise à jour.

## SFU LiveKit

Configurer les trois variables `LIVEKIT_*` ensemble, ou les laisser toutes vides pour désactiver le SFU. Le client obtient un jeton court auprès de `POST /media/token`, avec une requête signée. Le secret API n'est pas transmis au client. L'exemple [LiveKit](LiveKit.md) décrit le déploiement mono-noeud et les ports ; la validité d'un jeton ne remplace pas les clés de chiffrement du canal.

## Sauvegardes

Conserver une copie hors de l'appareil et tester périodiquement la restauration. Une planification enregistrée dans les paramètres n'est pas une preuve d'exécution : vérifier le dernier résultat et le fichier produit. Un serveur relais n'est pas un service de sauvegarde avec garantie de conservation.

Les exports portables sont chiffrés avec XChaCha20-Poly1305 et une clé dérivée d'une phrase secrète d'au moins 12 caractères par Argon2id. Conserver cette phrase séparément. Les sauvegardes automatiques sont liées à la clé d'identité : une sauvegarde distante seule ne suffit pas après perte complète de l'appareil. Les profils exportés n'incluent pas les octets des pièces jointes ; sauvegarder aussi les fichiers nécessaires.

L'import de formats bruts historiques nécessite une option explicite. Ne pas supprimer une ancienne sauvegarde avant validation de la restauration du nouveau format.

