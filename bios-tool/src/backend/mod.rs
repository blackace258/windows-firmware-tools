//! Hardware backend: flashrom as child process only.
//! Never implement custom low-level SPI. Capture stdout/stderr/exit/args/version.
//! Never pass user strings through a shell.

pub mod flashrom;

pub use flashrom::*;
