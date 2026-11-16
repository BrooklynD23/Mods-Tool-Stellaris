//! Test-only trait probes.
//!
//! `implements!(Ty: Trait)` evaluates to `true` or `false` at compile time without
//! requiring `Ty: Trait`. It is how the negative half of Invariant C1 is asserted: `Fact`
//! must **not** implement `serde::Serialize`, and "must not implement" is not otherwise
//! expressible as a test.
//!
//! The mechanism is inherent-impl-beats-blanket-trait resolution: the inherent
//! `Wrapper<T>::IMPLS` is only a candidate when its bound holds, and the blanket trait
//! const is the fallback when it does not.

macro_rules! implements {
    ($type:ty: $($bound:tt)+) => {{
        #[allow(dead_code)]
        trait DoesNotImpl {
            const IMPLS: bool = false;
        }
        impl<T> DoesNotImpl for T {}
        struct Wrapper<T>(::core::marker::PhantomData<T>);
        #[allow(dead_code)]
        impl<T: $($bound)+> Wrapper<T> {
            const IMPLS: bool = true;
        }
        <Wrapper<$type>>::IMPLS
    }};
}

#[test]
fn the_probe_itself_works() {
    struct Unordered;
    assert!(implements!(u32: Ord));
    assert!(!implements!(Unordered: Ord));
}
