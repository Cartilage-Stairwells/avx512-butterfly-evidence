//! Vector engine layer — execution over encoded representations.
//!
//! A `VectorEngine` operates on physical storage (`Repr`), not abstract field objects.
//! Montgomery reduction is internal to each engine implementation.
//! Lane independence is a hard requirement: no cross-lane interaction is permitted.

pub mod traits;
pub mod reference;
pub mod neon;
pub mod avx512;
