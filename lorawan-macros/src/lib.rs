use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Fields, Meta, PathArguments, parse_macro_input};

struct Payload {
    name: Ident,
    lifetime: Option<syn::Lifetime>,
}

impl Payload {
    fn new(name: Ident, lifetime: Option<syn::Lifetime>) -> Self {
        Self { name, lifetime }
    }
}

enum CmdShape {
    ZeroLength,
    Fixed { len: u8, lifetime: syn::Lifetime },
    Variable { lifetime: syn::Lifetime },
}

struct Command {
    variant: Ident,
    payload: Ident,
    cid: u8,
    shape: CmdShape,
    doc: Vec<syn::Attribute>,
}

struct Attributes {
    doc: Vec<syn::Attribute>,
    cid: u8,
    len: Option<u8>,
}

fn slice_payload_definition(
    payload: &Ident,
    lifetime: &syn::Lifetime,
    cid: u8,
    doc: &[syn::Attribute],
) -> TokenStream {
    quote! {
        #[derive(Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
        #( #doc )*
        pub struct #payload <#lifetime> (pub(crate) &#lifetime [u8]);

        impl<#lifetime> #payload<#lifetime> {
            /// Payload CID.
            pub const fn cid() -> u8 {
                #cid
            }

            /// Constructs a new instance of the MAC command from the provided data,
            /// without verifying the data length.
            ///
            /// Improper use of this method could lead to panic during runtime!
            pub fn new_from_raw(data: &#lifetime [u8]) -> #payload<#lifetime> {
                #payload(data)
            }

            /// Reference to the payload.
            pub fn bytes(&self) -> &[u8] {
                self.0
            }
        }
    }
}

