//! GEPA public builder ladders.

use leaven_population::ParetoFrontier;

use crate::{
    DefaultReflectionRenderer, Gepa, LmBackedReflector, LmBackedReflectorConfig, MissingReflector,
    PlainTextEditParser, PopulationBestFallback, RoundRobinPart, StrictImprovement,
};

impl Gepa<(), ParetoFrontier, MissingReflector> {
    /// Starts the reference GEPA profile.
    #[must_use]
    pub fn reference() -> GepaReferenceBuilder {
        GepaReferenceBuilder
    }

    /// Starts a GEPA builder.
    #[must_use]
    pub fn builder() -> GepaBuilder {
        GepaBuilder
    }

    /// Starts a GEPA builder with the standard LM-backed reflection path.
    #[must_use]
    pub fn reflect_with_lm<L>(
        lm: L,
        model: impl Into<leaven_lm::ModelName>,
    ) -> GepaReflectWithLmBuilder<
        LmBackedReflector<L, DefaultReflectionRenderer, PlainTextEditParser>,
    > {
        GepaReflectWithLmBuilder {
            reflector: LmBackedReflector::with_default_renderer(lm, model),
        }
    }
}

/// Builder for the reference GEPA profile.
#[derive(Clone, Debug, Default)]
pub struct GepaReferenceBuilder;

impl GepaReferenceBuilder {
    /// Supplies the required edit surface.
    #[must_use]
    pub fn surface<S>(self, surface: S) -> GepaReferenceBuilderWithSurface<S> {
        GepaReferenceBuilderWithSurface { surface }
    }

    /// Starts a reference GEPA builder with the standard LM-backed reflection path.
    #[must_use]
    pub fn reflect_with_lm<L>(
        self,
        lm: L,
        model: impl Into<leaven_lm::ModelName>,
    ) -> GepaReflectWithLmBuilder<
        LmBackedReflector<L, DefaultReflectionRenderer, PlainTextEditParser>,
    > {
        Gepa::reflect_with_lm(lm, model)
    }
}

/// Reference GEPA profile builder after the edit surface is known.
#[derive(Clone, Debug)]
pub struct GepaReferenceBuilderWithSurface<S> {
    surface: S,
}

impl<S> GepaReferenceBuilderWithSurface<S> {
    /// Supplies an explicit reflector and builds reference GEPA defaults.
    #[must_use]
    pub fn reflector<Reflect>(
        self,
        reflector: Reflect,
    ) -> Gepa<S, ParetoFrontier, Reflect, PopulationBestFallback, RoundRobinPart, StrictImprovement>
    {
        Gepa::new(self.surface, ParetoFrontier::by_case().build(), reflector)
    }

    /// Supplies the standard LM-backed reflector and builds reference GEPA defaults.
    #[must_use]
    pub fn reflect_with_lm<L>(
        self,
        lm: L,
        model: impl Into<leaven_lm::ModelName>,
    ) -> Gepa<
        S,
        ParetoFrontier,
        LmBackedReflector<L, DefaultReflectionRenderer, PlainTextEditParser>,
        PopulationBestFallback,
        RoundRobinPart,
        StrictImprovement,
    > {
        self.reflector(LmBackedReflector::with_default_renderer(lm, model))
    }
}

/// GEPA builder entrypoint.
#[derive(Clone, Debug, Default)]
pub struct GepaBuilder;

impl GepaBuilder {
    /// Supplies the required edit surface.
    #[must_use]
    pub fn surface<S>(self, surface: S) -> GepaBuilderWithSurface<S> {
        GepaBuilderWithSurface { surface }
    }
}

/// Builder started from a default LM-backed reflector.
#[derive(Clone, Debug)]
pub struct GepaReflectWithLmBuilder<Reflect> {
    reflector: Reflect,
}

impl<Reflect> GepaReflectWithLmBuilder<Reflect> {
    /// Supplies the required edit surface.
    #[must_use]
    pub fn surface<S>(self, surface: S) -> GepaReflectWithLmBuilderWithSurface<S, Reflect> {
        GepaReflectWithLmBuilderWithSurface {
            surface,
            reflector: self.reflector,
        }
    }
}

impl<L>
    GepaReflectWithLmBuilder<LmBackedReflector<L, DefaultReflectionRenderer, PlainTextEditParser>>
{
    /// Override default LM-backed reflection controls before choosing a surface.
    #[must_use]
    pub fn with_reflector_config(self, config: LmBackedReflectorConfig) -> Self {
        Self {
            reflector: self.reflector.with_config(config),
        }
    }
}

/// LM-backed builder after the edit surface is known.
#[derive(Clone, Debug)]
pub struct GepaReflectWithLmBuilderWithSurface<S, Reflect> {
    surface: S,
    reflector: Reflect,
}

impl<S, Reflect> GepaReflectWithLmBuilderWithSurface<S, Reflect> {
    /// Builds GEPA with default population policy.
    #[must_use]
    pub fn build(
        self,
    ) -> Gepa<S, ParetoFrontier, Reflect, PopulationBestFallback, RoundRobinPart, StrictImprovement>
    {
        Gepa::new(
            self.surface,
            ParetoFrontier::by_case().build(),
            self.reflector,
        )
    }

    /// Supplies explicit population and builds GEPA.
    #[must_use]
    pub fn population<Pop>(
        self,
        population: Pop,
    ) -> Gepa<S, Pop, Reflect, PopulationBestFallback, RoundRobinPart, StrictImprovement> {
        Gepa::new(self.surface, population, self.reflector)
    }
}

/// Builder after the edit surface is known.
#[derive(Clone, Debug)]
pub struct GepaBuilderWithSurface<S> {
    surface: S,
}

impl<S> GepaBuilderWithSurface<S> {
    /// Supplies the reflective proposer and builds default population policy.
    #[must_use]
    pub fn reflector<Reflect>(
        self,
        reflector: Reflect,
    ) -> Gepa<S, ParetoFrontier, Reflect, PopulationBestFallback, RoundRobinPart, StrictImprovement>
    {
        Gepa::new(self.surface, ParetoFrontier::by_case().build(), reflector)
    }

    /// Supplies explicit population and reflective proposer.
    #[must_use]
    pub fn population<Pop>(self, population: Pop) -> GepaBuilderWithPopulation<S, Pop> {
        GepaBuilderWithPopulation {
            surface: self.surface,
            population,
        }
    }
}

/// Builder after surface and population are known.
#[derive(Clone, Debug)]
pub struct GepaBuilderWithPopulation<S, Pop> {
    surface: S,
    population: Pop,
}

impl<S, Pop> GepaBuilderWithPopulation<S, Pop> {
    /// Supplies the reflective proposer.
    #[must_use]
    pub fn reflector<Reflect>(
        self,
        reflector: Reflect,
    ) -> Gepa<S, Pop, Reflect, PopulationBestFallback, RoundRobinPart, StrictImprovement> {
        Gepa::new(self.surface, self.population, reflector)
    }
}
