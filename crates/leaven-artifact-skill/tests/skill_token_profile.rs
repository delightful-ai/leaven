use std::collections::BTreeMap;
use std::str::FromStr;

use leaven_artifact_skill::{
    SkillBank, SkillBankChange, SkillFile, SkillFolder, SkillName, SkillPath, SkillTokenProfile,
    SkillTokenProfileError, SkillTokenizer,
};
use leaven_core::Artifact;

fn skill_name(value: &str) -> SkillName {
    SkillName::from_str(value).unwrap()
}

fn skill_path(value: &str) -> SkillPath {
    SkillPath::from_str(value).unwrap()
}

fn skill_md(name: &str, description: &str, body: &str) -> SkillFile {
    SkillFile::text(format!(
        "---\nname: {name}\ndescription: {description}\n---\n{body}\n"
    ))
}

fn folder_with_files(entries: BTreeMap<SkillPath, SkillFile>) -> SkillFolder {
    SkillFolder::from_entries(skill_name("product-marketing"), entries).unwrap()
}

fn skill_bank() -> SkillBank {
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        skill_md(
            "product-marketing",
            "Use when positioning B2B products.",
            "Always identify buyer, pain, capability, proof, and channel.",
        ),
    );
    entries.insert(
        skill_path("references/examples.md"),
        SkillFile::text("Example positioning statements.\n"),
    );
    entries.insert(
        skill_path("references/templates.md"),
        SkillFile::text("Template prompts.\n"),
    );
    entries.insert(
        skill_path("references/not-md.txt"),
        SkillFile::text("Non markdown reference bytes are not context modules.\n"),
    );
    entries.insert(
        skill_path("scripts/check.py"),
        SkillFile::text("print('not context')\n"),
    );
    SkillBank::from_folders([folder_with_files(entries)]).unwrap()
}

#[test]
fn token_profile_counts_description_body_and_direct_reference_modules() {
    let tokenizer = TableTokenizer::new([
        ("Use when positioning B2B products.", 6),
        (
            "Always identify buyer, pain, capability, proof, and channel.\n",
            9,
        ),
        ("Example positioning statements.\n", 3),
        ("Template prompts.\n", 2),
    ]);
    let profile = SkillTokenProfile::measure(&skill_bank(), &tokenizer).unwrap();
    let skill = profile.skill(&skill_name("product-marketing")).unwrap();

    assert_eq!(profile.tokenizer_id(), "table-tokenizer");
    assert_eq!(skill.description_tokens(), 6);
    assert_eq!(skill.body_tokens(), 9);
    assert_eq!(skill.always_loaded_tokens(), 15);
    assert_eq!(skill.reference_tokens_total(), 5);
    assert_eq!(skill.context_tokens(), 20);
    assert_eq!(
        skill.reference_tokens().keys().collect::<Vec<_>>(),
        [
            &skill_path("references/examples.md"),
            &skill_path("references/templates.md")
        ]
    );
    assert_eq!(profile.total_always_loaded_tokens(), 15);
    assert_eq!(profile.total_reference_tokens(), 5);
    assert_eq!(profile.total_context_tokens(), 20);
}

