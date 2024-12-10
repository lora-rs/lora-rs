use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Meta, PathArguments};

struct Payload {
    name: Ident,
    lifetime: Option<Ident>,
}

impl Payload {
    fn new(name: Ident, lifetime: Option<Ident>) -> Self {
        Self { name, lifetime }
    }
}

// TODO: Figure out how to parse value to literal (and handle value sanity checks)
struct CmdInfo {
    cid: Option<syn::Expr>,
    len: Option<syn::Expr>,
}

#[proc_macro_derive(CommandHandler, attributes(cmd))]
pub fn derive_command_handler(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;

    // Parse enum members into list of (Command, Payload) tuples
    let members = parse_enum_members(&input);

    let mut impl_len = Vec::new();
    let mut impl_bytes = Vec::new();
    let mut impl_cid = Vec::new();
    let mut impl_iter_next = Vec::new();

    let mut payload_struct_impls = Vec::new();

    for (n, payload, attributes) in members {
        let n = n.clone();
        let t = &payload.name;
        let _lt = &payload.lifetime;

        // ...len()
        impl_len.push(quote! {
            Self::#n(_) => #t::len()
        });

        // ...bytes()
        impl_bytes.push(quote! {
            Self::#n(ref v) => v.bytes()
        });

        // SerializableMacCommand::cid()
        impl_cid.push(quote! {
            Self::#n(_) => #t::cid()
        });

        // SerializableMacCommand::next()
        impl_iter_next.push(quote! {
            if data[0] == #t::cid() && data.len() >= #t::len() {
                self.index = self.index + #t::len() + 1;
                Some(#name::#n(#t::new_from_raw(&data[1..1 + #t::len()])))
            } else
        });

        // Generate PayLoad structs (for payloads with zero length)
        // TODO: Support for payloads where len != 0
        // TODO: Docstring handling (for structs) and manipulation (methods)
        if attributes.cid.is_some() && attributes.len.is_some() {
            let cid = attributes.cid.unwrap();
            let len = attributes.len.unwrap();

            payload_struct_impls.push(quote! {
                #[derive(Debug, PartialEq, Eq)]
                #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
                pub struct #t();

                impl #t {
                    /// Create new
                    pub fn new(_: &[u8]) -> #t {
                        #t()
                    }
                    /// Create from raw_bytes (for compatibility with non-zero length payloads)
                    pub fn new_from_raw(_: &[u8]) -> #t {
                        #t()
                    }
                    /// Payload CID
                    pub const fn cid() -> u8 {
                        #cid
                    }
                    /// Length of empty payload.
                    pub const fn len() -> usize {
                        #len
                    }
                    /// Reference to the empty payload.
                    pub fn bytes(&self) -> &[u8] {
                        &[]
                    }
                }
            });
        }
    }

    quote! {
        #[allow(clippy::len_without_is_empty)]
        impl<'a> #name<'a> {
            /// Get the length.
            pub fn len(&self) -> usize {
                match *self {
                    #( #impl_len, )*
                }
            }
            /// Get reference to the data.
            pub fn bytes(&self) -> &[u8] {
                match *self {
                    #( #impl_bytes, )*
                }
            }
        }

        impl<'a> SerializableMacCommand for #name<'a> {
            fn payload_bytes(&self) -> &[u8] {
                &self.bytes()
            }
            fn cid(&self) -> u8 {
                match *self {
                    #( #impl_cid, )*
                }
            }

            fn payload_len(&self) -> usize {
                self.len()
            }
        }

        impl<'a> Iterator for MacCommandIterator<'a, #name<'a>> {
            type Item = #name<'a>;

            fn next(&mut self) -> Option<Self::Item> {
                if self.index < self.data.len() {
                    let data = &self.data[self.index..];
                    #( #impl_iter_next )*
                    {
                        None
                    }
                } else {
                    None
                }
            }
        }

        #( #payload_struct_impls )*

    }
    .into()
}

// Parse enum variant arguments into `[Payload]` objects
// For example:
//
// ```
// enum Foo<'a> {
//   FieldA(A),     # Payload { name: A, lifetime: None }
//   FieldB(B<'a>), # Payload { name: B, lifetime: a }
// }
// ```
fn parse_variant_fields(input: &syn::Type) -> Payload {
    match input {
        syn::Type::Path(p) => {
            if p.path.segments.len() != 1 {
                unimplemented!();
            }
            let syn::PathSegment { ident: var, arguments } = &p.path.segments[0];
            match arguments {
                PathArguments::AngleBracketed(e) => {
                    if e.args.len() != 1 {
                        panic!("Only single argument is supported!");
                    }
                    match &e.args[0] {
                        syn::GenericArgument::Lifetime(lt) => {
                            Payload::new(var.clone(), Some(lt.ident.clone()))
                        }
                        _ => todo!("???"),
                    }
                }
                PathArguments::None => Payload::new(var.clone(), None),
                PathArguments::Parenthesized(_) => todo!("syn::PathArguments::None"),
            }
        }
        _ => unimplemented!(),
    }
}

// Parse `cid` and `len` values from `#[cmd(cid=cid, len=len)]` attribute into tuple
fn parse_variant_attrs(attrs: &Vec<syn::Attribute>) -> CmdInfo {
    let mut params = CmdInfo { cid: None, len: None };
    for attr in attrs {
        if !attr.path().is_ident("cmd") {
            continue;
        }
        // Parse arguments
        if let Ok(nested) = attr
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        {
            // We'll only expect NameValues (ie. val=xx)
            for meta in nested {
                match meta {
                    Meta::NameValue(v) => {
                        if let Some(id) = v.path.get_ident() {
                            match id.to_string().as_str() {
                                "cid" => {
                                    params.cid = Some(v.value);
                                }
                                "len" => {
                                    params.len = Some(v.value);
                                }
                                &_ => {
                                    eprintln!("Unhandled argument: {}", &id);
                                }
                            }
                        } else {
                            panic!("Missing ident?");
                        }
                    }
                    Meta::Path(_) => unimplemented!("Meta::Path is not supported!"),
                    Meta::List(_) => unimplemented!("Meta::List is not supported!"),
                }
            }
        }
    }
    params
}

// Parse enum variant into list of (Variant, Payload, CmdInfo) tuples.
// For example:
// ```
// enum Foo<'a> {
//   #[cmd(cid=0x1, len=1)]
//   FieldA(A),     # (FieldA, Payload { name: A, lifetime: None }, CmdInfo { cid: 0x1, len: 1})
//   #[cmd(cid=0x2, len=5)]
//   FieldB(B<'a>), # (FieldB, Payload { name: B, lifetime: Some(a) }, CmdInfo { cid: 0x2, len: 5})
// }
// ```
fn parse_enum_members(input: &DeriveInput) -> Vec<(Ident, Payload, CmdInfo)> {
    let mut items = vec![];
    match input.data {
        Data::Enum(ref item) => {
            for elem in item.variants.clone() {
                // eprintln!("{:#?}", elem);
                if elem.fields.len() != 1 {
                    panic!("Expecting single argument for {}", elem.ident)
                }
                let payload = match elem.fields {
                    Fields::Unnamed(f) => parse_variant_fields(&f.unnamed.get(0).unwrap().ty),
                    Fields::Named(_) | Fields::Unit => panic!("Unsupported!"),
                };
                items.push((elem.ident.clone(), payload, parse_variant_attrs(&elem.attrs)));
            }
        }
        _ => panic!("Unsupported!"),
    };
    // TODO: Sanity checks?
    // TODO: Warn about missing CmdInfo and its values...
    items
}
