//! Signal processing building blocks. Everything here is allocation-free after construction.

pub mod biquad;
pub mod eq;
pub mod filter;
pub mod transition;

#[cfg(test)]
pub(crate) mod testutil;
