use private::Sealed;
pub use rand_core::RngCore;

/// Type-level version of the [`None`] variant
pub struct NoneT;

/// Type-level Option representing a type that can either implement [`RngCore`] or be [`NoneT`].
/// This trait is an implementation detail and should not be implemented outside this crate.
#[doc(hidden)]
pub trait OptionalRng: Sealed {}

impl Sealed for NoneT {}
impl OptionalRng for NoneT {}

impl<T: RngCore> Sealed for T {}
impl<T: RngCore> OptionalRng for T {}

/// Representation of the physical radio + RNG. Two variants may be constructed through [`Device`].
/// Either:
/// * `R` implements [`RngCore`], or
/// * `G` implements [`RngCore`].
///
/// This allows for seamless functionality with either RNG variant and is an implementation detail.
/// Users are not expected to construct [`Phy`] directly. Use the constructors for [`Device`]
/// instead.
pub struct Phy<R, G: OptionalRng> {
    pub radio: R,
    rng: G,
}

impl<R, G: OptionalRng> Phy<R, G> {
    pub fn new(radio: R, rng: G) -> Self {
        Self { radio, rng }
    }
}

impl<R: RngCore> Sealed for Phy<R, NoneT> {}
impl<R: RngCore> GetRng for Phy<R, NoneT> {
    type RNG = R;
    fn get_rng(&mut self) -> &mut Self::RNG {
        &mut self.radio
    }
}
impl<R, G: RngCore> Sealed for Phy<R, G> {}
impl<R, G> GetRng for Phy<R, G>
where
    G: RngCore,
{
    type RNG = G;
    fn get_rng(&mut self) -> &mut Self::RNG {
        &mut self.rng
    }
}

impl<T: RngCore> GetRng for T {
    type RNG = Self;
    fn get_rng(&mut self) -> &mut Self::RNG {
        self
    }
}

/// Trait used to mark types which can give out an exclusive reference to [`RngCore`].
/// This trait is an implementation detail and should not be implemented outside this crate.
#[doc(hidden)]
pub trait GetRng: Sealed {
    type RNG: RngCore;
    fn get_rng(&mut self) -> &mut Self::RNG;
}

mod private {
    /// Super trait used to mark traits with an exhaustive set of
    /// implementations
    pub trait Sealed {}
}
