//! smbios-tool library: safe SMBIOS acquisition and parsing for Windows.
//! Core principle: the published table is not a universal write API and is distinct
//! from any persistent firmware source of truth.
//!
//! Write path (module `write`) is gated by a mandatory safety state machine and
//! fails closed unless an operator-supplied vendor tool is authorized.

pub mod acquisition;
pub mod parser;
pub mod report;
pub mod compare;
pub mod capability;
pub mod write;
pub mod error;

pub use error::{SmbiosError, Result};
