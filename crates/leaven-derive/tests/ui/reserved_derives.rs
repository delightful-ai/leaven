use leaven_derive::{
    Artifact as DeriveArtifact, ContentAddressed as DeriveContentAddressed,
    EditSurface as DeriveEditSurface,
};

#[derive(Clone, DeriveArtifact)]
struct ArtifactOnly;

#[derive(Clone, DeriveContentAddressed)]
struct ContentOnly;

#[derive(Clone, DeriveEditSurface)]
struct SurfaceOnly;

fn main() {}
