# SFU LiveKit optionnel

Le mode LiveKit complète les appels WebRTC entre pairs. Il nécessite un serveur choisi et configuré par l'opérateur ; aucun service cloud ni compte tiers n'est préconfiguré. Le SDK client applique le chiffrement de bout en bout avec une clé issue du canal. La présence du code ne démontre pas sa compatibilité avec chaque WebView ou chaque réseau.

## Déploiement mono-noeud

Depuis `docker/`, compléter `.env` sans versionner les secrets :

```dotenv
LIVEKIT_URL=wss://media.example.net
LIVEKIT_API_KEY=remplacer-par-un-identifiant-aleatoire
LIVEKIT_API_SECRET=remplacer-par-un-secret-aleatoire-long
LIVEKIT_NODE_IP=203.0.113.10
LIVEKIT_HTTP_BIND=127.0.0.1
```

Le domaine et l'adresse ci-dessus sont des exemples réservés à la documentation. Les remplacer par votre domaine et votre IP publique. Générer le secret avec `python -c "import secrets; print(secrets.token_hex(32))"`. La même paire API est fournie au serveur Liberté et à LiveKit. Ne pas la placer dans les préférences du client.

```bash
docker compose -f docker-compose.yml -f docker-compose.livekit.yml config --quiet
docker compose -f docker-compose.yml -f docker-compose.livekit.yml up --build -d
```

L'image est figée à `livekit/livekit-server:v1.13.10`. Ce mode mono-noeud n'a pas besoin de Redis. Un déploiement distribué exige une architecture supplémentaire, une configuration Redis et une revue de capacité ; il n'est pas fourni par cet exemple.

| Port | Fonction et exposition |
| --- | --- |
| TCP 7880 | Signalisation HTTP/WebSocket, publiée sur loopback et accessible via un proxy TLS |
| TCP 7881 | Transport média ICE TCP, accès direct et règles pare-feu correspondantes |
| UDP 50000 à 50100 | Transport média ICE UDP, plage limitée à exposer et à adapter à la capacité |
| TCP 443 | HTTPS/WSS public du proxy, certificat valide pour le domaine média |

Configurer un proxy compatible WebSocket vers `127.0.0.1:7880`. Par exemple, dans une installation Caddy existante :

```caddyfile
media.example.net {
    reverse_proxy 127.0.0.1:7880
}
```

Ce fragment ne démarre pas un proxy et ne remplace pas la configuration DNS, les certificats, le pare-feu et les éventuelles règles NAT. `LIVEKIT_URL` est l'URL atteignable par les clients. Le nom du conteneur `livekit` n'est pas une URL publique. L'API Liberté doit également être accessible par HTTPS pour les clients distants.

Le mode `ws://127.0.0.1:7880` est réservé au développement local. Ne pas employer `--dev` ni les clés des tests CI en production. Le serveur annonce l'IP explicite `LIVEKIT_NODE_IP`, sans découverte d'IP par STUN. Le SDK du client reçoit la liste ICE configurée dans les préférences ; une liste vide doit rester explicite pour éviter les serveurs STUN par défaut de LiveKit. Un TURN auto-hébergé peut être ajouté via [TURN](TURN.md) selon les réseaux traversés.

## Accès et chiffrement

Les trois variables `LIVEKIT_URL`, `LIVEKIT_API_KEY` et `LIVEKIT_API_SECRET` sont obligatoires ensemble. Lorsqu'elles sont absentes, le serveur ne fournit pas de jeton SFU. `POST /media/token` exige une requête authentifiée, puis produit un jeton de courte durée, limité à l'identité et à la salle demandées. La preuve d'identité ne constitue pas, à elle seule, un annuaire central des membres de canal.

La confidentialité du contenu dépend aussi de la clé du canal et du fonctionnement E2EE du SDK dans la WebView. Les participants, IP, tailles et chronologie des flux restent observables par le SFU. Un membre ayant accès à la clé peut enregistrer ou retransmettre les médias. Une clé partagée ne fournit pas de révocation rétroactive ni de confidentialité persistante après sa compromission.

## Validation avant utilisation

Vérifier un appel à deux clients, la réception audio et vidéo, le mute, les périphériques, le partage d'écran et la reconnexion. Vérifier qu'une clé de canal incorrecte empêche le décodage du média et qu'une absence de support E2EE est signalée avant publication. Refaire ces essais sur Windows/WebView2, Linux/WebKitGTK et macOS/WKWebView, puis entre deux réseaux distincts avec les règles NAT réelles.

La CI utilise Chromium, des périphériques simulés et une instance LiveKit locale avec des identifiants de test. Ces essais ne certifient pas les WebViews natives, les autorisations système, les périphériques physiques ou un déploiement public.

Références officielles : [auto-hébergement LiveKit](https://docs.livekit.io/transport/self-hosting/), [déploiement distribué](https://docs.livekit.io/transport/self-hosting/distributed/), [chiffrement de bout en bout](https://docs.livekit.io/transport/encryption/).
