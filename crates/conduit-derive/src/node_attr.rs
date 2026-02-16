use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

use crate::utils::{extract_emitter_event_type, extract_option_inner_type, extract_result_types};

pub fn node_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let struct_name = fn_name; // Keep same name (lowercase)
    let struct_name_input = syn::Ident::new(&format!("{}Input", fn_name), fn_name.span());

    let mut emitter_argument_identifier: Option<syn::Ident> = None;
    let mut emitter_event_type: Option<syn::Type> = None;

    for arg in &input_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                if let Some(event_type) = extract_emitter_event_type(&pat_type.ty) {
                    emitter_argument_identifier = Some(pat_ident.ident.clone());
                    emitter_event_type = Some(event_type);
                    break;
                }
            }
        }
    }

    // Capture input aliases before filtering attributes
    let mut input_mapping = std::collections::HashMap::new();
    for arg in &input_fn.sig.inputs {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                if emitter_argument_identifier
                    .as_ref()
                    .is_some_and(|identifier| identifier == &pat_ident.ident)
                {
                    continue;
                }
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

    let node_input_fields: Vec<_> = input_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    if emitter_argument_identifier
                        .as_ref()
                        .is_some_and(|identifier| identifier == &pat_ident.ident)
                    {
                        return None;
                    }
                    let ident = &pat_ident.ident;
                    let ty = &pat_type.ty;
                    return Some(quote! { #ident: #ty });
                }
            }
            panic!("Unsupported argument type in node function");
        })
        .collect();

    let function_inputs: Vec<_> = input_fn
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

    let input_fields_extract = input_fn.sig.inputs.iter().filter_map(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                if emitter_argument_identifier
                    .as_ref()
                    .is_some_and(|identifier| identifier == &pat_ident.ident)
                {
                    return None;
                }
                let ident = &pat_ident.ident;
                let ident_str = ident.to_string();
                let ty = &pat_type.ty;
                let option_inner_type = extract_option_inner_type(ty);

                let is_input = input_mapping.get(&ident_str).is_some();

                if is_input {
                    if let Some(inner_type) = option_inner_type {
                        return Some(quote! {
                            #ident: payload
                                .get("input")
                                .or_else(|| payload.get(#ident_str))
                                .map(|value| <#inner_type as conduit::node::FromSharedValue>::from_shared_value(value))
                                .transpose()?
                        });
                    }

                    return Some(quote! {
                        #ident: payload
                            .get("input")
                            .or_else(|| payload.get(#ident_str))
                            .ok_or(conduit::node::NodeError::MissingInput("input or explicit field".to_string()))
                            .and_then(|value| <#ty as conduit::node::FromSharedValue>::from_shared_value(value))?
                    });
                } else {
                    if let Some(inner_type) = option_inner_type {
                        return Some(quote! {
                            #ident: payload
                                .get(#ident_str)
                                .map(|value| <#inner_type as conduit::node::FromSharedValue>::from_shared_value(value))
                                .transpose()?
                        });
                    }

                    return Some(quote! {
                        #ident: payload
                            .get(#ident_str)
                            .ok_or(conduit::node::NodeError::MissingInput(#ident_str.to_string()))
                            .and_then(|value| <#ty as conduit::node::FromSharedValue>::from_shared_value(value))?
                    });
                }
            }
        }
        panic!("Unsupported argument type");
    });

    let input_field_names = input_fn.sig.inputs.iter().filter_map(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                if emitter_argument_identifier
                    .as_ref()
                    .is_some_and(|identifier| identifier == &pat_ident.ident)
                {
                    return None;
                }
                let ident_str = pat_ident.ident.to_string();
                let is_input = input_mapping.get(&ident_str).is_some();
                if is_input {
                    return Some(quote! { #ident_str, "input" });
                } else {
                    return Some(quote! { #ident_str });
                }
            }
        }
        panic!("Unsupported argument type");
    });

    let args_destructure = input_fn.sig.inputs.iter().map(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let ident = &pat_ident.ident;
                if emitter_argument_identifier
                    .as_ref()
                    .is_some_and(|identifier| identifier == ident)
                {
                    return quote! { emitter };
                }
                return quote! { input.#ident };
            }
        }
        panic!("Unsupported");
    });

    let body = &input_fn.block;
    let node_event_type = emitter_event_type.unwrap_or_else(|| syn::parse_quote! { () });

    let (output_ty, result_error_type) = match &input_fn.sig.output {
        syn::ReturnType::Default => (quote! { () }, None),
        syn::ReturnType::Type(_, ty) => {
            if let Some((ok_type, error_type)) = extract_result_types(ty) {
                (quote! { #ok_type }, Some(quote! { #error_type }))
            } else {
                (quote! { #ty }, None)
            }
        }
    };

    let run_impl = if let Some(error_type) = result_error_type {
        quote! {
            let result: Result<#output_ty, #error_type> = {
                let func = |#(#function_inputs),*| async move #body;
                func(#(#args_destructure),*).await
            };
            result.map_err(|error| conduit::node::NodeError::from(error.to_string()))
        }
    } else {
        quote! {
            let func = |#(#function_inputs),*| async move #body;
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
            #(#node_input_fields),*
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
            type Event = #node_event_type;

            async fn run(
                &self,
                input: Self::Input,
                emitter: conduit::traits::Emitter<Self::Event>,
            ) -> Result<Self::Output, conduit::node::NodeError> {
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
                self.run_with_payload_with_event_sender(payload, None).await
            }

            async fn run_with_payload_with_event_sender(
                &self,
                payload: conduit::registry::Payload,
                event_sender: Option<tokio::sync::mpsc::UnboundedSender<conduit::traits::EventData>>,
            ) -> Result<conduit::traits::NodeExecutionResult, conduit::node::NodeError> {
                let emitter = conduit::traits::Emitter::<<Self as conduit::traits::ExecutableNode>::Event>::with_event_sender(event_sender);
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
