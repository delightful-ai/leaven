//! leaven-render crate skeleton.

pub struct CandidateTreeHtmlRenderer;
pub struct RunGraphDebugRenderer;
pub struct LineageSummaryRenderer;
pub struct ReflectionPromptRenderer;
pub struct StructuredPromptRenderer;
pub struct SurfaceDiffRenderer;
pub struct SurfacePartsRenderer;
pub struct ArtifactMaterializer;
pub struct HistoryMaterializer;
pub struct SurfaceMaterializer;
pub mod prelude {
    pub use crate::{
        ArtifactMaterializer, CandidateTreeHtmlRenderer, HistoryMaterializer,
        LineageSummaryRenderer, ReflectionPromptRenderer, RunGraphDebugRenderer,
        StructuredPromptRenderer, SurfaceDiffRenderer, SurfaceMaterializer, SurfacePartsRenderer,
    };
}
