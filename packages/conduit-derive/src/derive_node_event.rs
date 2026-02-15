use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

use crate::utils::to_snake_case;

pub fn derive_node_event_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("NodeEvent derive only works on enums"),
    };

    let match_arms = variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let event_name = to_snake_case(&variant_name.to_string());

        match &variant.fields {
            Fields::Unit => quote! {
                Self::#variant_name => conduit::traits::EventData::without_value(#event_name)
            },
            Fields::Named(fields) => {
                let field_names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|field| field.ident.as_ref().unwrap())
                    .collect();

                if field_names.len() == 1 {
                    let value_field = field_names[0];
                    quote! {
                        Self::#variant_name { #value_field } => conduit::traits::EventData::with_value(#event_name, #value_field)
                    }
                } else {
                    quote! {
                        Self::#variant_name { #(#field_names),* } => conduit::traits::EventData::with_value(#event_name, (#(#field_names),*))
                    }
                }
            }
            Fields::Unnamed(fields) => {
                let field_names: Vec<syn::Ident> = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(index, _)| syn::Ident::new(&format!("value_{}", index), variant.ident.span()))
                    .collect();

                if field_names.len() == 1 {
                    let value_field = &field_names[0];
                    quote! {
                        Self::#variant_name(#value_field) => conduit::traits::EventData::with_value(#event_name, #value_field)
                    }
                } else {
                    quote! {
                        Self::#variant_name(#(#field_names),*) => conduit::traits::EventData::with_value(#event_name, (#(#field_names),*))
                    }
                }
            }
        }
    });

    let expanded = quote! {
        impl conduit::traits::NodeEvent for #name {
            fn into_parts(self) -> conduit::traits::EventData {
                match self {
                    #(#match_arms),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}
