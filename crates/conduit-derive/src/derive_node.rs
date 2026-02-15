use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

pub fn derive_node_impl(input: TokenStream) -> TokenStream {
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
