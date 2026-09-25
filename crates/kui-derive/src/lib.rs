//! `#[derive(Message)]` (backlog C50): an enum or a struct to and from the
//! `{kind, …fields}` map a kui payload is, so a Rust app writes
//! `on_click(Msg::Save)` and matches `ev.message::<Msg>()` where it would
//! otherwise build and pick apart `Value`s by string.
//!
//! What it generates, for a type `Msg`:
//!
//! - `From<Msg> for Value`: a map whose `kind` is the variant's name in
//!   snake_case (`TabNew` is `"tab_new"`, a struct's own name for a
//!   struct) and whose other keys are the fields — a tuple variant's by
//!   position, `"0"`, `"1"`, ….
//! - `TryFrom<&Value>` and `TryFrom<Value> for Msg`, with
//!   `kui_native::MessageError` saying what did not fit.
//! - `kui_native::MessageField for Msg`, so a message can be a field of another.
//!
//! Attributes, all under `#[message(…)]`:
//!
//! - on a variant or a struct, `kind = "…"` names its `kind` instead;
//! - on an enum of unit variants only, `string` makes it a bare string
//!   (`"h"`) rather than a map (`{kind: "h"}`) — the shape a field such as
//!   a split's direction reads best as;
//! - on the type, `crate = "…"` names the path the generated code reaches
//!   kui through: `::kui` unless said, `kui_core` for a crate that depends
//!   on kui-core alone.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input, spanned::Spanned};

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
            } else {
                return Err(m.error("expected `kind = \"…\"`, `crate = \"…\"` or `string`"));
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

/// One shape a message takes: its kind, the constructor path, and its
/// fields as (key, binding, type).
struct Shape {
    kind: String,
    path: Tokens,
    fields: Vec<(String, syn::Ident, syn::Type)>,
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
                    (id.to_string(), id, f.ty.clone())
                })
                .collect(),
        ),
        Fields::Unnamed(f) => (
            Some(false),
            f.unnamed
                .iter()
                .enumerate()
                .map(|(i, f)| (i.to_string(), format_ident!("f{i}"), f.ty.clone()))
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
        let binds = self.fields.iter().map(|(_, b, _)| b);
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
            .map(|(key, b, _)| quote!((#key, #k::MessageField::to_value(#b))));
        quote!(#k::Value::map([("kind", #k::Value::str(#kind)), #(#entries),*]))
    }

    /// The constructor reading every field from the map `v`.
    fn decode(&self, k: &Tokens) -> Tokens {
        let path = &self.path;
        let kind = &self.kind;
        let reads = self
            .fields
            .iter()
            .map(|(key, _, ty)| quote!(#k::message::field::<#ty>(v, #kind, #key)?));
        match self.named {
            None => quote!(#path),
            Some(true) => {
                let names = self.fields.iter().map(|(_, b, _)| b);
                quote!(#path { #(#names: #reads),* })
            }
            Some(false) => quote!(#path ( #(#reads),* )),
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
    let (imp, ty, wh) = input.generics.split_for_impl();

    let shapes: Vec<Shape> = match &input.data {
        Data::Enum(e) => {
            let mut out = Vec::new();
            for v in &e.variants {
                let a = attrs(&v.attrs)?;
                if a.krate.is_some() || a.string {
                    return Err(syn::Error::new(
                        v.span(),
                        "`crate` and `string` go on the type, not on a variant",
                    ));
                }
                let id = &v.ident;
                out.push(shape(
                    a.kind.unwrap_or_else(|| snake(&id.to_string())),
                    quote!(#name::#id),
                    &v.fields,
                ));
            }
            out
        }
        Data::Struct(s) => vec![shape(
            top.kind.clone().unwrap_or_else(|| snake(&name.to_string())),
            quote!(#name),
            &s.fields,
        )],
        Data::Union(u) => {
            return Err(syn::Error::new(
                u.union_token.span,
                "a message is an enum or a struct",
            ));
        }
    };
    if top.kind.is_some() && matches!(input.data, Data::Enum(_)) {
        return Err(syn::Error::new(
            Span::call_site(),
            "`kind` on an enum goes on each variant",
        ));
    }
    {
        let mut seen = std::collections::HashSet::new();
        for s in &shapes {
            if !seen.insert(s.kind.as_str()) {
                return Err(syn::Error::new(
                    Span::call_site(),
                    format!("two shapes of this message are both of kind {:?}", s.kind),
                ));
            }
        }
    }

    let (encode, decode) = if top.string {
        if shapes.iter().any(|s| s.named.is_some()) || !matches!(input.data, Data::Enum(_)) {
            return Err(syn::Error::new(
                Span::call_site(),
                "`string` is for an enum whose variants are all units",
            ));
        }
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
        let encodes = shapes.iter().map(|s| s.encode(&k));
        let kinds = shapes.iter().map(|s| &s.kind);
        let decodes = shapes.iter().map(|s| s.decode(&k));
        (
            quote!(match m { #(#pats => #encodes),* }),
            quote!(match #k::message::kind_of(v)? {
                #(#kinds => Ok(#decodes),)*
                other => Err(#k::MessageError::UnknownKind(other.to_string())),
            }),
        )
    };

    Ok(quote! {
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
    })
}

#[cfg(test)]
mod tests {
    use super::snake;

    #[test]
    fn variant_names_become_snake_case_kinds() {
        assert_eq!(snake("Inc"), "inc");
        assert_eq!(snake("TabNew"), "tab_new");
        assert_eq!(snake("PaneDrag"), "pane_drag");
        assert_eq!(snake("HTTPGet"), "http_get");
        assert_eq!(snake("Add10"), "add10");
        assert_eq!(snake("H"), "h");
    }
}
