//! MCP integration for local Muse projects.
//!
//! This crate owns the MCP transport boundary. Muse language compilation stays
//! independent from the protocol and can be used without an MCP client.

mod server;
mod tools;
mod types;

pub use server::{MuseMcpServer, serve};
