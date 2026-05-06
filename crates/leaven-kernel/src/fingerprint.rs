//! Stable behavior fingerprints.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Default)]
pub struct FingerprintBuilder {
    bytes: Vec<u8>,
}

impl FingerprintBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, bytes: impl AsRef<[u8]>) -> &mut Self {
        self.bytes.extend_from_slice(bytes.as_ref());
        self
    }

    #[must_use]
    pub fn finish(self) -> Fingerprint {
        Fingerprint(*blake3::hash(&self.bytes).as_bytes())
    }
}
