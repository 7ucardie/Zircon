//! Library surface for `zircon-patchmgr`.
//!
//! Exposes the core modules so benchmarks and future integration code can
//! import them without going through the CLI binary.

pub mod compress;
pub mod patch_info;
pub mod scanner;
