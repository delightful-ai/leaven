//! Curated standard library for Leaven.

pub mod artifacts {
    //! Standard artifact implementations.

    pub use leaven_artifacts::*;

    #[cfg(feature = "git")]
    pub use leaven_artifact_git::*;

    #[cfg(feature = "jj")]
    pub use leaven_artifact_jj::*;
}

pub mod evidence {
    //! Standard evidence shapes.

    pub use leaven_evidence::*;
}

pub mod preferences {
    //! Stateless preference relations.

    pub use leaven_preference::*;
}

pub mod populations {
    //! Standard populations.

    pub use leaven_population::*;
}

pub mod render {
    //! Standard renderers.

    pub use leaven_render::*;
}

pub mod surfaces {
    //! Standard surface exports.

    pub use leaven_surface::*;
}

pub mod prelude {
    //! Common standard-library imports.

    pub use leaven_artifacts::prelude::*;
    pub use leaven_evidence::prelude::*;
    pub use leaven_population::prelude::*;
    pub use leaven_preference::prelude::*;
    pub use leaven_render::prelude::*;
    pub use leaven_surface::prelude::*;
}
