# CLAUDE.md - Directives pour Claude Code

## Versioning

- Avant chaque `git push`, incrémenter la version de `0.0.1` dans `Cargo.toml` (champ `version`).
- L'exécutable dans `Dist/` doit toujours correspondre à la version courante de `Cargo.toml`.
- Rebuild l'exécutable release (`cargo build --release`) et le copier dans `Dist/` avant de push.
