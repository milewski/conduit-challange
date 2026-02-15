#![doc = "conduit-derive split into modules for maintainability"]
extern crate proc_macro;
use proc_macro::TokenStream;

mod derive_node;
mod derive_node_event;
mod derive_node_input;
mod derive_node_output;
mod node_attr;
mod utils;

// Wrappers at crate root are required for proc-macros to be registered.
#[proc_macro_attribute]
pub fn node(attr: TokenStream, item: TokenStream) -> TokenStream {
    node_attr::node_impl(attr, item)
}

#[proc_macro_derive(NodeInput, attributes(input))]
pub fn derive_node_input(input: TokenStream) -> TokenStream {
    derive_node_input::derive_node_input_impl(input)
}

#[proc_macro_derive(NodeOutput)]
pub fn derive_node_output(input: TokenStream) -> TokenStream {
    derive_node_output::derive_node_output_impl(input)
}

#[proc_macro_derive(NodeEvent)]
pub fn derive_node_event(input: TokenStream) -> TokenStream {
    derive_node_event::derive_node_event_impl(input)
}

#[proc_macro_derive(Node)]
pub fn derive_node(input: TokenStream) -> TokenStream {
    derive_node::derive_node_impl(input)
}
