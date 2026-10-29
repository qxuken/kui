//! kui's access tree to the platform's accessibility API, through
//! AccessKit (backlog F269).
//!
//! The core derives the tree, the announcements and the requests coming
//! back as data (docs/adr/0001-accessibility-as-data.md); something has to
//! hand them to the platform. Two things do, and both build on this crate:
//!
//! - **The runner's windows** (`kui-native`'s `access_bridge`), through
//!   `accesskit_winit`, which plumbs a winit window's events.
//! - **A host with a window of its own** — GLFW, SDL3, an engine's — through
//!   `HostBridge` (the `platform` feature, on by default): AccessKit's
//!   platform adapter on the host's native handle, with no winit. The C
//!   library's `kui_access_attach` is this.
//!
//! What they share is the translation: [`tree_update`] (the tree as one
//! AccessKit update, the platform role from [`role_of`]), [`request_of`]
//! (a platform request as the core's `AccessRequest`) and the [`Feed`]
//! (when a window's tree is sent, and its announcements as live nodes).
//!
//! It is a crate of its own because `kui-core` holds no platform code and
//! `kui-native` is the whole runtime (winit, wgpu, audio): the
//! runner-less C library links this and none of that.

mod translate;
pub use translate::{Feed, request_of, role_of, tree_update};

/// The AccessKit this crate speaks, for a bridge that builds on it.
pub use accesskit;

#[cfg(feature = "platform")]
mod host;
#[cfg(feature = "platform")]
pub use host::HostBridge;
