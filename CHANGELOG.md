# Changelog

## 2.4.0, en cours de validation

Cette version modifie le protocole réseau et l'authentification de l'API. Voir [Migration-2.4](wiki/Migration-2.4.md). Elle n'est pas réputée publiée par la présence de ce fichier.

- Enveloppes chiffrées et signées, validation de l'auteur, du canal, de la date et des rejeux.
- Invitations temporaires version 2 avec adresses de pairs authentifiées.
- Authentification et propriété des opérations de stockage serveur, limites de ressources et état persistant.
- Sauvegardes exportables protégées par phrase secrète et sauvegardes automatiques liées à l'identité.
- Parcours client et état de transmission rendus cohérents avec les résultats des commandes.
- Appels WebRTC entre pairs et configuration ICE explicite ; pas de promesse de SFU de production.
- Tests frontend, Rust, frontières HTTP et outils de release ; essais WebRTC locaux.
- Toolchain Rust figée, lockfiles préservés et releases conditionnées aux contrôles.
- Images Docker par architecture, chargées et démarrées avant distribution.
- Signature updater explicitement activée ou désactivée, sans manifeste de mise à jour trompeur.
- Documentation révisée pour distinguer implémentation, limites et validation.

Les résultats exacts à retenir sont ceux de la CI du commit concerné et des essais d'acceptation. Aucun test non exécuté ne doit être décrit comme réussi.

## 2.3.0 et antérieures

Versions historiques. Leur code et leurs artefacts sont conservés dans les tags GitHub ; ils ne bénéficient pas automatiquement des corrections de cette branche.
