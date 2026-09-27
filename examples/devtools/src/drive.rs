//! The scaffolding a headless drive is built from — `kui_native::testing::Drive`
//! (backlog DX11, published so an app's tests drive the same way), on the
//! `Core` the harness built for the example.
//!
//! ```ignore
//! fn headless(&mut self, core: &mut Core) -> Result<(), String> {
//!     let mut d = Drive::new(core, 480.0, 320.0);
//!     d.frame(self);
//!     let add = d.key_of("add").ok_or("no add button")?;
//!     d.click_key(self, add);
//!     d.frame(self);
//!     d.check(self.count == 1, "one click counts once")
//! }
//! ```

/// A headless driver over a borrowed `Core`.
pub type Drive<'c> = kui_native::testing::Drive<&'c mut kui_native::Core>;
