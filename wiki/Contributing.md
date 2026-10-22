# Contribuer

Travailler sur une branche et ouvrir une pull request. Les corrections de sécurité doivent inclure un scénario négatif qui échoue avant la correction.

## Contrôles

```bash
python -m unittest discover -s scripts -p ci_test.py -v
python scripts/ci.py versions
npm --prefix frontend ci
npm --prefix frontend test
npm --prefix frontend run typecheck
npm --prefix frontend run build
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Exécuter les smoke tests serveur et Docker pour les modifications concernées. Ne pas masquer un échec par `continue-on-error`, une assertion supprimée, un test ignoré ou une désactivation de l'authentification.

## Dépendances et versions

Versionner `Cargo.lock` et `frontend/package-lock.json`. Après modification des manifestes, régénérer les lockfiles avec les outils correspondants et examiner le diff. Un lockfile modifié à la main n'est pas une résolution validée.

Les versions Cargo, Tauri et frontend doivent être identiques. Un changement incompatible de protocole nécessite une documentation de migration et un nouveau tag, jamais le déplacement d'un tag existant.

Les workflows de release ne doivent pas recevoir de secrets depuis un code de pull request non fiable. Les builds de PR utilisent uniquement des permissions de lecture.

