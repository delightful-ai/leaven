//! Stable behavior fingerprints.
//!
//! Fingerprints identify *behavior*, not content. They appear on
//! evaluators, surfaces, renderers, and other stages whose configuration
//! affects what they produce. The evaluation cache mixes fingerprints
//! into its keys: changing an evaluator's prompt should invalidate prior
//! cache entries, even though the evaluator's `EvaluatorId` is the same.
//!
//! Fingerprints are 32 bytes wide and computed with BLAKE3 by default.
//! Implementors are responsible for feeding *every* configuration bit
//! that affects behavior into the [`FingerprintBuilder`]; missing inputs
//! produce silent cache poisoning.

use serde::{Deserialize, Serialize};

/// 32-byte BLAKE3 fingerprint identifying a stage's behavior.
///
/// Used as a cache-key ingredient and as a stability check across runs:
/// two stages with the same `Fingerprint` are expected to produce the
/// same outputs given the same inputs. Implementations must include
/// every behavior-affecting configuration parameter in the hash.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fingerprint(pub [u8; 32]);

impl Fingerprint {
    /// Wraps a raw 32-byte hash into a fingerprint. Use this when the
    /// hash already exists (e.g. computed by an external system); use
    /// [`FingerprintBuilder`] otherwise.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the full lowercase hexadecimal encoding of this fingerprint.
    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

/// Incremental builder that hashes feed-in bytes into a [`Fingerprint`].
///
/// Buffers bytes until [`finish`] runs them through BLAKE3 in one pass.
/// Internally a single `Vec<u8>`; for fingerprint inputs that are large
/// or already in memory, prefer feeding via [`update`] in chunks rather
/// than accumulating the full payload yourself.
///
/// [`update`]: FingerprintBuilder::update
/// [`finish`]: FingerprintBuilder::finish
#[derive(Default)]
pub struct FingerprintBuilder {
    bytes: Vec<u8>,
}

impl FingerprintBuilder {
    /// Constructs an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends bytes to the buffer to be hashed at finish time.
    ///
    /// Order matters: the same bytes appended in different orders
    /// produce different fingerprints. Implementors must keep their
    /// feeding order stable across versions or accept that fingerprints
    /// — and therefore cache keys — change between releases.
    pub fn update(&mut self, bytes: impl AsRef<[u8]>) -> &mut Self {
        self.bytes.extend_from_slice(bytes.as_ref());
        self
    }

    /// Hashes the accumulated buffer with BLAKE3 and returns the
    /// resulting [`Fingerprint`].
    #[must_use]
    pub fn finish(self) -> Fingerprint {
        Fingerprint(*blake3::hash(&self.bytes).as_bytes())
    }
}
