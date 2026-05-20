## Boundary
This crate owns the Agent Skills artifact family: `SkillBank`, `SkillFolder`, `SkillFile`, `SkillCard`, validated skill names/paths, `SKILL.md` parsing, metadata, change records, and explicit folder/manifest/file surfaces.

It models skills as artifacts and edit surfaces. It does not run agents, materialize workspaces, call provider CLIs, parse agent sessions, or decide optimizer policy.

## Routing
- Put `SKILL.md` frontmatter/body parsing and validation in `src/manifest.rs`.
- Put path/name/file/folder invariants in `src/path.rs`, `src/file.rs`, and `src/folder.rs`.
- Put manifest-only routing catalog projections in `src/card.rs`.
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
- Use `SkillBankChange` for functional artifact mutation. Apply changes in
  order and leave the original bank untouched if a later change invalidates the
  result.
- Use `SkillFolderSurface`, `SkillManifestSurface`, and `SkillFileSurface`
  depending on whether an optimizer is editing folders, frontmatter, or files.

## Local Bait
- A `SkillBankChange::WriteFile` is artifact mutation, not filesystem execution. Workspace side effects belong behind workspace/agentic adapters.
- `SkillCard` is a derived view over validated manifests. Utility tables,
  retrieval keys, trigger counts, router weights, and lifecycle state are not
  `SkillBank` facts unless a future registry artifact deliberately owns them.
- `SkillManifestSurface` preserves the existing body and file permissions while replacing frontmatter. Keep that distinction when adding narrower surfaces.
- YAML metadata is a generic bag after required fields are validated; do not turn provider/runtime-specific frontmatter into core skill artifact law here.
- Rename is a semantic skill operation here: folder name and `SKILL.md` name
  move together. Do not copy generic path-surface rename behavior into this
  artifact family.

## Proof Anchors
- `cargo nextest run -p leaven-artifact-skill --test skill_artifact` proves skill name/path validation, `SKILL.md` parsing, manifest-derived skill cards, folder/bank invariants, rollback, rename semantics, content identity including permissions, and all three skill surfaces.
- `cargo test -p leaven --test topology_contract` proves this crate stays an artifact/surface crate and Codex app-server protocol types remain leaf-only.