#[proc_macro_derive(CommandHandler, attributes(cmd))]
pub fn derive_command_handler(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_command_handler(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_command_handler(input: DeriveInput) -> syn::Result<TokenStream> {
    let handler = &input.ident;
    let (handler_lt, _ty, _where) = input.generics.split_for_impl();
    let handler_lifetimes =
        input.generics.lifetimes().map(|lt| lt.lifetime.clone()).collect::<Vec<_>>();
    if let Some(extra) = handler_lifetimes.get(1) {
        return Err(syn::Error::new_spanned(extra, "only one lifetime parameter is supported"));
    }

    let members = parse_enum_members(&input)?;
    let mut impl_len = Vec::new();
    let mut impl_bytes = Vec::new();
    let mut impl_cid = Vec::new();
    let mut impl_parse_one = Vec::new();
    let mut payload_struct_impls = Vec::new();
    let mut payload_struct_creator_impls = Vec::new();

    let creator_enum = Ident::new(&format!("{}Creator", handler), Span::call_site());
    let mut creator_enum_variants = Vec::new();
    let mut creator_enum_len = Vec::new();
    let mut creator_enum_build = Vec::new();
    let mut creator_enum_impl_cid = Vec::new();

    for command in members {
        let n = &command.variant;
        let t = &command.payload;
        let cid = command.cid;
        let doc = &command.doc;
        let creator = Ident::new(&format!("{}Creator", n), Span::call_site());

        // ...len()
        impl_len.push(quote! {
            Self::#n(ref v) => v.len()
        });

        // ...bytes()
        impl_bytes.push(quote! {
            Self::#n(ref v) => v.bytes()
        });

        // SerializableMacCommand::cid()
        impl_cid.push(quote! {
            Self::#n(_) => #t::cid()
        });

        // crate::maccommands::MacCommandSet::parse_one(): fallible framing.
        // Truncation and unknown CIDs are reported instead of silently
        // ending the stream, and payload len() helpers are never invoked on
        // an empty slice.
        impl_parse_one.push(match &command.shape {
            CmdShape::ZeroLength | CmdShape::Fixed { .. } => quote! {
                #cid => {
                    let len = #t::max_len();
                    if data.len() < 1 + len {
                        return Err(crate::maccommands::ParseError::Truncated { cid });
                    }
                    Ok((Self::#n(#t::new_from_raw(&data[1..1 + len])), 1 + len))
                }
            },
            CmdShape::Variable { .. } => quote! {
                #cid => {
                    let rest = &data[1..];
                    if rest.is_empty() {
                        return Err(crate::maccommands::ParseError::Truncated { cid });
                    }
                    let len = #t::new_from_raw(rest).len();
                    if rest.len() < len {
                        return Err(crate::maccommands::ParseError::Truncated { cid });
                    }
                    Ok((Self::#n(#t::new_from_raw(&rest[..len])), 1 + len))
                }
            },
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

        // Generate the payload definition and its shape-specific methods.
        match &command.shape {
            CmdShape::ZeroLength => {
                payload_struct_impls.push(quote! {
                    #[derive(Debug, PartialEq, Eq)]
                    #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
                    #( #doc )*
                    pub struct #t();

                    impl #t {
                        /// Payload CID.
                        pub const fn cid() -> u8 {
                            #cid
                        }

                        /// Reference to the payload.
                        pub fn bytes(&self) -> &[u8] {
                            &[]
                        }

                        /// Create new instance - zero-length payloads don't need validation
                        pub fn new(_: &[u8]) -> #t {
                            #t()
                        }

                        /// Create from raw_bytes (for compatibility with non-zero length payloads)
                        pub fn new_from_raw(_: &[u8]) -> #t {
                            #t()
                        }

                        /// Maximum length of payload without the CID.
                        pub const fn max_len() -> usize {
                            0
                        }

                        /// Actual length of payload without the CID.
                        #[allow(clippy::len_without_is_empty)]
                        pub fn len(&self) -> usize {
                            Self::max_len()
                        }
                    }
                });
            }
            CmdShape::Fixed { len, lifetime } => {
                payload_struct_impls.push(slice_payload_definition(t, lifetime, cid, doc));
                payload_struct_impls.push(quote! {
                    impl<#lifetime> #t<#lifetime> {
                        /// Creates a new instance of the MAC command if there is enough data.
                        pub fn new(data: &#lifetime [u8]) -> Result<#t<#lifetime>, Error> {
                            if data.len() != Self::max_len() {
                                Err(Error::BufferTooShort)
                            } else {
                                Ok(#t(data))
                            }
                        }

                        /// Maximum length of payload without the CID.
                        pub const fn max_len() -> usize {
                            #len as usize
                        }

                        /// Actual length of payload without the CID.
                        #[allow(clippy::len_without_is_empty)]
                        pub fn len(&self) -> usize {
                            Self::max_len()
                        }
                    }
                });
            }
            CmdShape::Variable { lifetime } => {
                payload_struct_impls.push(slice_payload_definition(t, lifetime, cid, doc));
            }
        }

        payload_struct_creator_impls.push(quote! {
            impl SerializableMacCommand for #creator {
                /// The CID of the SerializableMacCommand.
                fn cid(&self) -> u8 {
                    self.build()[0]
                }

                fn payload_bytes(&self) -> &[u8] {
                    &self.build()[1..]
                }

                /// Length of the SerializableMacCommand not including CID.
                fn payload_len(&self) -> usize {
                    self.build().len() - 1
                }
            }
        });

        if !matches!(&command.shape, CmdShape::Variable { .. }) {
            payload_struct_creator_impls.push(quote! {
                #[derive(Debug)]
                #[cfg_attr(feature = "defmt-03", derive(defmt::Format))]
                #[doc(hidden)]
                pub struct #creator {
                    pub(crate) data: [u8; #t::max_len() + 1],
                }

                impl #creator {
                    pub fn new() -> Self {
                        let mut data = [0; #t::max_len() + 1];
                        data[0] = #cid;
                        Self { data }
                    }

                    pub fn build(&self) -> &[u8] {
                        &self.data[..self.len()]
                    }

                    pub const fn cid(&self) -> u8 {
                        #cid
                    }

                    /// Get the length including CID.
                    #[allow(clippy::len_without_is_empty)]
                    pub fn len(&self) -> usize {
                        #t::max_len() + 1
                    }
                }
            });
        }
    }

    // The fallible-framing impl. All current command sets carry exactly
    // one lifetime; support lifetime-less enums for completeness.
    let parse_one_body = quote! {
        let cid = data[0];
        match cid {
            #( #impl_parse_one )*
            _ => Err(crate::maccommands::ParseError::UnknownCid(cid)),
        }
    };
    let impl_command_set = if let Some(lt) = handler_lifetimes.first() {
        quote! {
            impl<#lt> crate::maccommands::MacCommandSet<#lt> for #handler<#lt> {
                fn parse_one(data: &#lt [u8]) -> Result<(Self, usize), crate::maccommands::ParseError> {
                    #parse_one_body
                }
            }
        }
    } else {
        quote! {
            impl<'a> crate::maccommands::MacCommandSet<'a> for #handler {
                fn parse_one(data: &'a [u8]) -> Result<(Self, usize), crate::maccommands::ParseError> {
                    #parse_one_body
                }
            }
        }
    };

    // Generate the final implementations
    Ok(quote! {
        #impl_command_set

        impl #handler_lt #handler #handler_lt {
            /// Get the length.
            #[allow(clippy::len_without_is_empty)]
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
                    #( #creator_enum_len ),*
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
    })
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
fn parse_variant_fields(input: &syn::Type) -> syn::Result<Payload> {
    let syn::Type::Path(path) = input else {
        return Err(syn::Error::new_spanned(input, "expected a payload type"));
    };

    if path.path.segments.len() != 1 {
        return Err(syn::Error::new_spanned(&path.path, "expected a single payload type path"));
    }

    let segment = &path.path.segments[0];

    match &segment.arguments {
        PathArguments::AngleBracketed(args) => {
            if args.args.len() != 1 {
                return Err(syn::Error::new_spanned(
                    args,
                    "expected exactly one lifetime argument",
                ));
            }

            match &args.args[0] {
                syn::GenericArgument::Lifetime(lifetime) => {
                    Ok(Payload::new(segment.ident.clone(), Some(lifetime.clone())))
                }
                other => {
                    Err(syn::Error::new_spanned(other, "only a lifetime argument is supported"))
                }
            }
        }
        PathArguments::None => Ok(Payload::new(segment.ident.clone(), None)),
        PathArguments::Parenthesized(args) => {
            Err(syn::Error::new_spanned(args, "parenthesized payload types are not supported"))
        }
    }
}

/// Handler for `#[cmd(cid = ..., len = ...)]` attribute
fn attr_handle_cmd(attr: &syn::Attribute) -> syn::Result<(u8, Option<u8>)> {
    let nested = attr
        .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)?;

    let mut cid = None;
    let mut len = None;

    for meta in nested {
        let Meta::NameValue(entry) = meta else {
            return Err(syn::Error::new_spanned(
                meta,
                "expected `cid = <integer>` or `len = <integer>`",
            ));
        };

        let Some(key) = entry.path.get_ident() else {
            return Err(syn::Error::new_spanned(&entry.path, "expected `cid` or `len`"));
        };

        let value = match &entry.value {
            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(lit), .. }) => {
                lit.base10_parse::<u8>().map_err(|err| syn::Error::new_spanned(lit, err))?
            }
            other => {
                return Err(syn::Error::new_spanned(other, "expected an integer literal"));
            }
        };

        match key.to_string().as_str() {
            "cid" => {
                if cid.is_some() {
                    return Err(syn::Error::new_spanned(key, "duplicate `cid`"));
                }
                cid = Some(value);
            }
            "len" => {
                if len.is_some() {
                    return Err(syn::Error::new_spanned(key, "duplicate `len`"));
                }
                len = Some(value);
            }
            _ => {
                return Err(syn::Error::new_spanned(key, "expected `cid` or `len`"));
            }
        }
    }

    let cid = cid.ok_or_else(|| syn::Error::new_spanned(attr, "missing `cid = <integer>`"))?;

    Ok((cid, len))
}

