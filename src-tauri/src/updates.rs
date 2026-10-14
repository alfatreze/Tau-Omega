//! The GitHub update check and download (roadmap item 1b). The code lives in the `tau-net` crate so the `tau` command line
//! shares it; this module only re-exports it for the commands in `main.rs`.
pub use tau_net::{Downloaded, check, download};