#[test]
fn token_profile_compares_before_and_after_skill_costs() {
    let tokenizer = TableTokenizer::new([
        ("Use when positioning B2B products.", 6),
        (
            "Always identify buyer, pain, capability, proof, and channel.\n",
            9,
        ),
        ("Use when positioning B2B products.", 6),
        ("Identify buyer, pain, proof, and channel.\n", 6),
        ("Example positioning statements.\n", 3),
        ("Template prompts.\n", 2),
        ("Long moved examples.\n", 8),
    ]);
    let before = SkillTokenProfile::measure(&skill_bank(), &tokenizer).unwrap();
    let after = SkillTokenProfile::measure(
        &skill_bank()
            .apply_change(&SkillBankChange::Atomic(vec![
                SkillBankChange::WriteFile {
                    skill: skill_name("product-marketing"),
                    path: SkillPath::skill_md(),
                    file: skill_md(
                        "product-marketing",
                        "Use when positioning B2B products.",
                        "Identify buyer, pain, proof, and channel.",
                    ),
                },
                SkillBankChange::WriteFile {
                    skill: skill_name("product-marketing"),
                    path: skill_path("references/examples.md"),
                    file: SkillFile::text("Long moved examples.\n"),
                },
            ]))
            .unwrap(),
        &tokenizer,
    )
    .unwrap();
    let comparison = before.compare(&after).unwrap();

    assert_eq!(comparison.before_always_loaded_tokens(), 15);
    assert_eq!(comparison.after_always_loaded_tokens(), 12);
    assert_eq!(comparison.always_loaded_token_change(), -3);
    assert_eq!(comparison.before_context_tokens(), 20);
    assert_eq!(comparison.after_context_tokens(), 22);
    assert_eq!(comparison.context_token_change(), 2);
}

#[test]
fn token_profile_refuses_to_compare_different_tokenizers() {
    let before = SkillTokenProfile::measure(
        &skill_bank(),
        &TableTokenizer::new([
            ("Use when positioning B2B products.", 6),
            (
                "Always identify buyer, pain, capability, proof, and channel.\n",
                9,
            ),
            ("Example positioning statements.\n", 3),
            ("Template prompts.\n", 2),
        ])
        .with_tokenizer_id("paper-tokenizer-a"),
    )
    .unwrap();
    let after = SkillTokenProfile::measure(
        &skill_bank(),
        &TableTokenizer::new([
            ("Use when positioning B2B products.", 12),
            (
                "Always identify buyer, pain, capability, proof, and channel.\n",
                18,
            ),
            ("Example positioning statements.\n", 6),
            ("Template prompts.\n", 4),
        ])
        .with_tokenizer_id("paper-tokenizer-b"),
    )
    .unwrap();

    let err = before.compare(&after).unwrap_err();

    assert!(matches!(
        err,
        SkillTokenProfileError::TokenizerMismatch { before, after }
            if before == "paper-tokenizer-a" && after == "paper-tokenizer-b"
    ));
}

#[test]
fn token_profile_rejects_non_utf8_markdown_references() {
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        skill_md(
            "product-marketing",
            "Use when testing token profile.",
            "Body.",
        ),
    );
    entries.insert(
        skill_path("references/binary.md"),
        SkillFile::new([0xff, 0xfe]),
    );
    let bank = SkillBank::from_folders([folder_with_files(entries)]).unwrap();
    let tokenizer = TableTokenizer::new([("Use when testing token profile.", 5), ("Body.\n", 1)]);
    let err = SkillTokenProfile::measure(&bank, &tokenizer).unwrap_err();

    assert!(matches!(
        err,
        SkillTokenProfileError::NonUtf8Reference { skill, path, .. }
            if skill == skill_name("product-marketing")
                && path == skill_path("references/binary.md")
    ));
}

#[derive(Default)]
struct TableTokenizer {
    tokenizer_id: &'static str,
    counts: BTreeMap<&'static str, u64>,
}

impl TableTokenizer {
    fn new<const N: usize>(entries: [(&'static str, u64); N]) -> Self {
        Self {
            tokenizer_id: "table-tokenizer",
            counts: BTreeMap::from(entries),
        }
    }

    fn with_tokenizer_id(mut self, tokenizer_id: &'static str) -> Self {
        self.tokenizer_id = tokenizer_id;
        self
    }
}

impl SkillTokenizer for TableTokenizer {
    fn tokenizer_id(&self) -> &str {
        self.tokenizer_id
    }

    fn count_tokens(&self, text: &str) -> u64 {
        *self
            .counts
            .get(text)
            .unwrap_or_else(|| panic!("unexpected tokenized text: {text:?}"))
    }
}
