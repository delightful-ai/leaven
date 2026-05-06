//! leaven-render crate skeleton.

pub struct CandidateTreeHtmlRenderer;
pub struct RunGraphDebugRenderer;
pub struct LineageSummaryRenderer;
pub struct ReflectionPromptRenderer;
pub struct StructuredPromptRenderer;
pub struct SurfaceDiffRenderer;
pub struct SurfacePartsRenderer;
pub struct ArtifactWorkspaceRenderer;
pub struct HistoryWorkspaceRenderer;
pub struct SurfaceWorkspaceRenderer;
pub mod prelude {
    pub use crate::{
        ArtifactWorkspaceRenderer, CandidateTreeHtmlRenderer, HistoryWorkspaceRenderer,
        LineageSummaryRenderer, ReflectionPromptRenderer, RunGraphDebugRenderer,
        StructuredPromptRenderer, SurfaceDiffRenderer, SurfacePartsRenderer,
        SurfaceWorkspaceRenderer,
    };
}
