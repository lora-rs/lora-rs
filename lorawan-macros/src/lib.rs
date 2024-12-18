use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Meta, PathArguments};

struct Payload {
    name: Ident,
    lifetime: Option<syn::Lifetime>,
}

impl Payload {
    fn new(name: Ident, lifetime: Option<syn::Lifetime>) -> Self {
        Self { name, lifetime }
    }
}

struct Attributes {
    doc: Vec<syn::Attribute>,
    attrs: Option<(syn::Expr, syn::Expr)>,
}

#[proc_macro_derive(CommandHandler, attributes(ack, cmd))]
pub fn derive_command_handler(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let handler = &input.ident;
    let (handler_lt, _ty, _where) = input.generics.split_for_impl();
    // Collect lifetime requirements for MacCommandIterator<'_, T<..>>
    let handler_lifetimes =
        input.generics.lifetimes().map(|lt| lt.lifetime.clone()).collect::<Vec<_>>();
    let anon_lifetime = syn::Lifetime::new("'_", Span::call_site());
    let iter_lifetime = match handler_lifetimes.len() {
        1 => handler_lifetimes.first(),
        0 => Some(&anon_lifetime),
        _ => panic!("Multiple lifetimes for this enum are unsupported!"),
    };

    let creator_enum = Ident::new(&format!("{}Creator", handler), Span::call_site());

    // Parse enum members into list of (Command, Payload, Attributes) tuples
    let members = parse_enum_members(&input);

    let mut impl_len = Vec::new();
    let mut impl_bytes = Vec::new();
    let mut impl_cid = Vec::new();
    let mut impl_iter_next = Vec::new();

    let mut payload_struct_impls = Vec::new();

    let mut payload_struct_creator_impls = Vec::new();

    let mut creator_enum_variants = Vec::new();
    let mut creator_enum_len = Vec::new();
    let mut creator_enum_build = Vec::new();
    let mut creator_enum_impl_cid = Vec::new();

    for (n, payload, attributes) in members {
        let n = n.clone();
        let t = &payload.name;
        let lt = &payload.lifetime;
        let doc = &attributes.doc;
        let creator = Ident::new(&format!("{}Creator", n), Span::call_site());

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
                Some(#handler::#n(#t::new_from_raw(&data[1..1 + #t::len()])))
            } else
        });

        // CommandCreator enum
        creator_enum_variants.push(quote! {
            #n(#creator)
        });

        creator_enum_len.push(quote! {
            Self::#n(c) => c.len()
        });
        creator_enum_build.push(quote! {
            Self::#n(ref c) => c.build()
        });
        creator_enum_impl_cid.push(quote! {
            Self::#n(c) => c.cid()
        });

        // Generate definition and common implementation for payloads
        if let Some((cid, len)) = attributes.attrs {
            let payload_creator = Ident::new(&format!("{}Creator", n), Span::call_site());

            // Generate common code used by zero and non-zero length payloads
            let common = quote! {
                /// Payload CID.
                pub const fn cid() -> u8 {
                    #cid
                }
                /// Length of payload without the CID.
                pub const fn len() -> usize {
                    #len
                }
            };

            // Payloads with len > 0 (which have lifetime)
            if let Some(lt) = lt {
                payload_struct_impls.push(quote! {
                    #[derive(Debug, PartialEq, Eq)]
                    #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
                    #( #doc )*
                    pub struct #t <#lt> (pub(crate) &#lt [u8]);

                    impl<#lt> #t<#lt> {
                        /// Creates a new instance of the MAC command if there is enough data.
                        pub fn new(data: &#lt [u8]) -> Result<#t<#lt>, Error> {
                            if data.len() != #len {
                                Err(Error::BufferTooShort)
                            } else {
                                Ok(#t(&data))
                            }
                        }
                        /// Constructs a new instance of the MAC command from the provided data,
                        /// without verifying the data length.
                        ///
                        /// Improper use of this method could lead to panic during runtime!
                        pub fn new_from_raw(data: &#lt [u8]) ->#t<#lt> {
                            #t(&data)
                        }

                        #common

                        /// Reference to the payload.
                        pub fn bytes (&self) -> &[u8]{
                            self.0
                        }
                    }
                });
            } else {
                // Handle zero-length payloads
                payload_struct_impls.push(quote! {
                    #[derive(Debug, PartialEq, Eq)]
                    #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
                    #( #doc )*
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

                        #common

                        /// Reference to the empty payload.
                        pub fn bytes(&self) -> &[u8] {
                            &[]
                        }
                    }
                });
            }

            payload_struct_creator_impls.push(quote! {
                #[derive(Debug)]
                #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
                #[doc(hidden)]
                pub struct #payload_creator {
                    pub(crate) data: [u8; #len + 1],
                }

                impl #payload_creator {
                    pub fn new() -> Self {
                        let mut data = [0; #len + 1];
                        data[0] = #cid;
                        Self { data }
                    }

                    pub fn build(&self) -> &[u8] {
                     &self.data[..]
                    }

                    /// Get the CID.
                    pub const fn cid(&self) -> u8 {
                        #cid
                    }

                    /// Get the length.
                    #[allow(clippy::len_without_is_empty)]
                    pub const fn len(&self) -> usize {
                        #len + 1
                    }
                }

                impl SerializableMacCommand for #payload_creator {
                    fn payload_bytes(&self) -> &[u8] {
                        &self.build()[1..]
                    }

                    /// The CID of the SerializableMacCommand.
                    fn cid(&self) -> u8 {
                        self.build()[0]
                    }

                    /// Length of the SerializableMacCommand not including CID.
                    fn payload_len(&self) -> usize {
                        self.build().len() - 1
                    }
                }
            });
        }
    }

    quote! {
        #[allow(clippy::len_without_is_empty)]
        impl #handler_lt #handler #handler_lt {
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

        impl #handler_lt SerializableMacCommand for #handler #handler_lt {
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

        impl #handler_lt Iterator for MacCommandIterator<#iter_lifetime, #handler #handler_lt > {
            type Item = #handler #handler_lt;

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

        #( #payload_struct_creator_impls )*

        #[derive(Debug)]
        #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
        pub enum #creator_enum {
            #( #creator_enum_variants ),*
        }

        impl #creator_enum {
            /// Get the length.
            #[allow(clippy::len_without_is_empty)]
            pub fn len(&self) -> usize {
                match self {
                    # ( #creator_enum_len ),*
                }
            }

            /// Build.
            pub fn build(&self) -> &[u8] {
                match *self {
                    #( #creator_enum_build ),*
                }
            }
        }

        impl SerializableMacCommand for #creator_enum {
            fn payload_bytes(&self) -> &[u8] {
                &self.build()[1..]
            }

            fn cid(&self) -> u8 {
                match self {
                    #( #creator_enum_impl_cid ),*
                }
            }

            fn payload_len(&self) -> usize {
                self.len() - 1
            }
        }
    }
    .into()
}

