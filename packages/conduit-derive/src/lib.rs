use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, FieldsNamed, Ident, parse_macro_input};

/// Derive macro for automatically implementing the Descriptor trait, From<Payload> trait,
/// and registering the node with the NodeRegistry
#[proc_macro_derive(Node)]
pub fn derive_node(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract the fields from the struct
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => fields,
            _ => panic!("Node derive only works on structs with named fields"),
        },
        _ => panic!("Node derive only works on structs"),
    };

    // Generate implementations
    let descriptor_impl = generate_descriptor_impl(name, fields);
    let from_payload_impl = generate_from_payload_impl(name, fields);

    // Generate registration code
    let name_str = name.to_string();
    let registration_impl = quote! {
        // Implement the RegisterableNode trait for this node type
        impl conduit::registry::RegisterableNode for #name {
            fn register_type(registry: &mut conduit::registry::NodeRegistry) {
                // Convert struct name to lowercase for registration
                let node_name = stringify!(#name).to_string();

                // Use as_str() to convert String to &str
                registry.register::<#name>(node_name.as_str());
            }

            fn type_name() -> &'static str {
                #name_str
            }
        }

        // Add this node to the inventory
        inventory::submit! {
            conduit::registry::NodeRegistration::new::<#name>()
        }
    };

    // Combine the implementations
    let expanded = quote! {
        #descriptor_impl

        #from_payload_impl

        #registration_impl
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
            quote! { conduit::traits::FieldType::Input(#field_name_str) }
        } else if type_str.contains("Output") {
            quote! { conduit::traits::FieldType::Output(#field_name_str) }
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
        impl conduit::traits::Descriptor for #name {
            fn name(&self) -> &'static str {
                #struct_name_lowercase
            }

            fn fields(&self) -> Vec<conduit::traits::FieldType> {
                vec![
                    #(#field_types),*
                ]
            }

            fn take_outputs(self: Box<Self>) -> Vec<(&'static str, conduit::node::SharedValue)> {
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
                #field_name: conduit::node::Input::new(value.get(#field_name_str).unwrap().to_owned())
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
        impl From<conduit::registry::Payload> for #name {
            fn from(value: conduit::registry::Payload) -> Self {
                Self {
                    #(#field_initializers),*
                }
            }
        }
    }
}