/// Collect supported attributes for enum members into [`Attributes`]:
/// * docstring
/// * `cmd(..)` - used to specify size and CID for payload
fn parse_variant_attrs(variant: &Ident, input: &[syn::Attribute]) -> syn::Result<Attributes> {
    let mut doc = Vec::new();
    let mut attrs = None;
    for attr in input {
        if attr.path().is_ident("cmd") {
            if attrs.is_some() {
                return Err(syn::Error::new_spanned(attr, "duplicate `cmd` attribute"));
            }
            attrs = Some(attr_handle_cmd(attr)?);
            continue;
        }
        // Docstrings
        if attr.path().is_ident("doc") {
            doc.push(attr.clone());
            continue;
        }
    }
    let (cid, len) =
        attrs.ok_or_else(|| syn::Error::new_spanned(variant, "missing `#[cmd(...)]` attribute"))?;
    Ok(Attributes { doc, cid, len })
}

// Parse enum variants into a list of [`Command`]s.
// For example:
// ```
// enum Foo<'a> {
//   /// Field documentation.
//   #[cmd(cid=0x1, len=0)]
//   Var(A),        # Command { variant: Var, payload: A, cid: 1, shape: ZeroLength, .. }
//   /// Field documentation...
//   /// ...continued.
//   #[cmd(cid=0x2, len=5)]
//   VarB(B<'a>),   # Command { variant: VarB, payload: B, cid: 2, shape: Fixed { len: 5, .. }, .. }
// }
// ```
fn parse_enum_members(input: &DeriveInput) -> syn::Result<Vec<Command>> {
    let Data::Enum(item) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "CommandHandler can only be derived for enums",
        ));
    };

    let mut items = Vec::new();
    for elem in &item.variants {
        let Fields::Unnamed(fields) = &elem.fields else {
            return Err(syn::Error::new_spanned(
                &elem.ident,
                "expected exactly one unnamed payload field",
            ));
        };

        if fields.unnamed.len() != 1 {
            return Err(syn::Error::new_spanned(
                &elem.fields,
                "expected exactly one unnamed payload field",
            ));
        }

        let payload = parse_variant_fields(&fields.unnamed[0].ty)?;
        let attributes = parse_variant_attrs(&elem.ident, &elem.attrs)?;
        let shape = match (attributes.len, payload.lifetime) {
            (Some(0), None) => CmdShape::ZeroLength,
            (Some(0), Some(lifetime)) => {
                return Err(syn::Error::new_spanned(
                    lifetime,
                    "zero-length payload must not have a lifetime parameter",
                ));
            }
            (Some(len), Some(lifetime)) => CmdShape::Fixed { len, lifetime },
            (None, Some(lifetime)) => CmdShape::Variable { lifetime },
            (Some(_), None) => {
                return Err(syn::Error::new_spanned(
                    &payload.name,
                    "fixed-length payload must have a lifetime parameter",
                ));
            }
            (None, None) => {
                return Err(syn::Error::new_spanned(
                    &payload.name,
                    "variable-length payload must have a lifetime parameter or use `len = 0`",
                ));
            }
        };
        items.push(Command {
            variant: elem.ident.clone(),
            payload: payload.name,
            cid: attributes.cid,
            shape,
            doc: attributes.doc,
        });
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    fn error_for(input: DeriveInput) -> String {
        expand_command_handler(input).unwrap_err().to_string()
    }

    #[test]
    fn rejects_multiple_lifetimes() {
        let input: DeriveInput = parse_quote! {
            enum Command<'a, 'b> {
                A(Payload<'a>),
            }
        };

        assert_eq!(error_for(input), "only one lifetime parameter is supported");
    }

    #[test]
    fn rejects_named_payload_fields() {
        let input: DeriveInput = parse_quote! {
            enum Command {
                A { data: Payload },
            }
        };

        assert_eq!(error_for(input), "expected exactly one unnamed payload field");
    }

    #[test]
    fn rejects_non_lifetime_payload_arguments() {
        let input: DeriveInput = parse_quote! {
            enum Command {
                A(Payload<u8>),
            }
        };

        assert_eq!(error_for(input), "only a lifetime argument is supported");
    }

    #[test]
    fn rejects_missing_cid() {
        let input: DeriveInput = parse_quote! {
            enum Command<'a> {
                #[cmd(len = 2)]
                A(Payload<'a>),
            }
        };

        assert_eq!(error_for(input), "missing `cid = <integer>`");
    }

    #[test]
    fn rejects_unknown_cmd_argument() {
        let input: DeriveInput = parse_quote! {
            enum Command<'a> {
                #[cmd(cid = 1, size = 2)]
                A(Payload<'a>),
            }
        };

        assert_eq!(error_for(input), "expected `cid` or `len`");
    }

    #[test]
    fn rejects_non_integer_cmd_argument() {
        let input: DeriveInput = parse_quote! {
            enum Command<'a> {
                #[cmd(cid = value)]
                A(Payload<'a>),
            }
        };

        assert_eq!(error_for(input), "expected an integer literal");
    }

    #[test]
    fn rejects_duplicate_cmd_attribute() {
        let input: DeriveInput = parse_quote! {
            enum Command<'a> {
                #[cmd(cid = 1)]
                #[cmd(cid = 2)]
                A(Payload<'a>),
            }
        };

        assert_eq!(error_for(input), "duplicate `cmd` attribute");
    }

    #[test]
    fn rejects_fixed_length_payload_without_lifetime() {
        let input: DeriveInput = parse_quote! {
            enum Command {
                #[cmd(cid = 1, len = 2)]
                A(Payload),
            }
        };

        assert_eq!(error_for(input), "fixed-length payload must have a lifetime parameter");
    }

    #[test]
    fn rejects_zero_length_payload_with_lifetime() {
        let input: DeriveInput = parse_quote! {
            enum Command<'a> {
                #[cmd(cid = 1, len = 0)]
                A(Payload<'a>),
            }
        };

        assert_eq!(error_for(input), "zero-length payload must not have a lifetime parameter");
    }

    #[test]
    fn rejects_variable_length_payload_without_lifetime() {
        let input: DeriveInput = parse_quote! {
            enum Command {
                #[cmd(cid = 1)]
                A(Payload),
            }
        };

        assert_eq!(
            error_for(input),
            "variable-length payload must have a lifetime parameter or use `len = 0`"
        );
    }
}