// Parse enum variant argument into `[Payload]` objects
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
                            Payload::new(var.clone(), Some(lt.clone()))
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

/// Handler for `#[cmd(cid = ..., len = ...)]` attribute
fn attr_handle_cmd(attr: &syn::Attribute) -> Option<(syn::Expr, syn::Expr)> {
    // TODO: Figure out how to convert cid/len values to u8
    // TODO: Raise errors on missing cid/len as these are required?
    let mut cid = None;
    let mut len = None;
    if let Ok(nested) =
        attr.parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
    {
        for meta in nested {
            match meta {
                Meta::Path(_) => unimplemented!("Meta::Path is not supported!"),
                Meta::List(_) => unimplemented!("Meta::List is not supported!"),
                // We'll only expect NameValues (ie. val=xx)
                Meta::NameValue(v) => {
                    if let Some(id) = v.path.get_ident() {
                        match id.to_string().as_str() {
                            "cid" => {
                                cid = Some(v.value);
                            }
                            "len" => {
                                len = Some(v.value);
                            }
                            &_ => {
                                panic!("Invalid argument: {}", &id);
                            }
                        }
                    } else {
                        panic!("Missing ident?");
                    }
                }
            }
        }
    }
    if let (Some(cid), Some(len)) = (cid, len) {
        Some((cid, len))
    } else {
        None
    }
}

/// Collect supported attributes for enum members into [`Attributes`]:
/// * docstring
/// * `cmd(..)` - used to specify size and CID for payload
fn parse_variant_attrs(input: &Vec<syn::Attribute>) -> Attributes {
    let mut doc = Vec::new();
    let mut attrs: Option<(syn::Expr, syn::Expr)> = None;
    for attr in input {
        if attr.path().is_ident("cmd") {
            attrs = attr_handle_cmd(attr);
            continue;
        }
        // Docstrings
        if attr.path().is_ident("doc") {
            doc.push(attr.clone());
            continue;
        }
    }
    Attributes { doc, attrs }
}

// Parse enum variant into list of (Variant, Payload, Attributes) tuples.
// For example:
// ```
// enum Foo<'a> {
//   /// Field documentation.
//   #[cmd(cid=0x1, len=1)]
//   Var(A),        # (VarA, Payload { name: A, lifetime: None }, Attributes)
//   /// Field documentation...
//   /// ...continued.
//   #[cmd(cid=0x2, len=5)]
//   VarB(B<'a>),   # (VarB, Payload { name: B, lifetime: Some(a) }, Attributes)
// }
// ```
fn parse_enum_members(input: &DeriveInput) -> Vec<(Ident, Payload, Attributes)> {
    let mut items = vec![];
    match input.data {
        Data::Enum(ref item) => {
            for elem in item.variants.clone() {
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
    items
}
