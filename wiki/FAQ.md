# Questions fréquentes

## Est-ce une version prête à remplacer Discord ?

La branche 2.4.0 vise à corriger les frontières réseau, stockage et distribution. L'état de la CI, les essais entre appareils et les limitations documentées doivent être examinés avant un déploiement. Aucun nombre d'utilisateurs simultanés n'est garanti.

## Faut-il un serveur ?

Les échanges directs sont possibles lorsque les pairs peuvent se découvrir et se joindre. Certains réseaux nécessitent un relais P2P et les appels peuvent nécessiter TURN. Un serveur HTTP de sauvegarde ne résout pas ces deux problèmes.

## Les invitations sont-elles confidentielles ?

Elles contiennent un secret d'accès. Leur signature protège l'intégrité et l'identité du signataire, pas leur confidentialité. Les transmettre en privé, respecter leur expiration et ne pas les publier.

## Puis-je réutiliser les invitations 2.3 ?

Non pour la nouvelle version de protocole. Mettre à jour les participants et générer de nouvelles invitations. Voir [Migration](Migration-2.4.md).

## Les appels passent-ils par un SFU ?

Le client utilise WebRTC entre pairs. La présence de types ou composants SFU dans le dépôt n'est pas une preuve d'un service SFU utilisable.

## Pourquoi les mises à jour automatiques peuvent-elles être absentes ?

Elles nécessitent des bundles signés, la clé publique intégrée et un manifeste complet. Sans configuration de signature, la release produit des installateurs manuels et n'annonce pas de mise à jour automatique.

## Le chiffrement protège-t-il un appareil compromis ?

Non. Les clés et le profil local doivent être protégés par les mécanismes du système d'exploitation. Les IP et d'autres métadonnées restent visibles aux participants et aux services réseau utilisés.

