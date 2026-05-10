use std::fmt;
use std::future::Future;

use leaven_kernel::{Fingerprint, Metered};
use serde::{Deserialize, Serialize};

use crate::{LmError, LmRequest, LmResponse};

/// Provider-neutral language-model capability.
pub trait Lm: Send + Sync {
    /// Stable identifier for this LM implementation.
    fn id(&self) -> LmId;

    /// Behavior fingerprint for cache keys and run reproducibility.
    fn fingerprint(&self) -> Fingerprint;

    /// Runs one completion request and returns the assistant response with cost.
    fn complete(
        &self,
        request: LmRequest,
    ) -> impl Future<Output = Result<Metered<LmResponse>, LmError>> + Send + '_;
}

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Creates a new string identifier.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Returns the identifier as a string slice.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(LmId);
string_id!(ModelName);
string_id!(ProviderName);
