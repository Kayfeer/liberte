# Frontend dependency security

Reviewed on 2026-10-22. The first integration build reported 18 npm findings
(2 critical, 12 high); that count alone does not identify the affected dependency
paths. CI preserves the full `npm audit --json` report so each remaining finding
can be checked against the generated lockfile.

## Targeted updates

| Package | Previous manifest / repository lock | Required floor | Reason |
| --- | --- | --- | --- |
| Vite | `^6` / 6.4.1 | 6.4.5 | File access fixes, including the 6.4.4 query-handling regression fixed on October 21. |
| Vitest | `^3.2.0` / newly introduced | 4.1.12 | Fixes the affected `@vitest/mocker` dependency without overriding its internal API across major versions. |
| PostCSS | `^8` / 8.5.6 | 8.5.23 | Fixes source-map file disclosure, including the incomplete earlier correction. |
| Rollup | transitive / 4.57.1 | 4.59.0 | Fixes output path traversal and arbitrary file writes. |

The manifest keeps Vite 6 and PostCSS 8. Its overrides reuse the direct Vite and
PostCSS specifications and constrain Rollup to the patched 4.x line, preventing
an older nested copy from remaining in the lockfile. LiveKit stays at 2.23.0.
No `npm audit fix --force` or guessed lockfile integrity values are used.

These are build/test dependencies; exploit conditions vary. For example, several
Vite/Vitest issues require an exposed development or test server. Updating them
still protects developer machines and CI. The production desktop application
does not need to run these servers.

## Primary references

- [Vite 6.4.4 regression, fixed in 6.4.5](https://github.com/vitejs/vite/security/advisories/GHSA-8g5g-mxqh-grgj)
- [Vite file allowlist fix](https://github.com/vitejs/vite/security/advisories/GHSA-rq7h-c2jc-7f22)
- [Vitest mock registration injection, fixed in 4.1.12](https://github.com/vitest-dev/vitest/security/advisories/GHSA-qxg6-89mg-rqv8)
- [Vitest Windows traversal fix](https://github.com/vitest-dev/vitest/security/advisories/GHSA-xp26-3454-mjgp)
- [Vitest 4 migration requirements: Node 20+ and Vite 6+](https://v4.vitest.dev/guide/migration.html)
- [PostCSS source-map correction, fixed in 8.5.23](https://github.com/advisories/GHSA-fxqj-rqcc-2cmp)
- [Rollup path traversal, fixed in 4.59.0](https://github.com/advisories/GHSA-mw96-cpmx-2vgc)

## Verification

Regenerate and review the candidate `frontend/package-lock.json` through the CI
bootstrap step, then commit that exact lockfile. Verify the installed tree with
`npm ci`, `npm audit --json`, `npm test`, `npm run typecheck`, `npm run build`, and
the browser media tests. A manifest change alone is not evidence that all audit
findings are resolved. The final audit and test results must come from the same
commit and lockfile used for release.
