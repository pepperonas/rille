//! Signal processing building blocks. Everything here is allocation-free after construction.

pub mod biquad;
pub mod eq;
pub mod filter;

#[cfg(test)]
pub(crate) mod testutil;
