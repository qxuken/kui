//! `#[derive(Message)]`: a Rust enum or struct to and from the plain-data
//! payload a kui message is.
//!
//! kui carries every message as a `Value` map shaped `{kind, ...fields}`,
//! so one view model serves Rust, Lua, C and Node alike. This derive lets a
//! Rust app keep a typed enum instead of building and picking apart those
//! maps by string: `widgets::button(ui, "Save", Msg::Save)` sends it and
//! `ev.message::<Msg>()` reads it back. Most apps get the derive through
//! kui-native (`use kui_native::Message`; cargo feature `derive`, on by
//! default) and never name this crate.
//!
//! # Example
//!
//! The generated code reaches kui through `kui_native`, which this crate
//! does not depend on, so the example is not compiled here.
//!
//! ```rust,ignore
//! use kui_native::widgets;
//! use kui_native::{App, Message, NodeSpec, Ui, UiEvent};
//!
//! #[derive(Message, Clone, Debug, PartialEq)]
//! enum Msg {
//!     Inc,                          // {kind: "inc"}
//!     Pick { id: u64 },             // {kind: "pick", id: 3}
//!     #[message(kind = "add10")]
//!     AddTen,                       // {kind: "add10"}
//! }
//!
//! #[derive(Default)]
//! struct Counter {
//!     count: i64,
//! }
//!
//! impl App for Counter {
//!     fn view(&mut self, ui: &mut Ui<'_>) {
//!         ui.with(NodeSpec::row().gap(8.0), |ui| {
//!             widgets::button(ui, "+1", Msg::Inc);
//!             widgets::button(ui, "+10", Msg::AddTen);
//!         });
//!     }
//!
//!     fn on_event(&mut self, ev: UiEvent) {
//!         match ev.message::<Msg>() {
//!             Some(Msg::Inc) => self.count += 1,
//!             Some(Msg::AddTen) => self.count += 10,
//!             Some(Msg::Pick { id }) => println!("picked {id}"),
//!             None => {} // not a `Msg`: a resize, a focus change
//!         }
//!     }
//! }
//! ```
//!
//! # What it generates
//!
//! For a type `Msg`:
//!
//! - `From<Msg> for Value`: a map whose `kind` is the variant's name in
//!   snake_case (`TabNew` is `"tab_new"`; a struct uses its own name) and
//!   whose other keys are the fields, a tuple variant's by position
//!   (`"0"`, `"1"`, ...).
//! - `TryFrom<&Value>` and `TryFrom<Value> for Msg`, with `MessageError`
//!   saying what did not fit.
//! - `MessageField for Msg`, so a message can be a field of another.
//! - With `#[message(on = …)]` on a variant, a second type, `MsgIn`: the
//!   message as an event brings it (see [below](#the-arriving-twin)).
//!
//! A field is anything that implements `MessageField`: `bool`, the
//! numbers, `String`, `Option<T>`, `Vec<T>`, `Box<T>`, `Value` and other
//! messages.
//!
//! # Attributes
//!
//! All under `#[message(...)]`:
//!
//! - on a variant or a struct, `kind = "..."` names its `kind` instead of
//!   the snake_case name;
//! - on an enum of unit variants only, `string` makes it a bare string
//!   (`"h"`) rather than a map (`{kind: "h"}`), which reads best for a
//!   field such as a split's direction;
//! - on the type, `crate = "..."` names the path the generated code
//!   reaches kui through: `::kui_native` unless said, `kui_core` for a
//!   crate that depends on kui-core alone, `kui_value` for one on no kui
//!   core;
//! - on a variant (or a struct), `on = <event>` gives the arriving twin
//!   that variant with the event's data; on the type, `incoming = "..."`
//!   names the twin and `incoming_derive(...)` lists its derives (see
//!   [below](#the-arriving-twin)).
//!
//! ```rust,ignore
//! #[derive(Message, Clone, Copy, Debug, PartialEq)]
//! #[message(string)]
//! enum Dir { H, V }                 // "h" / "v"
//!
//! #[derive(Message, Clone, Debug, PartialEq)]
//! #[message(crate = "kui_core")]
//! struct Resize { w: f64, h: f64 }  // {kind: "resize", w, h}
//! ```
//!
//! # The arriving twin
//!
//! A node's handler declares the message as it is (`.on_drag(Msg::Pan)`),
//! and the event arrives as that message with the event's data in one
//! field named for the event: `{kind: "pan", drag: {phase, x, y, dx, dy,
//! parent}}`. `ev.message::<Msg>()` reads the message alone. Marking the
//! variant `#[message(on = drag)]` makes the derive emit a second enum,
//! `MsgIn`, whose `Pan` carries the data, so one flat `match` reads both:
//!
//! ```rust,ignore
//! #[derive(Message, Clone, Debug, PartialEq)]
//! enum Msg {
//!     #[message(on = drag)]
//!     Pan,                          // MsgIn::Pan { drag: kui_native::Drag }
//!     #[message(on = change)]
//!     Rename { id: u64 },           // MsgIn::Rename { id, change: EditChange }
//!     #[message(on = slider_change)]
//!     Volume(u8),                   // MsgIn::Volume(u8, SliderChange)
//!     Save,                         // MsgIn::Save, as declared
//! }
//!
//! // the view: .on_drag(Msg::Pan), .on_change(Msg::Rename { id }) on an
//! // editor, widgets::slider(ui, "vol", v, 0.0, 1.0, 0.1, Msg::Volume(1))
//! match ev.message::<MsgIn>() {
//!     Some(MsgIn::Pan { drag }) => self.pan(drag.delta),
//!     Some(MsgIn::Rename { id, change }) => self.rename(id, change.text),
//!     Some(MsgIn::Volume(ch, change)) => self.volume(ch, change.value),
//!     Some(MsgIn::Save) => self.save(),
//!     None => {}
//! }
//! ```
//!
//! The twin has the original's variants, kinds and fields; a marked one
//! gains a field named for the event (a unit variant becomes one with
//! that field, a tuple variant gains it last), holding the event's data
//! type from kui's `event` module (what `ev.read()`'s variant holds, with
//! a struct of its own for `key`, `contextmenu`, `forceclick` and `files`):
//!
//! - `on = <event>` is the event's wire name: `click`, `drag`, `key`,
//!   `text`, `preedit`, `select`, `selectionrange`, `contextmenu`, `menu`,
//!   `forceclick`, `button`, `scroll`, `focus`, `hover`, `drop`, `files`,
//!   `layout`, `boundary`, `sound`, `dismiss`, and a window's own (ADR
//!   0048) `open`, `resize`, `window`, `system`, `modifiers`. The field is
//!   that name and its type `event::Click`, `Drag`, `KeyEvent`, `Text`,
//!   `Preedit`, `Select`, `SelectionRange`, `ContextMenu`, `MenuChoice`,
//!   `ForceClick`, `ButtonEvent`, `Scroll`, `Focus`, `Hover`, `FileDrop`,
//!   `Files`, `Layout`, `Boundary`, `Sound`, `Dismiss`, `Open`, `Resize`,
//!   `WindowEvent`, `SystemEnv`, `KeyMods`.
//! - `change` is the field two events share. `on = change` (or
//!   `edit_change`) is an editor's, `change: EditChange` (`{text}`);
//!   `on = slider_change` a slider's, `change: SliderChange` (`{value,
//!   phase}`). The twin's field is `change` either way, as the payload's.
//! - A `submit`, a `cancel`, a `fonts` and a `reply` carry no data, and a
//!   variant declared on them needs no `on`: it is refused, as is a name
//!   that is no event.
//! - `click` is for a node that can say more than that it was clicked (a
//!   `cells` grid, a sink's `line` rows): a plain button's click carries
//!   no `click` field, and its message needs no `on`. Nor does a grid's
//!   click from Enter, Space or a screen reader, which has no point to
//!   say: its twin does not decode, and `ev.message::<Msg>()` reads it.
//!
//! A message declared on the wrong handler — `Msg::Pan` on `on_click` —
//! arrives without its field and does not decode as the twin (its
//! `MessageError` names the missing field), while `ev.message::<Msg>()`
//! still reads it. One `on` per variant: `on_key` delivers `key`, `text`,
//! `preedit` and `select` alike, and a twin variant holds one of them, so
//! a sink that hears several reads them with `ev.split::<Msg>()`.
//!
//! The twin is `<Type>In` unless the type says `#[message(incoming =
//! "...")]`, is as visible as the original, and derives `Clone`, `Debug`
//! and `PartialEq`, which every event data type has; a message whose own
//! fields lack one names the derives it wants, `#[message(incoming_derive(Debug))]`.
//! It gets the same conversions as the original, so it also writes back
//! to the payload the core sent. `on` needs a map message (not a `string`
//! one) and does not go on the enum itself; on a struct it goes on the
//! struct, and makes `<Struct>In`.
//!
//! The book: <https://kui-book.qxuken.dev>. Repository:
//! <https://github.com/qxuken/kui>.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, LitStr, parse_macro_input, spanned::Spanned};

