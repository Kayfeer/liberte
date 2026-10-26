# Relais TURN optionnel

TURN aide les flux WebRTC lorsque les pairs ne peuvent pas établir une connexion directe. Il ne remplace ni la découverte libp2p, ni le relais QUIC, ni l'API HTTP de sauvegarde.

L'exemple utilise Coturn 4.18.0-r0 et des identifiants TURN longue durée. Il est optionnel et aucun service tiers n'est activé automatiquement.

## Préparer le serveur

Utiliser une machine ayant une adresse IPv4 publique, un nom DNS et des règles de pare-feu maîtrisées. Dans `docker/.env`, définir :

```dotenv
TURN_REALM=turn.votre-domaine.example
TURN_PUBLIC_IP=203.0.113.10
TURN_USERNAME=liberte-client
TURN_PASSWORD=REMPLACER_PAR_UN_MOT_DE_PASSE_ALEATOIRE_LONG
```

Les valeurs ci-dessus sont des exemples réservés à la documentation. Générer un vrai mot de passe distinct de tous vos comptes. Ne jamais diffuser le fichier `.env` complet.

Autoriser TCP/UDP 3478 et UDP 49160 à 49200 sur le serveur. Les plages privées, loopback, link-local et multicast sont exclues comme destinations de relais ; cet exemple est destiné aux connexions Internet, pas au relais vers votre réseau interne.

```bash
cd docker
docker compose -f docker-compose.yml -f docker-compose.turn.yml up -d
```

Le démarrage exige explicitement les quatre paramètres TURN. Des quotas par utilisateur, un quota global et une limite de débit sont configurés. Ajuster ces valeurs en fonction des ressources et de vos essais.

## Configurer le client

Dans les paramètres ICE du client, ajouter un objet `RTCIceServer` :

```json
{
  "urls": [
    "turn:turn.votre-domaine.example:3478?transport=udp",
    "turn:turn.votre-domaine.example:3478?transport=tcp"
  ],
  "username": "liberte-client",
  "credential": "VOTRE_MOT_DE_PASSE_TURN"
}
```

Transmettre ces identifiants seulement aux utilisateurs autorisés et les renouveler en cas d'exposition. Le serveur reçoit les métadonnées du trafic relayé. Le média reste soumis à la sécurité de la connexion WebRTC ; ne pas confondre TURN avec un SFU qui termine les sessions.

## TLS et déploiement durable

L'exemple expose TURN UDP/TCP 3478 et désactive ses listeners TLS. Pour un endpoint `turns:`, ajouter un certificat valide et sa clé en montage lecture seule, retirer `--no-tls`, fournir `--cert`, `--pkey` et `--tls-listening-port=5349`, puis exposer TCP 5349. Un proxy HTTP classique ne relaie pas le protocole TURN.

Pour un service public, préférer des identifiants temporaires par utilisateur, un contrôle d'admission et de la supervision. La configuration statique fournie ne constitue pas un service commercial complet.

## Validation

Faire un appel depuis deux réseaux différents, puis vérifier qu'une paire ICE de type `relay` est réellement sélectionnée lorsqu'une connexion directe est impossible. Le test automatisé de deux onglets couvre la négociation WebRTC locale ; il ne certifie pas votre pare-feu, votre NAT ou votre hébergement TURN.

Sources : [Coturn](https://github.com/coturn/coturn), [configuration officielle](https://github.com/coturn/coturn/blob/master/examples/etc/turnserver.conf), [release Docker utilisée](https://github.com/coturn/coturn/releases/tag/docker/4.18.0-r0).
