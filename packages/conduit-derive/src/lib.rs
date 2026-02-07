use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Derive macro for `NodeInput` — generates `from_payload` that reads each
/// struct field from the `Payload` HashMap by name.
#[proc_macro_derive(NodeInput)]
pub fn derive_node_input(input: TokenStream) -> TokenStream {
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

        quote! {
            #field_name: payload
                .get(#field_name_str)
                .and_then(|v| v.downcast_ref::<#ty>())
                .cloned()
                .ok_or(conduit::node::NodeError::MissingInput(#field_name_str))?
        }
    });

    let field_names = fields.named.iter().map(|field| {
        let field_name_str = field.ident.as_ref().unwrap().to_string();
        quote! { #field_name_str }
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

/// Derive macro for `NodeOutput` — generates `into_outputs` that converts
/// each struct field into a `(name, SharedValue)` pair.
#[proc_macro_derive(NodeOutput)]
pub fn derive_node_output(input: TokenStream) -> TokenStream {
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

/// Derive macro for `Node` — generates the `DynNode` bridge implementation
/// and inventory-based registration. The struct should be empty (unit-like or
/// with no meaningful fields); the actual input/output shape is defined by the
/// `ExecutableNode` trait impl.
#[proc_macro_derive(Node)]
pub fn derive_node(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();
    let struct_name_lowercase = name.to_string().to_lowercase();

    let dyn_node_impl = quote! {
        #[async_trait::async_trait]
        impl conduit::traits::DynNode for #name {
            fn name(&self) -> &'static str {
                #struct_name_lowercase
            }

            async fn run_with_payload(
                &self,
                payload: conduit::registry::Payload,
            ) -> Result<Vec<(&'static str, conduit::node::SharedValue)>, conduit::node::NodeError> {
                let input = <<Self as conduit::traits::ExecutableNode>::Input as conduit::traits::NodeInput>::from_payload(&payload)?;
                let output = <Self as conduit::traits::ExecutableNode>::run(self, input).await?;
                Ok(<<Self as conduit::traits::ExecutableNode>::Output as conduit::traits::NodeOutput>::into_outputs(output))
            }

            fn input_fields(&self) -> Vec<&'static str> {
                <<Self as conduit::traits::ExecutableNode>::Input as conduit::traits::NodeInput>::field_names()
            }

            fn output_fields(&self) -> Vec<&'static str> {
                <<Self as conduit::traits::ExecutableNode>::Output as conduit::traits::NodeOutput>::field_names()
            }
        }
    };

    let registration_impl = quote! {
        impl conduit::registry::RegisterableNode for #name {
            fn register_type(registry: &mut conduit::registry::NodeRegistry) {
                let node_name = stringify!(#name).to_string();
                registry.register::<#name>(node_name.as_str());
            }

            fn type_name() -> &'static str {
                #name_str
            }
        }

        inventory::submit! {
            conduit::registry::NodeRegistration::new::<#name>()
        }
    };

    let expanded = quote! {
        #dyn_node_impl
        #registration_impl
    };

    TokenStream::from(expanded)
}
