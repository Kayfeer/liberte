# Migration vers 2.4

Cette migration concerne le protocole, l'authentification du relais et la distribution. Ne pas mélanger les clients 2.3 et 2.4 en supposant une compatibilité réseau.

## Avant de mettre à jour

- Exporter une sauvegarde de chaque profil et vérifier qu'elle peut être relue sur un profil de test.
- Conserver une copie du répertoire local et des données du serveur avant toute modification.
- Identifier les autres participants et planifier leur mise à jour.
- Tester la nouvelle version avec des copies des données, puis vérifier messages, fichiers et appels.

## Réseau et invitations

Le nouveau format `liberte-envelope-v2` chiffre l'enveloppe et lie par signature l'auteur, le canal, l'identifiant, la date et le contenu. Les anciens messages réseau en clair sont refusés. Les messages récents sont bornés dans le temps et les identifiants déjà vus sont filtrés.

Les invitations passent en version 2 avec des adresses de pairs signées. Générer de nouvelles invitations après mise à jour. Elles expirent au bout de cinq minutes et restent des secrets au porteur. Leur expiration n'expulse pas un membre qui a déjà obtenu la clé du canal.

Le format interne des messages déjà enregistrés reste destiné à être relu localement ; une réception réseau ancienne n'est pas pour autant autorisée. Vérifier l'historique après migration et conserver la sauvegarde tant que la restauration n'a pas été validée.

## API de stockage

Les écritures et lectures de blobs et sauvegardes doivent utiliser des requêtes signées. Les anciens scripts qui ne fournissent qu'une clé publique ou un identifiant ne sont plus compatibles. Mettre à jour le client et le serveur ensemble. Les fichiers hérités sans propriétaire authentifiable ne doivent pas être réattribués automatiquement à un appelant non vérifié.

## Releases

La version 2.4 utilise un tag complet, par exemple `v2.4.0`. Ne pas déplacer un tag publié. Les clés de signature des mises à jour sont distinctes des clés de conversation et doivent être conservées durablement.

## Retour arrière

Ne pas ouvrir automatiquement une base modifiée avec un ancien client. Restaurer une copie antérieure dans un profil séparé et vérifier le résultat. Un retour à 2.3 réintroduit également les limites de sécurité de son protocole.
