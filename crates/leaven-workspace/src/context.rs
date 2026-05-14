//! Typed context attached by workspace factories.

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct WorkspaceFactoryContext {
    entries: Arc<BTreeMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl WorkspaceFactoryContext {
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn builder() -> WorkspaceFactoryContextBuilder {
        WorkspaceFactoryContextBuilder::default()
    }

    pub fn get<T>(&self) -> Result<Arc<T>, WorkspaceFactoryContextError>
    where
        T: Any + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let Some(value) = self.entries.get(&type_id) else {
            return Err(WorkspaceFactoryContextError::Missing {
                type_name: std::any::type_name::<T>(),
            });
        };
        value
            .clone()
            .downcast::<T>()
            .map_err(|_| WorkspaceFactoryContextError::TypeMismatch {
                type_name: std::any::type_name::<T>(),
            })
    }
}

#[derive(Default)]
pub struct WorkspaceFactoryContextBuilder {
    entries: BTreeMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl WorkspaceFactoryContextBuilder {
    pub fn insert<T>(&mut self, value: Arc<T>) -> Result<(), WorkspaceFactoryContextError>
    where
        T: Any + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        if self.entries.contains_key(&type_id) {
            return Err(WorkspaceFactoryContextError::Duplicate {
                type_name: std::any::type_name::<T>(),
            });
        }
        self.entries.insert(type_id, value);
        Ok(())
    }

    #[must_use]
    pub fn build(self) -> WorkspaceFactoryContext {
        WorkspaceFactoryContext {
            entries: Arc::new(self.entries),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceFactoryContextError {
    #[error("workspace factory context missing value of type {type_name}")]
    Missing { type_name: &'static str },

    #[error("workspace factory context already has a value of type {type_name}")]
    Duplicate { type_name: &'static str },

    #[error("workspace factory context value had wrong type for {type_name}")]
    TypeMismatch { type_name: &'static str },
}
