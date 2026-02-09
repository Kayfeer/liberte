# Contributing

## Comment contribuer

1. **Fork** le dépôt
2. Créez une branche : `git checkout -b feature/ma-feature`
3. Commitez : `git commit -m "feat: description"`
4. Poussez : `git push origin feature/ma-feature`
5. Ouvrez une **Pull Request**

## Conventions

### Commits
Suivre [Conventional Commits](https://www.conventionalcommits.org/) :

- `feat:` — nouvelle fonctionnalité
- `fix:` — correction de bug
- `docs:` — documentation
- `refactor:` — refactorisation sans changement fonctionnel
- `security:` — correctif de sécurité
- `chore:` — maintenance (deps, CI, etc.)

### Code Rust
- `cargo fmt` avant chaque commit
- `cargo clippy --all-targets` doit passer sans warnings
- Tests : `cargo test`

### Code TypeScript
- `npx tsc --noEmit` doit passer sans erreurs
- Strict mode activé (`noUnusedLocals`, `noUnusedParameters`)

## Structure du projet

Voir [Architecture](Architecture) pour la vue d'ensemble des crates et du frontend.

## Issues

- Utilisez les labels appropriés (`bug`, `enhancement`, `security`)
- Pour les vulnérabilités de sécurité, voir [Security](Security)
