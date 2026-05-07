//! leaven-render crate skeleton.

mod candidate_tree;
mod lineage;
mod materializer;
mod prompt;
mod run_graph;
mod surface;

pub use candidate_tree::CandidateTreeHtmlRenderer;
pub use lineage::LineageSummaryRenderer;
pub use materializer::{ArtifactMaterializer, HistoryMaterializer, SurfaceMaterializer};
pub use prompt::{ReflectionPromptRenderer, StructuredPromptRenderer};
pub use run_graph::RunGraphDebugRenderer;
pub use surface::{SurfaceDiffRenderer, SurfacePartsRenderer};

pub mod prelude {
    pub use crate::{
        ArtifactMaterializer, CandidateTreeHtmlRenderer, HistoryMaterializer,
        LineageSummaryRenderer, ReflectionPromptRenderer, RunGraphDebugRenderer,
        StructuredPromptRenderer, SurfaceDiffRenderer, SurfaceMaterializer, SurfacePartsRenderer,
    };
}
