use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, ItemFn, parse_macro_input};

// ... existing derives ...

fn extract_ok_type(ty: &syn::Type) -> Option<&syn::Type> {
    if let syn::Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Result" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if !args.args.is_empty() {
                        if let syn::GenericArgument::Type(inner) = &args.args[0] {
                            return Some(inner);
                        }
                    }
                }
            }
        }
    }
    None
}

#[proc_macro_attribute]
pub fn node(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let struct_name = fn_name; // Keep same name (lowercase)
    let struct_name_input = syn::Ident::new(&format!("{}Input", fn_name), fn_name.span());

    // Capture input aliases before filtering attributes
    let mut input_mapping = std::collections::HashMap::new();
    for arg in &input_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                for attr in &pat_type.attrs {
                    if attr.path().is_ident("input") {
                        let name = pat_ident.ident.to_string();
                        input_mapping.insert(name, "input".to_string());
                        break;
                    }
                }
            }
        }
    }

    // Filter out #[input] attribute from arguments to avoid compilation error in the generated function
    for arg in &mut input_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            pat_type.attrs.retain(|attr| !attr.path().is_ident("input"));
        }
    }

    let inputs: Vec<_> = input_fn
        .sig
        .inputs
        .iter()
        .map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let ident = &pat_ident.ident;
                    let ty = &pat_type.ty;
                    return quote! { #ident: #ty };
                }
            }
            panic!("Unsupported argument type in node function");
        })
        .collect();

    let input_fields_extract = input_fn.sig.inputs.iter().map(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let ident = &pat_ident.ident;
                let ident_str = ident.to_string();
                let ty = &pat_type.ty;

                let is_input = input_mapping.get(&ident_str).is_some();

                if is_input {
                    return quote! {
                        #ident: payload
                            .get("input")
                            .or_else(|| payload.get(#ident_str))
                            .ok_or(conduit::node::NodeError::MissingInput("input or explicit field".to_string()))
                            .and_then(|v| <#ty as conduit::node::FromSharedValue>::from_shared_value(v))?
                    };
                } else {
                    return quote! {
                        #ident: payload
                            .get(#ident_str)
                            .ok_or(conduit::node::NodeError::MissingInput(#ident_str.to_string()))
                            .and_then(|v| <#ty as conduit::node::FromSharedValue>::from_shared_value(v))?
                    };
                }
            }
        }
        panic!("Unsupported argument type");
    });

    let input_field_names = input_fn.sig.inputs.iter().map(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let ident_str = pat_ident.ident.to_string();
                let is_input = input_mapping.get(&ident_str).is_some();
                if is_input {
                    return quote! { #ident_str, "input" };
                } else {
                    return quote! { #ident_str };
                }
            }
        }
        panic!("Unsupported argument type");
    });

    let args_destructure = input_fn.sig.inputs.iter().map(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let ident = &pat_ident.ident;
                return quote! { input.#ident };
            }
        }
        panic!("Unsupported");
    });

    let body = &input_fn.block;

    let (output_ty, is_result) = match &input_fn.sig.output {
        syn::ReturnType::Default => (quote! { () }, false),
        syn::ReturnType::Type(_, ty) => {
            if let Some(ok_ty) = extract_ok_type(ty) {
                (quote! { #ok_ty }, true)
            } else {
                (quote! { #ty }, false)
            }
        }
    };

    let run_impl = if is_result {
        quote! {
            func(#(#args_destructure),*).await.map_err(|error| conduit::node::NodeError::from(error.to_string()))
        }
    } else {
        quote! {
            Ok(func(#(#args_destructure),*).await)
        }
    };

    let struct_name_str = struct_name.to_string();

    let expanded = quote! {
        #[allow(non_camel_case_types)]
        #[derive(Default, Clone)]
        struct #struct_name;

        #[allow(non_camel_case_types)]
        struct #struct_name_input {
            #(#inputs),*
        }

        impl conduit::traits::NodeInput for #struct_name_input {
            fn from_payload(payload: &conduit::registry::Payload) -> Result<Self, conduit::node::NodeError> {
                Ok(Self {
                    #(#input_fields_extract),*
                })
            }
            fn field_names() -> Vec<&'static str> {
                vec![#(#input_field_names),*]
            }
        }

        #[async_trait::async_trait]
        impl conduit::traits::ExecutableNode for #struct_name {
            type Input = #struct_name_input;
            type Output = #output_ty;
            type Event = ();

            async fn run(
                &self,
                input: Self::Input,
                _emitter: conduit::traits::Emitter<Self::Event>,
            ) -> Result<Self::Output, conduit::node::NodeError> {
                let func = |#(#inputs),*| async move #body;
                #run_impl
            }
        }

        #[async_trait::async_trait]
        impl conduit::traits::DynNode for #struct_name {
            fn name(&self) -> &'static str {
                #struct_name_str
            }

            async fn run_with_payload(
                &self,
                payload: conduit::registry::Payload,
            ) -> Result<conduit::traits::NodeExecutionResult, conduit::node::NodeError> {
                let emitter = conduit::traits::Emitter::<<Self as conduit::traits::ExecutableNode>::Event>::default();
                let input = <<Self as conduit::traits::ExecutableNode>::Input as conduit::traits::NodeInput>::from_payload(&payload)?;
                let output = <Self as conduit::traits::ExecutableNode>::run(self, input, emitter.clone()).await?;
                Ok(conduit::traits::NodeExecutionResult {
                    outputs: <<Self as conduit::traits::ExecutableNode>::Output as conduit::traits::NodeOutput>::into_outputs(output),
                    events: emitter.into_events(),
                })
            }

            fn input_fields(&self) -> Vec<&'static str> {
                <<Self as conduit::traits::ExecutableNode>::Input as conduit::traits::NodeInput>::field_names()
            }

            fn output_fields(&self) -> Vec<&'static str> {
                <<Self as conduit::traits::ExecutableNode>::Output as conduit::traits::NodeOutput>::field_names()
            }
        }

        impl conduit::registry::RegisterableNode for #struct_name {
            fn register_type(registry: &mut conduit::registry::NodeRegistry) {
                // Use the string literal name, not stringify! which might vary
                registry.register::<#struct_name>(#struct_name_str);
            }

            fn type_name() -> &'static str {
                #struct_name_str
            }
        }

        inventory::submit! {
            conduit::registry::NodeRegistration::new::<#struct_name>()
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_derive(NodeInput, attributes(input))]
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

        let has_input_attr = field.attrs.iter().any(|attr| attr.path().is_ident("input"));

        if has_input_attr {
            quote! {
                #field_name: payload
                    .get("input")
                    .or_else(|| payload.get(#field_name_str))
                    .ok_or(conduit::node::NodeError::MissingInput("input or explicit field".to_string()))
                    .and_then(|v| <#ty as conduit::node::FromSharedValue>::from_shared_value(v))?
            }
        } else {
            quote! {
                #field_name: payload
                    .get(#field_name_str)
                    .ok_or(conduit::node::NodeError::MissingInput(#field_name_str.to_string()))
                    .and_then(|v| <#ty as conduit::node::FromSharedValue>::from_shared_value(v))?
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
            ) -> Result<conduit::traits::NodeExecutionResult, conduit::node::NodeError> {
                let emitter = conduit::traits::Emitter::<<Self as conduit::traits::ExecutableNode>::Event>::default();
                let input = <<Self as conduit::traits::ExecutableNode>::Input as conduit::traits::NodeInput>::from_payload(&payload)?;
                let output = <Self as conduit::traits::ExecutableNode>::run(self, input, emitter.clone()).await?;
                Ok(conduit::traits::NodeExecutionResult {
                    outputs: <<Self as conduit::traits::ExecutableNode>::Output as conduit::traits::NodeOutput>::into_outputs(output),
                    events: emitter.into_events(),
                })
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
