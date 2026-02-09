# FAQ

## Général

### Qu'est-ce que Liberté ?
Liberté est une application de messagerie **chiffrée de bout en bout**, **décentralisée** et **pair-à-pair**. Pas de compte, pas de serveur central, pas de collecte de données.

### Dois-je créer un compte ?
Non. Votre identité est une **clé cryptographique Ed25519** générée localement sur votre appareil. Pas d'email, pas de numéro de téléphone.

### Mes messages sont-ils chiffrés ?
Oui, tous les messages sont chiffrés avec **XChaCha20-Poly1305** avant d'être stockés et transmis. Seuls les membres du canal possédant la clé peuvent les lire.

## Technique

### Comment fonctionne la connexion entre pairs ?
Liberté utilise **libp2p** avec le transport **QUIC**. La découverte se fait via mDNS (réseau local) et DHT (Kademlia). Un serveur relay optionnel permet de traverser les NAT.

### Que se passe-t-il si je perds mon appareil ?
Votre clé privée est stockée uniquement sur votre appareil. Si vous perdez l'appareil sans avoir exporté votre profil, votre identité est perdue. **Utilisez la fonction de sauvegarde/export de profil régulièrement.**

### Puis-je utiliser Liberté sur plusieurs appareils ?
Pas encore nativement, mais vous pouvez **exporter votre profil** depuis un appareil et **l'importer** sur un autre.

### Comment inviter quelqu'un dans un canal ?
Générez un **code d'invitation** depuis le canal. Partagez-le au destinataire qui pourra le coller dans "Rejoindre un canal".

## Sécurité

### Liberté est-il audité ?
Le code est open source et audité en interne à chaque release. Des audits externes sont prévus pour les versions futures.

### Mes métadonnées sont-elles protégées ?
Les adresses IP sont visibles par les pairs directs (dans le modèle P2P). Un serveur relay peut atténuer ce problème en masquant les IP.

### Le serveur relay peut-il lire mes messages ?
Non. Les messages sont chiffrés de bout en bout. Le serveur relay ne voit que du trafic chiffré.
