//! Projects the locked `agent_kit` wire record into the flat Git-program file
//! map the host seeds a run-scoped repository from, and projects an evolved
//! revision's flat file map back into the wire record shape.
//!
//! The `agent_kit` wire record (`{ system_prompt, skills: [{ path, content }] }`)
//! is a lossy projection of a Git-backed `AgentKit` revision: V1 carries the
//! `system_prompt` slot and the `skills` subtree and omits `agent_docs`. This
//! module is the only place that knows the kit's on-disk layout
//! (`manifest.toml`, `system_prompt.md`, `skills/<path>`); the generic Git seed
//! construction and revision readback live in `leaven-agentic-git`.

use std::collections::BTreeMap;

use leaven_artifact_git::GitPath;
use leaven_public_seam::SkillFile;
use serde_json::{Value, json};

use crate::optimize_run_service::error::OptimizeRunHostError;

/// On-disk path of the kit manifest the materializer reads.
const MANIFEST_PATH: &str = "manifest.toml";
/// On-disk path of the system-prompt slot.
const SYSTEM_PROMPT_PATH: &str = "system_prompt.md";
/// On-disk subtree the Codex profile mounts as skills.
const SKILLS_DIR: &str = "skills";

/// One parsed kit wire part: the system prompt plus its skill files.
pub(in crate::optimize_run_service) struct KitParts {
    pub(in crate::optimize_run_service) system_prompt: String,
    pub(in crate::optimize_run_service) skills: Vec<(String, String)>,
}

/// Lowers the wire `agent_kit` parts into the flat Git-program seed file map.
///
/// The file map always carries a `manifest.toml` declaring the `system_prompt`
/// slot (and the `skills` slot when any skill file is present), the
/// `system_prompt.md` body, and each skill file under `skills/<path>`. The skill
/// paths were already validated against the `AgentKit` path law at the wire
/// layer, so they are joined under the skills subtree without re-validation
/// beyond `GitPath` construction.
pub(in crate::optimize_run_service) fn kit_files_from_wire(
    system_prompt: &str,
    skills: &[SkillFile],
) -> Result<BTreeMap<GitPath, Vec<u8>>, OptimizeRunHostError> {
    let mut files = BTreeMap::new();
    files.insert(
        git_path(MANIFEST_PATH)?,
        manifest_toml(!skills.is_empty()).into_bytes(),
    );
    files.insert(
        git_path(SYSTEM_PROMPT_PATH)?,
        system_prompt.as_bytes().to_vec(),
    );
    for skill in skills {
        let path = git_path(&format!("{SKILLS_DIR}/{}", skill.path()))?;
        files.insert(path, skill.content().as_bytes().to_vec());
    }
    Ok(files)
}

/// Projects a revision's flat file map back into the wire `agent_kit` parts.
///
/// This is the inverse of [`kit_files_from_wire`] for one revision: it reads the
/// `system_prompt.md` body and every file under `skills/` (stripped back to its
/// wire-relative path). The `manifest.toml` is layout the host owns, not wire
/// content, so it is not projected back.
pub(in crate::optimize_run_service) fn kit_parts_from_files(
    files: &BTreeMap<GitPath, Vec<u8>>,
) -> Result<KitParts, OptimizeRunHostError> {
    let system_prompt = files
        .get(&git_path(SYSTEM_PROMPT_PATH)?)
        .map(|bytes| decode_utf8(bytes, SYSTEM_PROMPT_PATH))
        .transpose()?
        .ok_or_else(|| {
            OptimizeRunHostError::projection(
                "agent_kit revision is missing system_prompt.md; the kit projection requires it",
            )
        })?;
    let skill_prefix = format!("{SKILLS_DIR}/");
    let mut skills = Vec::new();
    for (path, bytes) in files {
        let Some(relative) = path.as_str().strip_prefix(&skill_prefix) else {
            continue;
        };
        let content = decode_utf8(bytes, path.as_str())?;
        skills.push((relative.to_owned(), content));
    }
    Ok(KitParts {
        system_prompt,
        skills,
    })
}

/// Builds the wire `agent_kit` artifact value from parts.
pub(in crate::optimize_run_service) fn kit_wire_artifact(parts: &KitParts) -> Value {
    let skills = parts
        .skills
        .iter()
        .map(|(path, content)| json!({ "path": path, "content": content }))
        .collect::<Vec<_>>();
    json!({
        "system_prompt": parts.system_prompt,
        "skills": skills,
    })
}

fn manifest_toml(has_skills: bool) -> String {
    let mut manifest = String::from("schema = \"v1\"\nsystem_prompt = \"system_prompt.md\"\n");
    if has_skills {
        manifest.push_str("skills = \"skills\"\n");
    }
    manifest
}

fn git_path(path: &str) -> Result<GitPath, OptimizeRunHostError> {
    GitPath::new(path)
        .map_err(|error| OptimizeRunHostError::lowering(format!("invalid kit file path: {error}")))
}

fn decode_utf8(bytes: &[u8], path: &str) -> Result<String, OptimizeRunHostError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| {
        OptimizeRunHostError::projection(format!("agent_kit file `{path}` is not valid UTF-8"))
    })
}
