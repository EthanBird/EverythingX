#![forbid(unsafe_code)]

/// Minimal shape only. Real operators define family-specific typed input,
/// output, parameters, errors, limits, and evidence-backed guarantees.
pub trait AtomicOperator<I, O> {
    type Error;

    fn apply(&self, input: I) -> Result<O, Self::Error>;
}

