//! SigmaOS releng component.
//!
//! Rust implementation of `archlinux/releng` release-engineering tooling.
//!
//! Ports the self-contained pieces of upstream releng:
//! - [`ipxe`] — generates the Arch netboot iPXE script from live release and
//!   mirror data (upstream `ipxe/generate_archlinux_ipxe.py` +
//!   `archlinux.ipxe.jinja`).
//! - [`codesign`] — models the Secure Boot code-signing key-pair workflow
//!   (`codesigning/create_codesigning_key_pair.sh`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod codesign;
pub mod error;
pub mod ipxe;

pub use codesign::{CertPlan, KeyPairPlan, plan_cert_enrollment, plan_key_pair};
pub use error::RelengError;
pub use ipxe::{Mirror, MirrorGroup, Release, generate_ipxe, group_mirrors, render_ipxe};
