use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

pub fn derive_node_output_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => panic!("NodeOutput derive only works on structs with named fields"),
        },
        _ => panic!("NodeOutput derive only works on structs"),
    };

    let output_entries = fields.named.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        quote! {
            (#field_name_str, std::sync::Arc::new(self.#field_name) as conduit::node::SharedValue)
        }
    });

    let field_names = fields.named.iter().map(|field| {
        let field_name_str = field.ident.as_ref().unwrap().to_string();
        quote! { #field_name_str }
    });

    let expanded = quote! {
        impl conduit::traits::NodeOutput for #name {
            fn into_outputs(self) -> Vec<(&'static str, conduit::node::SharedValue)> {
                vec![#(#output_entries),*]
            }

            fn field_names() -> Vec<&'static str> {
                vec![#(#field_names),*]
            }
        }
    };

    TokenStream::from(expanded)
}
