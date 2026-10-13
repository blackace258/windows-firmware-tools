//! Library surface for bios-tool (enables unit/integration tests without hardware).

pub mod cli;
pub mod domain;
pub mod backend;
pub mod setup;
pub mod acquisition;
pub mod firmware;
pub mod validation;
pub mod programming;
pub mod audit;

#[cfg(feature = "gui")]
pub mod gui;