/// Derives `From<Self> for Value`, `TryFrom<&Value>`, `TryFrom<Value>` and
/// `MessageField` for an enum or a struct, and with `#[message(on = …)]`
/// the arriving twin; see the [crate docs](crate).
#[proc_macro_derive(Message, attributes(message))]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// What `#[message(…)]` said on one item.
#[derive(Default)]
struct Attrs {
    kind: Option<String>,
    krate: Option<syn::Path>,
    string: bool,
    /// `on = drag`: the event whose data the twin's variant carries.
    on: Option<Ident>,
    /// `incoming = "…"`: the twin's name.
    incoming: Option<Ident>,
    /// `incoming_derive(…)`: the twin's derives, instead of the default.
    incoming_derive: Option<Vec<syn::Path>>,
}

fn attrs(list: &[syn::Attribute]) -> syn::Result<Attrs> {
    let mut out = Attrs::default();
    for a in list.iter().filter(|a| a.path().is_ident("message")) {
        a.parse_nested_meta(|m| {
            if m.path.is_ident("kind") {
                out.kind = Some(m.value()?.parse::<LitStr>()?.value());
            } else if m.path.is_ident("crate") {
                out.krate = Some(m.value()?.parse::<LitStr>()?.parse()?);
            } else if m.path.is_ident("string") {
                out.string = true;
            } else if m.path.is_ident("on") {
                if out.on.is_some() {
                    return Err(m.error("one `on` per variant: a variant carries one event"));
                }
                out.on = Some(m.value()?.parse::<Ident>()?);
            } else if m.path.is_ident("incoming") {
                let name = m.value()?.parse::<LitStr>()?;
                out.incoming = Some(name.parse::<Ident>()?);
            } else if m.path.is_ident("incoming_derive") {
                let mut paths = Vec::new();
                m.parse_nested_meta(|d| {
                    paths.push(d.path);
                    Ok(())
                })?;
                out.incoming_derive = Some(paths);
            } else {
                return Err(m.error(
                    "expected `kind = \"…\"`, `crate = \"…\"`, `string`, `on = <event>`, \
                     `incoming = \"…\"` or `incoming_derive(…)`",
                ));
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// `TabNew` → `tab_new`, `HTTPGet` → `http_get`.
fn snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower = i > 0 && !chars[i - 1].is_uppercase() && chars[i - 1] != '_';
            let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            let prev_upper = i > 0 && chars[i - 1].is_uppercase();
            if i > 0 && (prev_lower || (prev_upper && next_lower)) {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// What `on = <name>` may name: the event's field in the arriving message
/// and the type, in kui's `event` module, its data reads as. A name is the
/// event's wire name, except that `change` is two events' field: an
/// editor's (`change`, or `edit_change` to say so) and a slider's
/// (`slider_change`).
const EVENTS: &[(&str, &str, &str)] = &[
    ("click", "click", "Click"),
    ("drag", "drag", "Drag"),
    ("key", "key", "KeyEvent"),
    ("text", "text", "Text"),
    ("preedit", "preedit", "Preedit"),
    ("select", "select", "Select"),
    ("selectionrange", "selectionrange", "SelectionRange"),
    ("contextmenu", "contextmenu", "ContextMenu"),
    ("menu", "menu", "MenuChoice"),
    ("forceclick", "forceclick", "ForceClick"),
    ("button", "button", "ButtonEvent"),
    ("scroll", "scroll", "Scroll"),
    ("focus", "focus", "Focus"),
    ("hover", "hover", "Hover"),
    ("drop", "drop", "FileDrop"),
    ("files", "files", "Files"),
    ("layout", "layout", "Layout"),
    ("change", "change", "EditChange"),
    ("edit_change", "change", "EditChange"),
    ("slider_change", "change", "SliderChange"),
    ("boundary", "boundary", "Boundary"),
    ("sound", "sound", "Sound"),
    ("capture", "capture", "Captured"),
    ("dismiss", "dismiss", "Dismiss"),
    ("open", "open", "Open"),
    ("resize", "resize", "Resize"),
    ("window", "window", "WindowEvent"),
    ("system", "system", "SystemEnv"),
    ("modifiers", "modifiers", "KeyMods"),
];

/// The field and the data type `on = <name>` names, or why it names none.
fn event_of(on: &Ident) -> syn::Result<(&'static str, &'static str)> {
    let name = on.to_string();
    if let Some((_, field, ty)) = EVENTS.iter().find(|(n, _, _)| *n == name) {
        return Ok((field, ty));
    }
    let why = match name.as_str() {
        "submit" | "cancel" | "reply" | "fonts" => format!(
            "a `{name}` carries no data: its message arrives as declared, so the variant \
             needs no `on`"
        ),
        _ => {
            let names: Vec<&str> = EVENTS.iter().map(|(n, _, _)| *n).collect();
            format!(
                "no event is named `{name}`; `on` takes one of {}",
                names.join(", ")
            )
        }
    };
    Err(syn::Error::new(on.span(), why))
}

/// One field of a shape: the payload key it is read from (an expression
/// of type `&'static str`), its binding, its type.
struct Field {
    key: Tokens,
    bind: Ident,
    ty: Tokens,
}

/// One shape a message takes: its kind, the constructor path, and its
/// fields.
struct Shape {
    kind: String,
    path: Tokens,
    fields: Vec<Field>,
    named: Option<bool>,
}

fn shape(kind: String, path: Tokens, fields: &Fields) -> Shape {
    let (named, fields) = match fields {
        Fields::Unit => (None, Vec::new()),
        Fields::Named(f) => (
            Some(true),
            f.named
                .iter()
                .map(|f| {
                    let id = f.ident.clone().expect("named");
                    let key = id.to_string();
                    let ty = &f.ty;
                    Field {
                        key: quote!(#key),
                        bind: id,
                        ty: quote!(#ty),
                    }
                })
                .collect(),
        ),
        Fields::Unnamed(f) => (
            Some(false),
            f.unnamed
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let key = i.to_string();
                    let ty = &f.ty;
                    Field {
                        key: quote!(#key),
                        bind: format_ident!("f{i}"),
                        ty: quote!(#ty),
                    }
                })
                .collect(),
        ),
    };
    Shape {
        kind,
        path,
        fields,
        named,
    }
}

impl Shape {
    /// The pattern binding every field, for the encoding side.
    fn pattern(&self) -> Tokens {
        let path = &self.path;
        let binds = self.fields.iter().map(|f| &f.bind);
        match self.named {
            None => quote!(#path),
            Some(true) => quote!(#path { #(#binds),* }),
            Some(false) => quote!(#path ( #(#binds),* )),
        }
    }

    /// The map this shape encodes to.
    fn encode(&self, k: &Tokens) -> Tokens {
        let kind = &self.kind;
        let entries = self
            .fields
            .iter()
            .map(|Field { key, bind, .. }| quote!((#key, #k::MessageField::to_value(#bind))));
        quote!(#k::Value::map([("kind", #k::Value::str(#kind)), #(#entries),*]))
    }

    /// The constructor reading every field from the map `v`.
    fn decode(&self, k: &Tokens) -> Tokens {
        let path = &self.path;
        let kind = &self.kind;
        let reads = self
            .fields
            .iter()
            .map(|Field { key, ty, .. }| quote!(#k::message::field::<#ty>(v, #kind, #key)?));
        match self.named {
            None => quote!(#path),
            Some(true) => {
                let names = self.fields.iter().map(|f| &f.bind);
                quote!(#path { #(#names: #reads),* })
            }
            Some(false) => quote!(#path ( #(#reads),* )),
        }
    }
}

/// An item's twin as it arrives (`#[message(on = …)]`): the declaration's
/// body and the shape, for one variant or the struct.
struct Arriving {
    /// `{ a: A, drag: Drag }`, `(A, Drag)`, or the original's own body.
    body: Tokens,
    shape: Shape,
}

/// The twin of one variant or struct whose original is `fields` and
/// `shape` (at `path`, the twin's constructor): its fields, plus the
/// event's data under its field when it says `on`. `vis` is what the
/// added field of a struct is visible as.
fn arriving(
    k: &Tokens,
    on: Option<&Ident>,
    fields: &Fields,
    mut shape: Shape,
    vis: Option<&syn::Visibility>,
) -> syn::Result<Arriving> {
    let Some(on) = on else {
        let body = match fields {
            Fields::Unit => quote!(),
            Fields::Named(f) => quote!(#f),
            Fields::Unnamed(f) => quote!(#f),
        };
        return Ok(Arriving { body, shape });
    };
    let (field, ty) = event_of(on)?;
    if shape.named == Some(true) && shape.fields.iter().any(|f| f.bind == field) {
        return Err(syn::Error::new(
            on.span(),
            format!(
                "this already has a field `{field}`, which the event's data arrives under: \
                 rename the field"
            ),
        ));
    }
    let ty_ident = format_ident!("{}", ty);
    let ty: Tokens = quote!(#k::event::#ty_ident);
    let key = quote!(<#ty as #k::message::EventField>::FIELD);
    let bind = format_ident!("{}", field, span = on.span());
    let vis = vis.map(|v| quote!(#v));
    let body = match fields {
        Fields::Unit => quote!({ #vis #bind: #ty }),
        Fields::Named(f) => {
            let own = f.named.iter();
            quote!({ #(#own,)* #vis #bind: #ty })
        }
        Fields::Unnamed(f) => {
            let own = f.unnamed.iter();
            quote!(( #(#own,)* #vis #ty ))
        }
    };
    let bind = match fields {
        Fields::Unnamed(f) => format_ident!("f{}", f.unnamed.len()),
        _ => bind,
    };
    if shape.named.is_none() {
        shape.named = Some(true);
    }
    shape.fields.push(Field {
        key,
        bind,
        ty: ty.clone(),
    });
    Ok(Arriving { body, shape })
}

/// The `kind`s of `shapes` are all different.
fn distinct(shapes: &[Shape]) -> syn::Result<()> {
    let mut seen = std::collections::HashSet::new();
    for s in shapes {
        if !seen.insert(s.kind.as_str()) {
            return Err(syn::Error::new(
                Span::call_site(),
                format!("two shapes of this message are both of kind {:?}", s.kind),
            ));
        }
    }
    Ok(())
}

/// The conversions for the type `name` (the original or its twin) whose
/// shapes are `shapes`.
fn impls(
    k: &Tokens,
    name: &Ident,
    generics: &syn::Generics,
    shapes: &[Shape],
    string: bool,
) -> Tokens {
    let (imp, ty, wh) = generics.split_for_impl();
    let (encode, decode) = if string {
        let pats = shapes.iter().map(Shape::pattern);
        let kinds: Vec<&String> = shapes.iter().map(|s| &s.kind).collect();
        let paths = shapes.iter().map(|s| &s.path);
        (
            quote!(match m { #(#pats => #k::Value::str(#kinds)),* }),
            quote!(match v.as_str().ok_or(#k::MessageError::NoKind)? {
                #(#kinds => Ok(#paths),)*
                other => Err(#k::MessageError::UnknownKind(other.to_string())),
            }),
        )
    } else {
        let pats = shapes.iter().map(Shape::pattern);
        let encodes = shapes.iter().map(|s| s.encode(k));
        let kinds = shapes.iter().map(|s| &s.kind);
        let decodes = shapes.iter().map(|s| s.decode(k));
        (
            quote!(match m { #(#pats => #encodes),* }),
            quote!(match #k::message::kind_of(v)? {
                #(#kinds => Ok(#decodes),)*
                other => Err(#k::MessageError::UnknownKind(other.to_string())),
            }),
        )
    };
    quote! {
        impl #imp ::core::convert::From<#name #ty> for #k::Value #wh {
            fn from(m: #name #ty) -> #k::Value {
                #encode
            }
        }

        impl #imp ::core::convert::TryFrom<&#k::Value> for #name #ty #wh {
            type Error = #k::MessageError;
            fn try_from(v: &#k::Value) -> ::core::result::Result<Self, #k::MessageError> {
                #decode
            }
        }

        impl #imp ::core::convert::TryFrom<#k::Value> for #name #ty #wh {
            type Error = #k::MessageError;
            fn try_from(v: #k::Value) -> ::core::result::Result<Self, #k::MessageError> {
                <Self as ::core::convert::TryFrom<&#k::Value>>::try_from(&v)
            }
        }

        impl #imp #k::MessageField for #name #ty #wh {
            fn to_value(self) -> #k::Value {
                self.into()
            }
            fn from_value(v: &#k::Value) -> ::core::option::Option<Self> {
                <Self as ::core::convert::TryFrom<&#k::Value>>::try_from(v).ok()
            }
        }
    }
}

fn expand(input: &DeriveInput) -> syn::Result<Tokens> {
    let top = attrs(&input.attrs)?;
    let k: Tokens = match &top.krate {
        Some(p) => quote!(#p),
        None => quote!(::kui_native),
    };
    let name = &input.ident;
    let twin_name = top
        .incoming
        .clone()
        .unwrap_or_else(|| format_ident!("{}In", name));

    // The original's shapes, and the twin's beside them; the twin is
    // emitted only when something says `on`.
    let mut shapes = Vec::new();
    let mut twins = Vec::new();
    let mut any_on = false;
    match &input.data {
        Data::Enum(e) => {
            if top.kind.is_some() {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "`kind` on an enum goes on each variant",
                ));
            }
            if let Some(on) = &top.on {
                return Err(syn::Error::new(
                    on.span(),
                    "`on` on an enum goes on each variant whose message an event brings",
                ));
            }
            for v in &e.variants {
                let a = attrs(&v.attrs)?;
                if a.krate.is_some()
                    || a.string
                    || a.incoming.is_some()
                    || a.incoming_derive.is_some()
                {
                    return Err(syn::Error::new(
                        v.span(),
                        "`crate`, `string`, `incoming` and `incoming_derive` go on the type, \
                         not on a variant",
                    ));
                }
                if let (Some(on), true) = (&a.on, top.string) {
                    return Err(syn::Error::new(
                        on.span(),
                        "`on` needs a map message, and a `string` message is a bare string",
                    ));
                }
                let id = &v.ident;
                let kind = a.kind.unwrap_or_else(|| snake(&id.to_string()));
                shapes.push(shape(kind.clone(), quote!(#name::#id), &v.fields));
                any_on |= a.on.is_some();
                let twin = arriving(
                    &k,
                    a.on.as_ref(),
                    &v.fields,
                    shape(kind, quote!(#twin_name::#id), &v.fields),
                    None,
                )?;
                let docs = v.attrs.iter().filter(|a| a.path().is_ident("doc"));
                let body = &twin.body;
                twins.push((quote!(#(#docs)* #id #body), twin.shape));
            }
        }
        Data::Struct(s) => {
            let kind = top.kind.clone().unwrap_or_else(|| snake(&name.to_string()));
            shapes.push(shape(kind.clone(), quote!(#name), &s.fields));
            any_on = top.on.is_some();
            let twin = arriving(
                &k,
                top.on.as_ref(),
                &s.fields,
                shape(kind, quote!(#twin_name), &s.fields),
                Some(&input.vis),
            )?;
            twins.push((twin.body, twin.shape));
        }
        Data::Union(u) => {
            return Err(syn::Error::new(
                u.union_token.span,
                "a message is an enum or a struct",
            ));
        }
    }
    distinct(&shapes)?;
    if top.string
        && (shapes.iter().any(|s| s.named.is_some()) || !matches!(input.data, Data::Enum(_)))
    {
        return Err(syn::Error::new(
            Span::call_site(),
            "`string` is for an enum whose variants are all units",
        ));
    }
    let original = impls(&k, name, &input.generics, &shapes, top.string);
    if !any_on {
        if top.incoming.is_some() || top.incoming_derive.is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                "`incoming` names the twin `on` makes, and nothing here says `on`",
            ));
        }
        return Ok(original);
    }

    // The twin: the same shapes, an annotated one with the event's field.
    let (bodies, twin_shapes): (Vec<Tokens>, Vec<Shape>) = twins.into_iter().unzip();
    let derives = match &top.incoming_derive {
        Some(paths) => quote!(#(#paths),*),
        None => quote!(
            ::core::clone::Clone,
            ::core::fmt::Debug,
            ::core::cmp::PartialEq
        ),
    };
    let doc = format!(
        " [`{name}`] as an event brings it: each variant marked `#[message(on = …)]` \
         carries the event's data in a field named for the event. Read it with \
         `ev.message::<{twin_name}>()`; a message declared on another handler has no \
         such field and does not decode. Generated by `#[derive(Message)]`."
    );
    let vis = &input.vis;
    let generics = &input.generics;
    let wh = &generics.where_clause;
    let decl = match &input.data {
        Data::Enum(_) => quote! {
            #[doc = #doc]
            #[derive(#derives)]
            #vis enum #twin_name #generics #wh { #(#bodies),* }
        },
        _ => {
            let body = &bodies[0];
            match twin_shapes[0].named {
                Some(false) => quote! {
                    #[doc = #doc]
                    #[derive(#derives)]
                    #vis struct #twin_name #generics #body #wh;
                },
                _ => quote! {
                    #[doc = #doc]
                    #[derive(#derives)]
                    #vis struct #twin_name #generics #wh #body
                },
            }
        }
    };
    let twin = impls(&k, &twin_name, generics, &twin_shapes, false);
    Ok(quote! {
        #original
        #decl
        #twin
    })
}

#[cfg(test)]
mod tests {
    use super::{EVENTS, expand, snake};

    #[test]
    fn variant_names_become_snake_case_kinds() {
        assert_eq!(snake("Inc"), "inc");
        assert_eq!(snake("TabNew"), "tab_new");
        assert_eq!(snake("PaneDrag"), "pane_drag");
        assert_eq!(snake("HTTPGet"), "http_get");
        assert_eq!(snake("Add10"), "add10");
        assert_eq!(snake("H"), "h");
    }

    /// What the derive refuses, and the words it refuses it with.
    fn refusal(input: syn::DeriveInput) -> String {
        match expand(&input) {
            Ok(t) => panic!("expanded: {t}"),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn on_names_an_event_with_data_and_a_message() {
        let unknown = refusal(syn::parse_quote! {
            enum Msg { #[message(on = dragg)] Pan }
        });
        assert!(unknown.contains("no event is named `dragg`"), "{unknown}");
        assert!(unknown.contains("slider_change"), "{unknown}");
        let submit = refusal(syn::parse_quote! {
            enum Msg { #[message(on = submit)] Save }
        });
        assert!(submit.contains("arrives as declared"), "{submit}");
        let fonts = refusal(syn::parse_quote! {
            enum Msg { #[message(on = fonts)] Fonts }
        });
        assert!(fonts.contains("arrives as declared"), "{fonts}");
    }

    #[test]
    fn on_and_incoming_go_where_they_mean_something() {
        let taken = refusal(syn::parse_quote! {
            enum Msg { #[message(on = drag)] Pan { drag: u32 } }
        });
        assert!(taken.contains("already has a field `drag`"), "{taken}");
        let twice = refusal(syn::parse_quote! {
            enum Msg { #[message(on = drag, on = hover)] Pan }
        });
        assert!(twice.contains("one `on` per variant"), "{twice}");
        let on_enum = refusal(syn::parse_quote! {
            #[message(on = drag)]
            enum Msg { Pan }
        });
        assert!(on_enum.contains("goes on each variant"), "{on_enum}");
        let string = refusal(syn::parse_quote! {
            #[message(string)]
            enum Dir { #[message(on = drag)] H, V }
        });
        assert!(string.contains("needs a map message"), "{string}");
        let lonely = refusal(syn::parse_quote! {
            #[message(incoming = "Arrived")]
            enum Msg { Pan }
        });
        assert!(lonely.contains("nothing here says `on`"), "{lonely}");
        let on_variant = refusal(syn::parse_quote! {
            enum Msg { #[message(incoming = "X")] Pan }
        });
        assert!(on_variant.contains("go on the type"), "{on_variant}");
    }

    /// The twin is emitted only when a variant says `on`, under `<Type>In`
    /// or the name `incoming` gives, and its annotated variant gains the
    /// field.
    #[test]
    fn the_twin_is_named_and_carries_the_field() {
        let plain = expand(&syn::parse_quote! { enum Msg { Pan } })
            .unwrap()
            .to_string();
        assert!(!plain.contains("MsgIn"), "{plain}");
        let twin = expand(&syn::parse_quote! {
            enum Msg { #[message(on = slider_change)] Vol { ch: u8 }, Save }
        })
        .unwrap()
        .to_string();
        assert!(twin.contains("enum MsgIn"), "{twin}");
        assert!(
            twin.contains("change : :: kui_native :: event :: SliderChange"),
            "{twin}"
        );
        let renamed = expand(&syn::parse_quote! {
            #[message(incoming = "Arrived", crate = "kui_core")]
            enum Msg { #[message(on = change)] Rename }
        })
        .unwrap()
        .to_string();
        assert!(renamed.contains("enum Arrived"), "{renamed}");
        assert!(!renamed.contains("MsgIn"), "{renamed}");
        assert!(
            renamed.contains("change : kui_core :: event :: EditChange"),
            "{renamed}"
        );
    }

    /// Every name `on` takes is a field the core writes: its own wire name,
    /// or `change` for the two spellings that say whose change.
    #[test]
    fn every_on_name_is_an_event_field() {
        for (name, field, _) in EVENTS {
            assert!(name == field || *field == "change", "{name} → {field}");
        }
    }
}
