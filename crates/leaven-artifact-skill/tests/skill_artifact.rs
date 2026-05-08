use std::collections::BTreeMap;
use std::str::FromStr;

use leaven_artifact_skill::{
    ParsedSkillMd, SkillBank, SkillBankChange, SkillBankError, SkillBody, SkillDescription,
    SkillFile, SkillFileEdit, SkillFilePartId, SkillFileSurface, SkillFolder, SkillFolderEdit,
    SkillFolderSurface, SkillManifestEdit, SkillManifestPartId, SkillManifestSurface,
    SkillMetadataValue, SkillName, SkillNameError, SkillParseError, SkillPath, SkillPathError,
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
fn validates_skill_names_and_paths_with_typed_errors() {
    assert_eq!(SkillName::new("").unwrap_err(), SkillNameError::Empty);
    assert_eq!(
        SkillName::new("a".repeat(65)).unwrap_err(),
        SkillNameError::TooLong
    );
    assert_eq!(
        SkillName::new("-bad").unwrap_err(),
        SkillNameError::StartsWithHyphen
    );
    assert_eq!(
        SkillName::new("bad-").unwrap_err(),
        SkillNameError::EndsWithHyphen
    );
    assert_eq!(
        SkillName::new("bad--name").unwrap_err(),
        SkillNameError::ConsecutiveHyphen
    );
    assert_eq!(
        SkillName::new("Bad").unwrap_err(),
        SkillNameError::InvalidCharacter('B')
    );

    assert_eq!(SkillPath::new("").unwrap_err(), SkillPathError::Empty);
    assert_eq!(
        SkillPath::new("/abs").unwrap_err(),
        SkillPathError::Absolute
    );
    assert_eq!(
        SkillPath::new("scripts//run.py").unwrap_err(),
        SkillPathError::EmptyComponent
    );
    assert_eq!(
        SkillPath::new("./run.py").unwrap_err(),
        SkillPathError::CurrentDirectory
    );
    assert_eq!(
        SkillPath::new("../run.py").unwrap_err(),
        SkillPathError::ParentTraversal
    );
    assert_eq!(
        SkillPath::new("scripts\\run.py").unwrap_err(),
        SkillPathError::Backslash
    );
    assert_eq!(
        SkillPath::new("scripts/\0.py").unwrap_err(),
        SkillPathError::Nul
    );
    assert_eq!(
        SkillPath::try_from("references/guide.md".to_owned())
            .unwrap()
            .as_str(),
        "references/guide.md"
    );
    assert_eq!(
        SkillName::try_from("data-cleanup".to_owned())
            .unwrap()
            .as_str(),
        "data-cleanup"
    );
}

#[test]
fn validated_skill_text_newtypes_preserve_display_and_refuse_invalid_values() {
    let name = SkillName::new("data-cleanup").unwrap();
    assert_eq!(name.to_string(), "data-cleanup");

    let description = SkillDescription::new("Use when cleaning CSV files.").unwrap();
    assert_eq!(description.to_string(), "Use when cleaning CSV files.");
    assert!(SkillDescription::new("x".repeat(1025)).is_err());

    let body = SkillBody::new("Steps go here.").unwrap();
    assert_eq!(body.as_str(), "Steps go here.");
    assert!(SkillBody::new("   ").is_err());
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
fn parses_nested_metadata_and_reports_skill_md_shape_errors() {
    let parsed = ParsedSkillMd::parse(
        b"---\nname: data-cleanup\ndescription: Clean CSV files. Use for cleanup.\nflag: true\ncount: 3\nmissing:\ntags: [csv, cleanup]\nnested:\n  owner: data\n---\nSteps.\n",
    )
    .unwrap();
    assert_eq!(
        parsed.manifest.metadata.get("flag"),
        Some(&SkillMetadataValue::Bool(true))
    );
    assert_eq!(
        parsed.manifest.metadata.get("count"),
        Some(&SkillMetadataValue::Number("3".to_owned()))
    );
    assert_eq!(
        parsed.manifest.metadata.get("missing"),
        Some(&SkillMetadataValue::Null)
    );
    assert!(matches!(
        parsed.manifest.metadata.get("tags"),
        Some(SkillMetadataValue::Sequence(values)) if values.len() == 2
    ));
    assert!(matches!(
        parsed.manifest.metadata.get("nested"),
        Some(SkillMetadataValue::Mapping(values)) if values.contains_key("owner")
    ));

    assert!(matches!(
        ParsedSkillMd::parse(b"not frontmatter"),
        Err(SkillParseError::MissingFrontmatter)
    ));
    assert!(matches!(
        ParsedSkillMd::parse(b"---\nname: data-cleanup\n"),
        Err(SkillParseError::MissingClosingFrontmatter)
    ));
    assert!(matches!(
        ParsedSkillMd::parse(b"---\n[\n---\nBody.\n"),
        Err(SkillParseError::Yaml(_))
    ));
    assert!(matches!(
        ParsedSkillMd::parse(b"---\n- item\n---\nBody.\n"),
        Err(SkillParseError::FrontmatterNotMap)
    ));
    assert!(matches!(
        ParsedSkillMd::parse(b"---\ndescription: ok\n---\nBody.\n"),
        Err(SkillParseError::MissingRequiredField { field: "name" })
    ));
    assert!(matches!(
        ParsedSkillMd::parse(b"---\nname: data-cleanup\ndescription: 3\n---\nBody.\n"),
        Err(SkillParseError::RequiredFieldNotString {
            field: "description"
        })
    ));
    assert!(matches!(
        ParsedSkillMd::parse(b"---\nname: data-cleanup\ndescription: '   '\n---\nBody.\n"),
        Err(SkillParseError::InvalidDescription(_))
    ));
    assert!(matches!(
        ParsedSkillMd::parse(
            b"---\nname: data-cleanup\ndescription: ok\n? [bad]\n: value\n---\nBody.\n"
        ),
        Err(SkillParseError::NonStringMetadataKey)
    ));
    assert!(matches!(
        ParsedSkillMd::parse(b"---\nname: data-cleanup\ndescription: ok\n---\n   \n"),
        Err(SkillParseError::EmptyBody)
    ));
    assert!(matches!(
        ParsedSkillMd::parse(&[0xff, 0xfe]),
        Err(SkillParseError::Utf8(_))
    ));
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
fn folders_require_skill_md_and_matching_frontmatter_name() {
    let missing = SkillFolder::from_entries(skill_name("missing"), BTreeMap::new()).unwrap_err();
    assert!(matches!(
        missing,
        SkillBankError::MissingSkillMd { skill } if skill == "missing"
    ));

    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        skill_md("other-name", "Use when a test needs a mismatch.", "Body."),
    );
    let mismatch = SkillFolder::from_entries(skill_name("folder-name"), entries).unwrap_err();
    assert!(matches!(
        mismatch,
        SkillBankError::NameMismatch {
            folder,
            manifest_name,
        } if folder == "folder-name" && manifest_name == "other-name"
    ));
}

#[test]
fn bank_rejects_duplicate_and_missing_skill_operations() {
    let duplicate =
        SkillBank::from_folders([folder("test-skill"), folder("test-skill")]).unwrap_err();
    assert!(matches!(
        duplicate,
        SkillBankError::DuplicateSkillName { name } if name == "test-skill"
    ));

    let bank = bank();
    assert!(!bank.is_empty());
    assert!(SkillBank::default().is_empty());
    assert!(matches!(
        bank.apply_change(&SkillBankChange::ReplaceSkill {
            name: skill_name("missing-skill"),
            folder: folder("missing-skill"),
        })
        .unwrap_err(),
        SkillBankError::MissingSkill { name } if name == "missing-skill"
    ));
    assert!(matches!(
        bank.apply_change(&SkillBankChange::RemoveSkill {
            name: skill_name("missing-skill"),
        })
        .unwrap_err(),
        SkillBankError::MissingSkill { name } if name == "missing-skill"
    ));
    assert!(matches!(
        bank.apply_change(&SkillBankChange::RenameSkill {
            from: skill_name("missing-skill"),
            to: skill_name("renamed-skill"),
        })
        .unwrap_err(),
        SkillBankError::MissingSkill { name } if name == "missing-skill"
    ));
    let two = bank
        .apply_change(&SkillBankChange::CreateSkill {
            folder: folder("other-skill"),
        })
        .unwrap();
    assert!(matches!(
        two.apply_change(&SkillBankChange::RenameSkill {
            from: skill_name("test-skill"),
            to: skill_name("other-skill"),
        })
        .unwrap_err(),
        SkillBankError::SkillAlreadyExists { name } if name == "other-skill"
    ));
}

#[test]
fn bank_create_replace_and_remove_skill_are_functional_and_typed() {
    let bank = bank();
    let other = folder("other-skill");
    let created = bank
        .apply_change(&SkillBankChange::CreateSkill {
            folder: other.clone(),
        })
        .unwrap();
    assert!(created.get(&skill_name("other-skill")).is_some());
    assert!(matches!(
        created.apply_change(&SkillBankChange::CreateSkill { folder: other }).unwrap_err(),
        SkillBankError::SkillAlreadyExists { name } if name == "other-skill"
    ));

    let replacement = folder("test-skill");
    let replaced = created
        .apply_change(&SkillBankChange::ReplaceSkill {
            name: skill_name("test-skill"),
            folder: replacement,
        })
        .unwrap();
    assert!(matches!(
        replaced
            .apply_change(&SkillBankChange::ReplaceSkill {
                name: skill_name("test-skill"),
                folder: folder("other-skill"),
            })
            .unwrap_err(),
        SkillBankError::NameMismatch { folder, manifest_name }
            if folder == "test-skill" && manifest_name == "other-skill"
    ));

    let removed_skill = replaced
        .apply_change(&SkillBankChange::RemoveSkill {
            name: skill_name("other-skill"),
        })
        .unwrap();
    assert!(removed_skill.get(&skill_name("other-skill")).is_none());
}

#[test]
fn bank_file_changes_are_functional_and_typed() {
    let bank = bank();
    let written = bank
        .apply_change(&SkillBankChange::WriteFile {
            skill: skill_name("test-skill"),
            path: skill_path("references/guide.md"),
            file: SkillFile::text("Guide.\n"),
        })
        .unwrap();
    let renamed = written
        .apply_change(&SkillBankChange::RenameFile {
            skill: skill_name("test-skill"),
            from: skill_path("references/guide.md"),
            to: skill_path("references/new-guide.md"),
        })
        .unwrap();
    assert!(matches!(
        renamed
            .apply_change(&SkillBankChange::RenameFile {
                skill: skill_name("test-skill"),
                from: skill_path("references/new-guide.md"),
                to: skill_path("scripts/run.py"),
            })
            .unwrap_err(),
        SkillBankError::FileAlreadyExists { skill, path }
            if skill == "test-skill" && path == "scripts/run.py"
    ));
    let executable = renamed
        .apply_change(&SkillBankChange::SetExecutable {
            skill: skill_name("test-skill"),
            path: skill_path("references/new-guide.md"),
            executable: true,
        })
        .unwrap();
    assert!(
        executable
            .get(&skill_name("test-skill"))
            .unwrap()
            .file(&skill_path("references/new-guide.md"))
            .unwrap()
            .permissions()
            .executable
    );
    let removed_file = executable
        .apply_change(&SkillBankChange::RemoveFile {
            skill: skill_name("test-skill"),
            path: skill_path("references/new-guide.md"),
        })
        .unwrap();
    assert!(matches!(
        removed_file
            .apply_change(&SkillBankChange::RemoveFile {
                skill: skill_name("test-skill"),
                path: skill_path("missing.md"),
            })
            .unwrap_err(),
        SkillBankError::MissingFile { skill, path }
            if skill == "test-skill" && path == "missing.md"
    ));
    assert!(matches!(
        removed_file
            .apply_change(&SkillBankChange::WriteFile {
                skill: skill_name("missing-skill"),
                path: skill_path("notes.md"),
                file: SkillFile::text("Notes.\n"),
            })
            .unwrap_err(),
        SkillBankError::MissingSkill { name } if name == "missing-skill"
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
fn atomic_changes_apply_in_order_and_roll_back_on_invalid_result() {
    let bank = bank();
    let changed = bank
        .apply_change(&SkillBankChange::Atomic(vec![
            SkillBankChange::WriteFile {
                skill: skill_name("test-skill"),
                path: skill_path("references/guide.md"),
                file: SkillFile::text("Guide.\n"),
            },
            SkillBankChange::SetExecutable {
                skill: skill_name("test-skill"),
                path: skill_path("scripts/run.py"),
                executable: true,
            },
        ]))
        .unwrap();
    assert!(
        changed
            .get(&skill_name("test-skill"))
            .unwrap()
            .file(&skill_path("scripts/run.py"))
            .unwrap()
            .permissions()
            .executable
    );

    let err = bank
        .apply_change(&SkillBankChange::Atomic(vec![
            SkillBankChange::WriteFile {
                skill: skill_name("test-skill"),
                path: SkillPath::skill_md(),
                file: SkillFile::text("---\nname: test-skill\ndescription: ok\n---\n"),
            },
            SkillBankChange::RemoveSkill {
                name: skill_name("test-skill"),
            },
        ]))
        .unwrap_err();
    assert!(matches!(
        err,
        SkillBankError::InvalidSkillMd {
            source: SkillParseError::EmptyBody,
            ..
        }
    ));
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
fn skill_file_into_bytes_consumes_without_changing_content() {
    assert_eq!(SkillFile::text("payload").into_bytes(), b"payload");
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

#[test]
fn folder_surface_exposes_parts_and_translates_all_folder_edits() {
    let bank = bank();
    let surface = SkillFolderSurface;
    let parts = surface.parts(&bank).unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].address, "test-skill");
    assert_ne!(surface.fingerprint(), SkillFileSurface.fingerprint());

    let replace = surface
        .change_part(
            &bank,
            skill_name("test-skill"),
            SkillFolderEdit::Replace(folder("test-skill")),
        )
        .unwrap();
    assert!(matches!(replace, SkillBankChange::ReplaceSkill { .. }));
    let remove = surface
        .change_part(&bank, skill_name("test-skill"), SkillFolderEdit::Remove)
        .unwrap();
    assert!(matches!(remove, SkillBankChange::RemoveSkill { .. }));
    let rename = surface
        .change_part(
            &bank,
            skill_name("test-skill"),
            SkillFolderEdit::Rename(skill_name("renamed-skill")),
        )
        .unwrap();
    assert!(matches!(rename, SkillBankChange::RenameSkill { .. }));
    assert!(
        surface
            .change_part(&bank, skill_name("missing-skill"), SkillFolderEdit::Remove)
            .is_err()
    );
}

#[test]
fn file_surface_exposes_files_and_translates_all_file_edits() {
    let bank = bank();
    let surface = SkillFileSurface;
    let parts = surface.parts(&bank).unwrap();
    assert_eq!(parts.len(), 2);
    assert!(
        parts
            .iter()
            .any(|part| part.address == "test-skill/SKILL.md")
    );
    assert_ne!(surface.fingerprint(), SkillManifestSurface.fingerprint());

    let id = SkillFilePartId {
        skill: skill_name("test-skill"),
        path: skill_path("scripts/run.py"),
    };
    assert!(matches!(
        surface
            .change_part(&bank, id.clone(), SkillFileEdit::Remove)
            .unwrap(),
        SkillBankChange::RemoveFile { .. }
    ));
    assert!(matches!(
        surface
            .change_part(
                &bank,
                id.clone(),
                SkillFileEdit::Rename(skill_path("scripts/new.py"))
            )
            .unwrap(),
        SkillBankChange::RenameFile { .. }
    ));
    assert!(matches!(
        surface
            .change_part(&bank, id, SkillFileEdit::SetExecutable(true))
            .unwrap(),
        SkillBankChange::SetExecutable {
            executable: true,
            ..
        }
    ));
    assert!(
        surface
            .change_part(
                &bank,
                SkillFilePartId {
                    skill: skill_name("test-skill"),
                    path: skill_path("missing.py"),
                },
                SkillFileEdit::Remove,
            )
            .is_err()
    );
}

#[test]
fn manifest_surface_exposes_frontmatter_parts_and_preserves_body() {
    let bank = bank();
    let surface = SkillManifestSurface;
    let parts = surface.parts(&bank).unwrap();
    assert_eq!(parts[0].address, "test-skill/SKILL.md#frontmatter");
    assert_ne!(surface.fingerprint(), SkillFolderSurface.fingerprint());

    let changed = bank
        .apply_change(
            &surface
                .change_part(
                    &bank,
                    SkillManifestPartId {
                        skill: skill_name("test-skill"),
                    },
                    SkillManifestEdit::Replace {
                        description: SkillDescription::new("Use when manifest metadata changes.")
                            .unwrap(),
                        metadata: BTreeMap::from([(
                            "routing".to_owned(),
                            SkillMetadataValue::String("tests".to_owned()),
                        )]),
                    },
                )
                .unwrap(),
        )
        .unwrap();
    let folder = changed.get(&skill_name("test-skill")).unwrap();
    assert_eq!(folder.body().as_str(), "Do the testing work.\n");
    assert_eq!(
        folder.manifest().metadata.get("routing"),
        Some(&SkillMetadataValue::String("tests".to_owned()))
    );
}
