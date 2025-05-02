use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, FieldsNamed, Ident};

/// Derive macro for automatically implementing the Descriptor trait and From<Payload> trait
/// This allows nodes to be created with minimal boilerplate
#[proc_macro_derive(Node)]
pub fn derive_node(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract the fields from the struct
    let fields = match &input.data {
        Data::Struct(data) => {
            match &data.fields {
                Fields::Named(fields) => fields,
                _ => panic!("Node derive only works on structs with named fields"),
            }
        }
        _ => panic!("Node derive only works on structs"),
    };

    // Generate implementations
    let descriptor_impl = generate_descriptor_impl(name, fields);
    let from_payload_impl = generate_from_payload_impl(name, fields);

    // Combine the implementations
    let expanded = quote! {
        #descriptor_impl
        #from_payload_impl
    };

    TokenStream::from(expanded)
}

fn generate_descriptor_impl(name: &Ident, fields: &FieldsNamed) -> proc_macro2::TokenStream {
    let field_types = fields.named.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();

        // Check if the field is an Input or Output based on its type
        let ty = &field.ty;
        let type_str = quote!(#ty).to_string();

        if type_str.contains("Input") {
            quote! { FieldType::Input(#field_name_str) }
        } else if type_str.contains("Output") {
            quote! { FieldType::Output(#field_name_str) }
        } else {
            panic!("Field {} must be either Input<T> or Output<T>", field_name_str);
        }
    });

    let output_fields = fields.named.iter().filter_map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let ty = &field.ty;
        let type_str = quote!(#ty).to_string();

        if type_str.contains("Output") {
            Some(quote! {
                (#field_name_str, self.#field_name.into_shared_value())
            })
        } else {
            None
        }
    });

    // Convert struct name to lowercase for the node name
    let struct_name_lowercase = name.to_string().to_lowercase();

    quote! {
        impl Descriptor for #name {
            fn name(&self) -> &'static str {
                #struct_name_lowercase
            }
            
            fn fields(&self) -> Vec<FieldType> {
                vec![
                    #(#field_types),*
                ]
            }
            
            fn take_outputs(self: Box<Self>) -> Vec<(&'static str, SharedValue)> {
                vec![
                    #(#output_fields),*
                ]
            }
        }
    }
}

fn generate_from_payload_impl(name: &Ident, fields: &FieldsNamed) -> proc_macro2::TokenStream {
    let field_initializers = fields.named.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let ty = &field.ty;
        let type_str = quote!(#ty).to_string();

        if type_str.contains("Input") {
            quote! {
                #field_name: Input::new(value.get(#field_name_str).unwrap().to_owned())
            }
        } else if type_str.contains("Output") {
            quote! {
                #field_name: Default::default()
            }
        } else {
            panic!("Field {} must be either Input<T> or Output<T>", field_name_str);
        }
    });

    quote! {
        impl From<Payload> for #name {
            fn from(value: Payload) -> Self {
                Self {
                    #(#field_initializers),*
                }
            }
        }
    }
}
