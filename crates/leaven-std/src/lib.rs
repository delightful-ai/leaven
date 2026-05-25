//! Curated standard library for Leaven.

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

pub mod surfaces {
    //! Standard surface exports.

    pub use leaven_surface::*;
}

pub mod prelude {
    //! Common standard-library imports.

    pub use leaven_evidence::prelude::*;
    pub use leaven_population::prelude::*;
    pub use leaven_preference::prelude::*;
    pub use leaven_surface::prelude::*;

    #[cfg(feature = "git")]
    pub use leaven_artifact_git::*;

    #[cfg(feature = "jj")]
    pub use leaven_artifact_jj::*;

    #[cfg(feature = "skill")]
    pub use leaven_artifact_skill::*;
}
