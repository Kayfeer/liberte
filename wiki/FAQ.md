# Questions fréquentes

## Est-ce une version prête à remplacer Discord ?

Cette branche est le [prototype Tauri 3 / CEF](CEF-Prototype.md), pas une version stable publiée. Elle prolonge les corrections réseau, stockage et distribution du protocole 2.4. L'état de la CI, les essais entre appareils et les limitations documentées doivent être examinés avant un déploiement. Aucun nombre d'utilisateurs simultanés n'est garanti.

## Faut-il un serveur ?

Les échanges directs sont possibles lorsque les pairs peuvent se découvrir et se joindre. Certains réseaux nécessitent un relais P2P et les appels peuvent nécessiter TURN. Un serveur HTTP de sauvegarde ne résout pas ces deux problèmes.

## Les invitations sont-elles confidentielles ?

Elles contiennent un secret d'accès. Leur signature protège l'intégrité et l'identité du signataire, pas leur confidentialité. Les transmettre en privé, respecter leur expiration et ne pas les publier.

## Puis-je réutiliser les invitations 2.3 ?

Non pour la nouvelle version de protocole. Mettre à jour les participants et générer de nouvelles invitations. Voir [Migration](Migration-2.4.md).

## Les appels passent-ils par un SFU ?

Le mode par défaut utilise WebRTC entre pairs. Le mode SFU utilise une instance LiveKit configurée par l'opérateur et le chiffrement E2EE du SDK client. Le prototype Linux utilise CEF pour ces parcours ; leur présence dans le code ne vaut pas validation native. Voir [LiveKit](LiveKit.md) pour les prérequis et les essais ; aucun service cloud n'est préconfiguré.

## Pourquoi les mises à jour automatiques peuvent-elles être absentes ?

Elles nécessitent des bundles signés, la clé publique intégrée et un manifeste complet. Sans configuration de signature, la release produit des installateurs manuels et n'annonce pas de mise à jour automatique.

## Le chiffrement protège-t-il un appareil compromis ?

Non. Les clés et le profil local doivent être protégés par les mécanismes du système d'exploitation. Les IP et d'autres métadonnées restent visibles aux participants et aux services réseau utilisés.

