#![feature(let_chains)]
#![cfg_attr(not(test), warn(clippy::unwrap_used))]
#![warn(clippy::pedantic, clippy::panic)]
#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

pub mod contract;
pub mod error;
pub mod msg;
pub mod state;

pub mod evm;
