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
   (BUZZ_RELAY_URL, BUZZ_PRIVATE_KEY, BUZZ_AUTH_TAG) + dual emission in
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
| ECR repo `…amazonaws.com/griddle` | TF stack owns repo creation; switching now breaks staging deploy | squareup/block-coder-tf-stacks creates `/crew`, then flip deploy refs |
| GitHub org/repo `block/buzz`, `squareup/*` | external identity; redirects break on rename | post-merge admin action |
| GHCR image names `block/buzz*` | pushes create new repos but existing pullers/dependabot lag | after relay images published under crew |
| `~/.buzz*` state dirs | mid-session resume breakage; needs marker-file migration per consumer | dedicated follow-up PR |
| App store bundle ids already shipped (iOS prod profiles `buzz-ios-*`) | persisted push-profile data; see tier 2 dual-read | retained alongside new crew ids |

## Invariants honored

- No edits to historical `migrations/*.sql`; sentinel renames ship as new
  migrations with dual-read/write in code.
- Alias windows for env contracts; child-env builders emit both names.
- Paired surfaces (bin names ↔ Dockerfile/charts; deep-link scheme ↔ app
  registration; npm names ↔ pnpm filters/Dockerfile RUN filters) flip inside
  one commit.
- scan counts == applied counts == diff math per tier.
