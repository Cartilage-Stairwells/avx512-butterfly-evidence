//! `VectorEngine` trait.
//!
//! Spec: PORTABLE_BUTTERFLY_SPEC.md §6.
//!
//! An engine operates on **encoded representations** (`Repr`), not on
//! abstract `Field` objects. Montgomery reduction is internal to each
//! engine implementation; it is deliberately not abstracted here (§3:
//! abstracting Montgomery reduction typically produces worse code).
//!
//! `LANES` is a compile-time constant declared BY each engine. This trait
//! does not require any particular SIMD width, register type, or
//! instruction set. No runtime branching inside an engine.
//!
//! Lane independence requirement (§6): for a vector of width N, each lane
//! MUST behave as an independent scalar operation. No unintended
//! cross-lane interaction is permitted.
//!
//! No implementation of this trait is part of migration-commit-2
//! (traits only, per the frozen seven-step sequence).

/// Execution engine over encoded representations (spec §6, signature verbatim).
pub trait VectorEngine {
    /// Physical storage representation.
    ///
    /// Mathematical field: BabyBear
    /// Storage:            u32
    /// Encoding:           Montgomery residue
    type Repr;

    type Vector;

    const LANES: usize;

    unsafe fn load(ptr: *const Self::Repr) -> Self::Vector;

    unsafe fn store(ptr: *mut Self::Repr, value: Self::Vector);

    fn add(a: Self::Vector, b: Self::Vector) -> Self::Vector;

    fn sub(a: Self::Vector, b: Self::Vector) -> Self::Vector;

    fn mul(a: Self::Vector, b: Self::Vector) -> Self::Vector;
}
