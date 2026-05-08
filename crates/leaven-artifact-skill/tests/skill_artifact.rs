use std::collections::BTreeMap;
use std::str::FromStr;

use leaven_artifact_skill::{
    ParsedSkillMd, SkillBank, SkillBankChange, SkillBankError, SkillFile, SkillFileEdit,
    SkillFilePartId, SkillFileSurface, SkillFolder, SkillManifestEdit, SkillManifestPartId,
    SkillManifestSurface, SkillMetadataValue, SkillName, SkillPath,
};
use leaven_core::{Artifact, ContentAddressed};
use leaven_surface::EditSurface;

fn skill_name(value: &str) -> SkillName {
    SkillName::from_str(value).unwrap()
}

fn skill_path(value: &str) -> SkillPath {
    SkillPath::from_str(value).unwrap()
}

fn skill_md(name: &str, description: &str, body: &str) -> SkillFile {
    SkillFile::text(format!(
        "---\nname: {name}\ndescription: {description}\nlicense: MIT\nmetadata:\n  owner: tests\n---\n{body}\n"
    ))
}

fn folder(name: &str) -> SkillFolder {
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        skill_md(
            name,
            "Use when testing skill folders.",
            "Do the testing work.",
        ),
    );
    entries.insert(
        skill_path("scripts/run.py"),
        SkillFile::text("print('hello')\n"),
    );
    SkillFolder::from_entries(skill_name(name), entries).unwrap()
}

fn bank() -> SkillBank {
    SkillBank::from_folders([folder("test-skill")]).unwrap()
}

#[test]
fn parses_skill_md_required_fields_body_and_metadata_bag() {
    let parsed = ParsedSkillMd::parse(
        b"---\nname: data-cleanup\ndescription: Clean CSV files. Use for tabular cleanup.\ncompatibility: requires python\nmetadata:\n  owner: data\n---\nSteps go here.\n",
    )
    .unwrap();

    assert_eq!(parsed.manifest.name.as_str(), "data-cleanup");
    assert_eq!(
        parsed.manifest.description.as_str(),
        "Clean CSV files. Use for tabular cleanup."
    );
    assert_eq!(
        parsed.manifest.metadata.get("compatibility"),
        Some(&SkillMetadataValue::String("requires python".to_owned()))
    );
    assert!(matches!(
        parsed.manifest.metadata.get("metadata"),
        Some(SkillMetadataValue::Mapping(map)) if map.contains_key("owner")
    ));
    assert_eq!(parsed.body.as_str(), "Steps go here.\n");
}

#[test]
fn rejects_invalid_skill_md_before_a_folder_enters_a_bank() {
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text("---\nname: Bad\ndescription: ok\n---\nBody.\n"),
    );

    let err = SkillFolder::from_entries(skill_name("bad"), entries).unwrap_err();
    assert!(matches!(
        err,
        SkillBankError::InvalidSkillMd {
            skill,
            source: leaven_artifact_skill::SkillParseError::InvalidName(_)
        } if skill == "bad"
    ));
}

#[test]
fn applies_changes_functionally_and_keeps_original_on_failure() {
    let bank = bank();
    let invalid = SkillBankChange::WriteFile {
        skill: skill_name("test-skill"),
        path: SkillPath::skill_md(),
        file: SkillFile::text("---\nname: test-skill\ndescription: ok\n---\n"),
    };

    assert!(bank.apply_change(&invalid).is_err());
    assert_eq!(
        bank.get(&skill_name("test-skill")).unwrap().entries().len(),
        2
    );
}

#[test]
fn rename_skill_updates_folder_name_and_skill_md_name_together() {
    let bank = bank();
    let renamed = bank
        .apply_change(&SkillBankChange::RenameSkill {
            from: skill_name("test-skill"),
            to: skill_name("renamed-skill"),
        })
        .unwrap();

    assert!(renamed.get(&skill_name("test-skill")).is_none());
    let folder = renamed.get(&skill_name("renamed-skill")).unwrap();
    let parsed =
        ParsedSkillMd::parse(folder.file(&SkillPath::skill_md()).unwrap().bytes()).unwrap();
    assert_eq!(parsed.manifest.name.as_str(), "renamed-skill");
    assert_eq!(parsed.body.as_str(), "Do the testing work.\n");
}

#[test]
fn content_id_is_stable_for_content_and_changes_when_permissions_change() {
    let original = bank();
    let same = bank();
    let changed = original
        .apply_change(&SkillBankChange::SetExecutable {
            skill: skill_name("test-skill"),
            path: skill_path("scripts/run.py"),
            executable: true,
        })
        .unwrap();

    assert_eq!(original.content_id(), same.content_id());
    assert_ne!(original.content_id(), changed.content_id());
}

#[test]
fn manifest_and_file_surfaces_translate_edits_to_artifact_changes() {
    let bank = bank();
    let manifest_surface = SkillManifestSurface;
    let manifest_part = SkillManifestPartId {
        skill: skill_name("test-skill"),
    };
    let manifest_change = manifest_surface
        .change_part(
            &bank,
            manifest_part,
            SkillManifestEdit::Replace {
                description: leaven_artifact_skill::SkillDescription::new(
                    "Use when a test needs a changed description.",
                )
                .unwrap(),
                metadata: BTreeMap::new(),
            },
        )
        .unwrap();
    let changed = bank.apply_change(&manifest_change).unwrap();
    assert_eq!(
        changed
            .get(&skill_name("test-skill"))
            .unwrap()
            .manifest()
            .description
            .as_str(),
        "Use when a test needs a changed description."
    );

    let file_surface = SkillFileSurface;
    let file_change = file_surface
        .change_part(
            &changed,
            SkillFilePartId {
                skill: skill_name("test-skill"),
                path: skill_path("scripts/run.py"),
            },
            SkillFileEdit::Replace(SkillFile::text("print('changed')\n")),
        )
        .unwrap();
    let changed = changed.apply_change(&file_change).unwrap();
    assert_eq!(
        changed
            .get(&skill_name("test-skill"))
            .unwrap()
            .file(&skill_path("scripts/run.py"))
            .unwrap()
            .bytes(),
        b"print('changed')\n"
    );
}
