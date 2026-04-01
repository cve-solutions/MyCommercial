# CLAUDE.md - Directives pour Claude Code

## Versioning

- Avant chaque `git push`, incrémenter la version de `0.0.1` dans `Cargo.toml` (champ `version`).
- L'exécutable dans `Dist/` doit toujours correspondre à la version courante de `Cargo.toml`.
- Rebuild l'exécutable release (`cargo build --release`) et le copier dans `Dist/` avec le suffixe de version (ex: `mycommercial-0.2.7`).
- Conserver aussi une copie sans suffixe (`mycommercial`) pour un accès direct à la dernière version.
- Ne garder que les 3 dernières versions suffixées dans `Dist/`. Supprimer les plus anciennes.
