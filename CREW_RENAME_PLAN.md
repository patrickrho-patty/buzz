# Buzz/Griddle → Crew — Tiered Rename Plan

Whole-product rename executed as gated tiers. One commit per tier; each tier's
gates compare against baselines captured before any change:

| Baseline | Result |
|---|---|
| `cargo check --workspace --all-targets` | exit 0 (`/tmp/baseline-cargo-check.txt`) |
| `desktop`: `tsc --noEmit` | exit 0 |
| `web`: `tsc --noEmit` | exit 0 |
| `flutter analyze` | exit 0 |
| targeted cargo tests (`-p buzz-core -p buzz-workflow -p buzz-pubsub`) | ok |

## Census summary

~25.7k occurrences: `buzz`×16,754 · `Buzz`×3,264 · `BUZZ`×5,133 ·
`griddle`×641 · `Griddle`×58 · `GRIDDLE`×9.
Files: rs(567) ts(358) dart(237) tsx(186) mjs(163) md(91) sh(64) toml(46)
yaml(45) sql(18+3) …

**Collision check vs `crew`: PASS** — only dictionary wordlist entries,
emoji JSON and two test-fixture display strings ("Product crew",
"Contrast Crew"); no identifiers/env/packages collide.

## Tiers

1. **Rust crates** — `git mv` 30 crates/buzz-\* → crew-\* (+ tauri crate
   buzz-terminal), package names, workspace members, `buzz_x::` imports,
   bin names; paired same-commit: Dockerfiles' binary paths, compose/chart
   refs to renamed binaries, scripts invoking bins. Gate: cargo check parity.
2. **GUC/persisted sentinels** (DB contract) — new migrations only; never edit
   historical files. GUC namespace `buzz.*`, mesh d/k-tags `buzz-mesh-*`,
   app_profile strings, hashtext salt literals: dual-write/dual-read flip.
3. **Env vars** — BUZZ_\*/GRIDDLE_\* → CREW_\* across Rust/TS/Dart/sh/yaml;
   alias window at boot for externally documented contracts
   (CREW_RELAY_URL, CREW_PRIVATE_KEY, CREW_AUTH_TAG) + dual emission in
   child-env builders; `.env.example` both rows during window.
4. **Desktop/web TS** — identifiers then free strings incl. tauri.conf.json
   identifier `xyz.patty.griddle.app` → `…crew…`; deep-link scheme
   `buzz://` → `crew://` flipped everywhere same commit (apps register it).
5. **Mobile** — pubspec name `buzz`→crew + package imports;
   applicationId `xyz.block.buzz.mobile` → crew.
6. **npm/config/scripts/CI/assets** — package.json names, pnpm filters,
   .github workflows, Helm chart names/values, deploy compose, logos/text.
7. **Comments/docs last** — describe final state; AGENTS/README/VISION etc.

## Deferral table (cross-repo / data migrations)

| Surface | Why deferred | Follow-up |
|---|---|---|
| ECR repo `…amazonaws.com/griddle` | TF stack owns repo creation; switching now breaks staging deploy | squareup/block-coder-tf-stacks creates `/crew`, then flip deploy refs (`ecr-build.yml`, eks values) |
| GitHub org/repo `block/buzz`, `squareup/*` | external identity; redirects break on rename | post-merge admin action |
| GHCR image names `block/buzz*` | pushes create new repos but existing pullers/dependabot lag; merge ordering vs first publish race | after relay images published under crew |
| `~/.buzz*` state dirs | mid-session resume breakage; needs marker-file migration per consumer | dedicated follow-up PR |
| Desktop localStorage keys `buzz-…` (81 distinct keys) | opaque user-data keys (drafts, read-state, mutes); renaming silently drops user data — needs key-migration table | dedicated follow-up PR |
| APNS app profiles `buzz-ios-*` / Apple `TEAMID.xyz.buzz` / `push.buzz.xyz` domain | tied to shipped app bundles & prod DNS | alongside store re-release |

## Intentional legacy allowances (in-tree)

- `crew-core::env_alias` (+ local twins in git-sign-nostr/git-credential-nostr,
  desktop build.rs): reads prefer `CREW_*`, fall back to documented
  `BUZZ_RELAY_URL`/`BUZZ_PRIVATE_KEY`/`BUZZ_AUTH_TAG`.
- Deep-link parsers (desktop `messageLink.ts`, entity/composer links, CLI
  `links.rs`, iOS URL schemes list) accept legacy `buzz://`; emitters are
  crew-only. OS scheme registration keeps `buzz`+`griddle` during window.
- Project NIP-MP tags: emit `crew-channel`/`crew-visibility`; validators count
  cardinality across both spellings (relay ingest, TS `projectModels.ts`,
  conformance fixture `…_across_legacy_spelling`).
- Mesh status: emit `crew-mesh-*`, readers match both forever (persisted events).
- `xyz.patty.griddle.app` retained in `legacy_storage.rs` only as the legacy
  identifier prefix for migrating pre-rename installs.
- Per-shot hashtext lock salts renamed atomically with migration 0033
  (single-writer deploy model; see migration header for the argument).

## Status @ end of session

Gates green: cargo check/fmt/clippy(-p crew-desktop) · unit suite 1451/0 ·
flutter analyze+tests 1575/0 · desktop+web tsc · vite builds · size ratchet.
Known pre-existing reds carried from the SSO checkpoint (proven at HEAD~):
7 biome `useExhaustiveDependencies` errors, oidc.rs missing-docs clippy set,
1 useRetainedProjectGitViews stub-hook failure — not rename regressions.

**Open (needs a focused session): desktop node:test suite ~86 fixture/key
parity failures.** Root cause is asymmetric storage-key handling: prod still
writes legacy `buzz-…` keys (deliberate deferral) while a subset of test
fixtures and mock-bridge payloads were swept to crew spellings, plus provider
copy mismatches. Fix = decide key policy (alias-read or wholesale flip with
migration) and pair every fixture; repro: `cd desktop && pnpm test`.

## Invariants honored

- No edits to historical `migrations/*.sql`; sentinel renames ship as new
  migrations with dual-read/write in code.
- Alias windows for env contracts; child-env builders emit both names.
- Paired surfaces (bin names ↔ Dockerfile/charts; deep-link scheme ↔ app
  registration; npm names ↔ pnpm filters/Dockerfile RUN filters) flip inside
  one commit.
- scan counts == applied counts == diff math per tier.
