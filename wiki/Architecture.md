# Architecture

Le client Tauri relie l'interface React aux commandes Rust. Le bridge réseau traite les événements libp2p, vérifie les enveloppes et met à jour les données locales et l'interface.

| Composant | Responsabilité |
| --- | --- |
| shared | Types, identité, enveloppes, invitations et primitives cryptographiques |
| net | Swarm libp2p, transport QUIC, découverte locale et connexion aux pairs |
| store | Données SQLite, migrations, fichiers et sauvegardes |
| client | Commandes Tauri, session utilisateur et bridge événementiel |
| frontend | Interface React, état utilisateur et flux WebRTC |
| server | API authentifiée, stockage et relais P2P |
| media | Composants média bas niveau ; leur présence ne signifie pas qu'un SFU est déployé |

## Frontières

Le réseau est non fiable. Les signatures et le chiffrement de canal sont vérifiés avant la consommation des événements. Une signature de transport libp2p ne remplace pas la signature de l'auteur du message.

Le stockage local n'offre pas de protection contre un appareil compromis. Le serveur de blobs reçoit du contenu déjà chiffré et doit aussi vérifier qui possède le droit de le lire, modifier ou supprimer.

Le réseau de signalisation et les flux WebRTC sont deux chemins distincts. Se connecter au serveur HTTP ou au serveur TURN ne garantit pas la découverte d'un pair libp2p.

## Exploitation

L'API HTTP utilise TCP 8080 et le relais QUIC UDP 4001. TURN, s'il est ajouté, utilise ses propres ports et identifiants. Les contrôles CI vérifient la compilation et quelques frontières serveur ; une connexion réelle entre deux appareils reste un test d'acceptation nécessaire.

