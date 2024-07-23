extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn;

// InsertWriter currently only supports structs with fields that are either primitive types or implement RedisInsertWriter.
#[proc_macro_derive(RedisInsertWriter)]
pub fn insert_writer_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_insert_writer(&ast)
}


fn impl_insert_writer(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let data = match &ast.data {
        syn::Data::Struct(data) => data,
        _ => panic!("Only structs are supported"),
    };

    let sets: Vec<proc_macro2::TokenStream> = data.fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        quote! {
            pipe.set(format!("{base_key}:{}", stringify!(#field_name)), self.#field_name.clone());
        }

    }).collect();


    let gen = quote! {
            impl crate::adapters::redis::RedisInsertWriter for #name {
                fn write(&self, pipe: &mut redis::Pipeline, base_key: &str) -> Result<(), Box<dyn std::error::Error>> {
                    #(#sets)*
                    Ok(())
                }
            }
    };
    gen.into()
}
