use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, PathArguments};

#[proc_macro_derive(CommandHandler)]
pub fn derive_command_handler(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;

    // Vec<(Command, CommandPayload, Option<Lifetime>)>
    let members = parse_enum_members(&input);

    let mut impl_len = Vec::new();
    let mut impl_bytes = Vec::new();
    let mut impl_cid = Vec::new();
    let mut impl_iter_next = Vec::new();

    for (n, (t, _)) in members {
        let n = n.clone();
        let t = t.clone();

        // DownlinkMacCommand.len()
        impl_len.push(quote! {
            Self::#n(_) => #t::len()
        });

        // DownlinkMacCommand.bytes()
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
    }
    .into()
}

// Parse enum variant into tuple: (Name, Option<Lifetime>)
fn parse_variant_arg(input: &syn::Type) -> (Ident, Option<Ident>) {
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
                        syn::GenericArgument::Lifetime(lt) => (var.clone(), Some(lt.ident.clone())),
                        _ => todo!("???"),
                    }
                }
                PathArguments::None => (var.clone(), None),
                PathArguments::Parenthesized(_) => todo!("syn::PathArguments::None"),
            }
        }
        _ => unimplemented!(),
    }
}

// Parse enum variant into list of (Variant, Arg, ArgLifetime) tuples.
fn parse_enum_members(input: &DeriveInput) -> Vec<(Ident, (Ident, Option<Ident>))> {
    let mut items = vec![];
    match input.data {
        Data::Enum(ref item) => {
            for elem in item.variants.clone() {
                if elem.fields.len() != 1 {
                    panic!("Expecting single argument for {}", elem.ident)
                }
                items.push((
                    elem.ident,
                    match elem.fields {
                        Fields::Unnamed(f) => parse_variant_arg(&f.unnamed.get(0).unwrap().ty),
                        Fields::Named(_) | Fields::Unit => panic!("Unsupported!"),
                    },
                ));
            }
        }
        _ => panic!("Unsupported!"),
    };
    items
}
