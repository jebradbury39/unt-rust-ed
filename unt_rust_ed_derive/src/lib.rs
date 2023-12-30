use proc_macro::TokenStream;

use syn::{parse_macro_input, DeriveInput, parse_quote, Data, Fields, GenericParam, Generics, Index, Token};
use syn::token::{Paren, Bracket};
use syn::punctuated::Punctuated;
use syn::__private::Span;

use quote::quote;

#[proc_macro_derive(ExportedHostType)]
pub fn exported_host_type(initial_input: TokenStream) -> TokenStream {
    let defstr = initial_input.to_string();

    // parse input into an ast
    let input = parse_macro_input!(initial_input as DeriveInput);

    let name = input.ident;

    // Add a bound `T: ExportedHostType` to every type parameter T.
    let generics = add_trait_bounds(input.generics);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let expanded = quote! {
        // the generated impl
        impl #impl_generics unt_rust_ed::ExportedHostType for #name #ty_generics #where_clause {
            fn typedef_as_string() -> &'static str {
                #defstr
            }
        }
    };

    // hand outpput back to compiler
    TokenStream::from(expanded)    
}

// Add a bound `T: HeapSize` to every type parameter T.
fn add_trait_bounds(mut generics: Generics) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote!(unt_rust_ed::ExportedHostType));
        }
    }
    generics
}
