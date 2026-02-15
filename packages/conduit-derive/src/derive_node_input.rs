use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

use crate::utils::extract_option_inner_type;

pub fn derive_node_input_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => panic!("NodeInput derive only works on structs with named fields"),
        },
        _ => panic!("NodeInput derive only works on structs"),
    };

    let field_extractors = fields.named.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let ty = &field.ty;
        let option_inner_type = extract_option_inner_type(ty);

        let has_input_attr = field.attrs.iter().any(|attr| attr.path().is_ident("input"));

        if has_input_attr {
            if let Some(inner_type) = option_inner_type {
                return quote! {
                    #field_name: payload
                        .get("input")
                        .or_else(|| payload.get(#field_name_str))
                        .map(|value| <#inner_type as conduit::node::FromSharedValue>::from_shared_value(value))
                        .transpose()?
                };
            }

            quote! {
                #field_name: payload
                    .get("input")
                    .or_else(|| payload.get(#field_name_str))
                    .ok_or(conduit::node::NodeError::MissingInput("input or explicit field".to_string()))
                    .and_then(|value| <#ty as conduit::node::FromSharedValue>::from_shared_value(value))?
            }
        } else {
            if let Some(inner_type) = option_inner_type {
                return quote! {
                    #field_name: payload
                        .get(#field_name_str)
                        .map(|value| <#inner_type as conduit::node::FromSharedValue>::from_shared_value(value))
                        .transpose()?
                };
            }

            quote! {
                #field_name: payload
                    .get(#field_name_str)
                    .ok_or(conduit::node::NodeError::MissingInput(#field_name_str.to_string()))
                    .and_then(|value| <#ty as conduit::node::FromSharedValue>::from_shared_value(value))?
            }
        }
    });

    let field_names = fields.named.iter().map(|field| {
        let field_name_str = field.ident.as_ref().unwrap().to_string();
        let has_input_attr = field.attrs.iter().any(|attr| attr.path().is_ident("input"));

        if has_input_attr {
            quote! { #field_name_str, "input" }
        } else {
            quote! { #field_name_str }
        }
    });

    let expanded = quote! {
        impl conduit::traits::NodeInput for #name {
            fn from_payload(payload: &conduit::registry::Payload) -> Result<Self, conduit::node::NodeError> {
                Ok(Self {
                    #(#field_extractors),*
                })
            }

            fn field_names() -> Vec<&'static str> {
                vec![#(#field_names),*]
            }
        }
    };

    TokenStream::from(expanded)
}
