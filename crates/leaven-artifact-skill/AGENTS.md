## Boundary
This crate owns the Agent Skills artifact family: `SkillBank`, `SkillFolder`, `SkillFile`, `SkillCard`, `SkillRouteRegistry`, validated skill names/paths, `SKILL.md` parsing, metadata, change records, and explicit folder/manifest/body/file/reference surfaces.

It models skills as artifacts and edit surfaces. It does not run agents, materialize workspaces, call provider CLIs, parse agent sessions, or decide optimizer policy.

## Routing
- Put `SKILL.md` frontmatter/body parsing and validation in `src/manifest.rs`.
- Put path/name/file/folder invariants in `src/path.rs`, `src/file.rs`, and `src/folder.rs`.
- Put manifest-only routing catalog projections in `src/card.rs`.
- Put explicit route-pool/key overlays over validated skill banks in
  `src/route.rs`.
- Put bank-level change vocabulary in `src/change.rs` and `src/bank.rs`.
- Put edit-surface projections in `src/surface.rs`; agentic workspace materializers and proposal parsers belong in `leaven-agentic-skill`.
- Put Codex-specific protocol or CLI details in `leaven-agent-codex-*`, not here.

## Local Helper Stack
- Use `SkillName`, `SkillDescription`, `SkillPath`, `SkillBody`, and
  `SkillMetadataValue` constructors before a folder enters a `SkillBank`.
  Invalid text should fail with typed errors at construction/parse time.
- Use `ParsedSkillMd` and `SkillManifest` for `SKILL.md`; preserve body text and
  file permissions when editing only frontmatter.
- Use `SkillCard` or `SkillBank::cards` when callers need a routing catalog
  over validated skill names, descriptions, and generic metadata without
  exposing bodies or file payloads.
- Use `SkillRouteRegistry` when a caller has deliberate route-pool/key
  membership over an existing `SkillBank`, such as D2Skill task-vs-step pools.
  The caller still owns key extraction, embeddings, scoring, utility,
  lifecycle state, and materialization.
- Use `SkillBankChange` for functional artifact mutation. Apply changes in
  order and leave the original bank untouched if a later change invalidates the
  result.
- Use `SkillFolderSurface`, `SkillManifestSurface`, `SkillBodySurface`,
  `SkillFileSurface`, and `SkillReferenceSurface` depending on whether an
  optimizer is editing folders, frontmatter, the always-loaded body, arbitrary
  files, or direct `references/*.md` progressive-disclosure modules.

## Local Bait
- A `SkillBankChange::WriteFile` is artifact mutation, not filesystem execution. Workspace side effects belong behind workspace/agentic adapters.
- `SkillCard` is a derived view over validated manifests. `SkillRouteRegistry`
  can deliberately own route pool/key membership as an overlay over a bank.
  Utility tables, trigger counts, router weights, similarity scores, and
  lifecycle state are still not `SkillBank` or route-registry facts.
- `SkillManifestSurface` preserves the existing body and file permissions while replacing frontmatter. `SkillBodySurface` preserves frontmatter and file permissions while replacing the body. Keep that distinction when adding narrower surfaces.
- `SkillReferenceSurface` is deliberately narrower than `SkillFileSurface`: it
  accepts only direct `references/*.md` modules. Scripts, nested reference
  trees, assets, and non-markdown files stay behind the file surface or a more
  specific future surface.
- YAML metadata is a generic bag after required fields are validated; do not turn provider/runtime-specific frontmatter into core skill artifact law here.
- Rename is a semantic skill operation here: folder name and `SKILL.md` name
  move together. Do not copy generic path-surface rename behavior into this
  artifact family.

## Proof Anchors
- `cargo nextest run -p leaven-artifact-skill --test skill_artifact` proves skill name/path validation, `SKILL.md` parsing, manifest-derived skill cards, route registry pool/key overlays, folder/bank invariants, rollback, rename semantics, content identity including permissions, and all five skill surfaces.
- `cargo test -p leaven --test topology_contract` proves this crate stays an artifact/surface crate and Codex app-server protocol types remain leaf-only.
