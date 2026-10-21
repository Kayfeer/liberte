# Politique de sécurité

## Versions

La branche 2.4.0 contient les corrections en cours de validation. Les versions historiques 2.3 et antérieures utilisent l'ancien protocole et ne doivent pas être considérées comme équivalentes. Une compilation réussie ne valide pas à elle seule les propriétés de sécurité.

## Signaler un problème

Envoyer les détails à **genesistrakd@gmail.com**, sans publier de secret ni de preuve contenant des données privées dans une issue publique. Indiquer la version, les préconditions, les étapes reproductibles et l'impact. Aucun délai de correction garanti n'est annoncé dans ce dépôt.

## Garanties recherchées et contrôles

- Chaque enveloppe réseau est chiffrée avec une clé de canal et authentifiée par la signature Ed25519 de son auteur.
- L'identifiant, le canal et la date font partie des données authentifiées. Les messages hors fenêtre temporelle et les rejeux sont refusés.
- Les invitations sont signées et temporaires. Elles contiennent la clé d'accès du canal : ce sont des secrets au porteur, pas des liens publics.
- Les opérations de stockage HTTP exigent la preuve de possession d'une clé, avec protection contre le rejeu. Le chiffrement du contenu reste effectué côté client.
- L'administration utilise un jeton distinct. Une valeur vide désactive l'API d'administration.
- Une mise à jour automatique n'est proposée que si les fichiers de distribution et le manifeste sont signés et vérifiés avec la clé publique configurée.

Les détails exécutables sont dans le code et les tests. Les fonctions déclarées mais non branchées ne constituent pas une garantie.

## Limites du modèle

- Une personne ayant accès au profil local de l'utilisateur peut accéder aux secrets locaux. Le stockage SQLite n'est pas un coffre matériel ni un chiffrement intégral du disque.
- Les clés de canal sont durables. Pas de double ratchet, de confidentialité persistante des anciens messages après compromission de la clé, ni de révocation rétroactive des invitations déjà utilisées.
- Les membres d'un canal peuvent copier le contenu et le retransmettre. La suppression locale ne retire pas les copies distantes.
- Les pairs directs connaissent leurs adresses IP. Le chiffrement ne rend pas les participants anonymes.
- La protection contre le rejeu est bornée et locale au processus ; elle ne constitue pas un journal distribué persistant.
- Les connexions réseau, DNS, WebRTC, sauvegardes et téléchargements de mises à jour utilisent leurs propres chemins. Ne pas supposer que tout le trafic réseau est routé par un résolveur unique.
- WebRTC et TURN ne prouvent pas à eux seuls un chiffrement applicatif indépendant de tout serveur SFU. Le client utilise un maillage de pairs, pas un service SFU de production.
- Les limites de débit, de stockage et de nombre de pairs doivent être adaptées à l'hébergement. Un service public requiert supervision, mises à jour, sauvegardes et gestion des abus.

## Déploiement

Conserver l'API HTTP en écoute locale derrière un proxy HTTPS. Ne pas exposer le jeton administrateur à un client non fiable. Utiliser un compte TURN dédié avec un mot de passe aléatoire, des quotas et une rotation. Ne jamais copier la clé privée de signature des releases dans une configuration client.

La CI doit être verte sur le commit effectivement distribué, avec `Cargo.lock` versionné. Les tests automatisés ne remplacent ni les essais sur plusieurs machines, ni la revue indépendante des frontières cryptographiques.

