//! Kit components for kui, with typed Rust builders and an embedded Lua runtime.
//!
//! Register [`extension()`] under [`NAMESPACE`], then place components from
//! your Rust view. All Lua modules and Kit's shaders are embedded: there
//! are no runtime asset paths to configure or distribute.
//!
//! ```no_run
//! use kui_kit as kit;
//! use kui_native::{App, Ui};
//!
//! struct Hello;
//! impl App for Hello {
//!     fn view(&mut self, ui: &mut Ui<'_>) {
//!         kit::Settings::new().skin(kit::Skin::Neutral).place(ui);
//!         kit::Button::new("Hello from Rust").place(ui);
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     kui_native::app("Kit")
//!         .extension_as(kit::NAMESPACE, kit::extension()?)
//!         .run(Hello)
//! }
//! ```
//!
//! Each component has a constructor for its required props, setters for
//! optional props, and `place` / `place_with` methods. Event handlers carry
//! the host's messages; event structs decode their data with `from_payload`.

mod components;
mod embedded;

#[doc(inline)]
pub use components::*;
pub use embedded::extension;
