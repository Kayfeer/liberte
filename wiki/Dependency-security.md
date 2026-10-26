# Dependency security

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
| Tailwind CSS | `^3` / 3.4.19 | 4.3.3 | Removes the unpatched `braces` chain through `chokidar`, `fast-glob`, and `micromatch`, plus the old selector parser. |
| Babel core | transitive / 7.29.0 | 7.29.6 | Fixes source-map file reads when compiling untrusted source. |
| Browserslist | transitive / 4.28.6 | 4.28.7 | Fixes query-cache memory exhaustion and unsafe statistics parsing. |
| baseline-browser-mapping | transitive / below 2.11.0 | 2.11.0 | Invalid input throws instead of terminating the process. |
| Picomatch | transitive / 2.3.1 and 4.0.3 | 2.3.2 and 4.0.4 | Fixes crafted-pattern denial of service while keeping each consumer's major version. |

The manifest keeps Vite 6 and PostCSS 8. Its overrides reuse the direct Vite and
PostCSS specifications and constrain Rollup to the patched 4.x line, preventing
an older nested copy from remaining in the lockfile. LiveKit stays at 2.23.0.
No `npm audit fix --force` or guessed lockfile integrity values are used.

The third integration run (`39697891902`, commit `13188d2`) confirmed 10 remaining
findings: 7 high, 2 moderate, 1 low, and no critical findings. The package paths
above come from its saved `npm audit --json` report. `braces` has no patched
release, so a version override cannot fix it. Tailwind 4 removes that dependency
chain; its official PostCSS plugin also handles imports and prefixing, so the
old Autoprefixer plugin is removed. The existing color/font configuration is
loaded explicitly using `@config`, and renamed utility classes retain their
previous appearance and keyboard-focus behavior.

Tailwind 4 requires a modern webview (Chrome 111+, Safari 16.4+, Firefox 128+).
Desktop platform requirements and installers must match that minimum. Validate
the built UI on its supported webviews before a public release.

The override specifications are written explicitly and match their direct
dependency specifications exactly. The integration runner's npm 10.9.9 failed to
resolve the `$postcss` reference while bootstrapping the older lockfile, so the
manifest does not rely on reference-style overrides.

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
- [Unpatched braces recursion](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm)
- [Tailwind 4 migration and webview requirements](https://tailwindcss.com/docs/upgrade-guide)
- [Babel source-map file read](https://github.com/advisories/GHSA-4x5r-pxfx-6jf8)
- [Browserslist query cache](https://github.com/advisories/GHSA-c83g-rgw3-j3cx)
- [Browserslist statistics parsing](https://github.com/advisories/GHSA-73wf-gq98-2v4g)
- [Baseline browser mapping process exit](https://github.com/advisories/GHSA-w5vr-8v7q-w6rv)
- [Picomatch pattern denial of service](https://github.com/advisories/GHSA-c2c7-rcm5-vvqj)
- [Selector parser quadratic complexity](https://github.com/advisories/GHSA-rj75-hqrm-r3gf)

## Verification

Regenerate and review the candidate `frontend/package-lock.json` through the CI
bootstrap step, then commit that exact lockfile. Verify the installed tree with
`npm ci`, `npm audit --json`, `npm test`, `npm run typecheck`, `npm run build`, and
the browser media tests. A manifest change alone is not evidence that all audit
findings are resolved. The final audit and test results must come from the same
commit and lockfile used for release.
