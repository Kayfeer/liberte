# Architecture

Cette branche expérimente Tauri 3 alpha. Linux utilise CEF/Chromium embarqué avec GTK 4.14 minimum ; Windows et macOS conservent Wry et leur WebView système. La [qualification du prototype](CEF-Prototype.md) reste distincte de sa compilation.

Le client Tauri relie l'interface React aux commandes Rust. Le bridge réseau traite les événements libp2p, vérifie les enveloppes et met à jour les données locales et l'interface.

| Composant | Responsabilité |
| --- | --- |
| shared | Types, identité, enveloppes, invitations et primitives cryptographiques |
| net | Swarm libp2p, transport QUIC, découverte locale et connexion aux pairs |
| store | Données SQLite, migrations, fichiers et sauvegardes |
| client | Commandes Tauri, session utilisateur et bridge événementiel |
| frontend | Interface React, état utilisateur et flux WebRTC |
| server | API authentifiée, stockage, relais P2P et jetons d'accès au SFU optionnel |
| media | Composants média bas niveau ; leur présence ne signifie pas qu'un SFU est déployé |

## Moteur et processus

Le frontend Linux embarqué est servi depuis `http://tauri.localhost` (ou HTTPS si configuré). CEF exécute ses processus de rendu et de service à partir du même exécutable ; l'entrée CEF les traite avant la création de la fenêtre principale. Le runtime exige le sandbox Chromium, limite la navigation aux origines prévues et laisse Chromium demander les permissions de capture.

La page React crée les pistes WebRTC. En P2P, les commandes et événements Rust transportent la signalisation. En SFU, la commande Rust fournit l'accès à la salle et la clé locale au SDK LiveKit ; le Worker applique le chiffrement des médias.

Le binaire QA debug active CDP et des périphériques synthétiques dans des profils isolés. Le pilote se connecte à l'application réellement construite. L'observation des objets `MediaStreamTrack` permet de suivre les pistes conservées, mais ne mesure pas directement l'activité d'un périphérique physique. Le binaire de distribution n'active pas cette option.

## Frontières

Le réseau est non fiable. Les signatures et le chiffrement de canal sont vérifiés avant la consommation des événements. Une signature de transport libp2p ne remplace pas la signature de l'auteur du message.

Le stockage local n'offre pas de protection contre un appareil compromis. Le serveur de blobs reçoit du contenu déjà chiffré et doit aussi vérifier qui possède le droit de le lire, modifier ou supprimer.

Le réseau de signalisation et les flux WebRTC sont deux chemins distincts. Se connecter au serveur HTTP ou au serveur TURN ne garantit pas la découverte d'un pair libp2p.

En mode SFU, LiveKit transporte les flux média chiffrés par le SDK client. L'API Liberté délivre les jetons d'accès de courte durée ; les clés de chiffrement proviennent du canal et ne sont pas confiées au SFU. La connectivité du SFU et la distribution des clés sont des frontières distinctes.

## Exploitation

L'API HTTP utilise TCP 8080 et le relais QUIC UDP 4001. TURN, s'il est ajouté, utilise ses propres ports et identifiants. Le workflow prototype prévoit les contrôles de compilation, serveur, échanges natifs et médias décrits dans [Construire et publier](Releasing.md). Leurs rapports doivent être examinés pour le commit concerné ; une connexion réelle entre deux appareils reste un test d'acceptation nécessaire.

