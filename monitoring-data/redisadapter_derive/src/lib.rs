extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn;

#[proc_macro_derive(RedisInsertWriter, attributes(name))]
pub fn insert_writer_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_insert_writer(&ast)
}

#[proc_macro_derive(RedisOutputReader, attributes(uuid))]
pub fn output_reader_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_output_reader(&ast)
}

#[proc_macro_derive(RedisIdentifiable, attributes(name, single_instance))]
pub fn identifiable_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    impl_identifiable(&ast)
}

fn impl_insert_writer(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let data = match &ast.data {
        syn::Data::Struct(data) => data,
        _ => panic!("Only structs are supported"),
    };

    let mut db_name = format!("{}s", name.to_string().to_lowercase());
    ast.attrs.iter().for_each(|attr| {
        if attr.path.is_ident("name") {
            db_name = attr.parse_args::<syn::LitStr>().unwrap().value();
        }
    });

    let sets: Vec<proc_macro2::TokenStream> = data.fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        quote! {
            self.#field_name.write(pipe, format!("{base_key}:{}", stringify!(#field_name)).as_str())?;
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

fn impl_output_reader(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let data = match &ast.data {
        syn::Data::Struct(data) => data,
        _ => panic!("Only structs are supported"),
    };

    let mut uuid_field = Option::None;

    let found = data
        .fields
        .iter()
        .find(|x| x.attrs.iter().any(|x| x.path.is_ident("uuid")));
    if let Some(found) = found {
        uuid_field = Some(found.ident.as_ref().unwrap());
    }

    let sets: Vec<proc_macro2::TokenStream> = data
        .fields
        .iter()
        .filter(|x| uuid_field == None || x.ident.as_ref().unwrap() != uuid_field.unwrap())
        .map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            quote! {
                #field_name: <#ty as crate::adapters::redis::RedisOutputReader>::read(connection, &format!("{base_key}:{}", stringify!(#field_name)))?
            }
        })
        .collect();

    let uuid_code = match uuid_field {
        Some(field) => quote! {
            #field: base_key.split(":").next().ok_or("Key is invalid")?.to_owned(),
        },
        None => quote! {},
    };

    let gen = quote! {
        impl crate::adapters::redis::RedisOutputReader for #name {
            fn read(connection: &mut redis::Connection, base_key: &str) -> Result<Self, Box<dyn std::error::Error>> {
                Ok(Self {
                    #uuid_code
                    #(#sets),*
                })
            }
        }
    };

    gen.into()
}

fn impl_identifiable(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let mut db_name = format!("{}s", name.to_string().to_lowercase());
    ast.attrs.iter().for_each(|attr| {
        if attr.path.is_ident("name") {
            db_name = attr.parse_args::<syn::LitStr>().unwrap().value();
        }
    });

    let mut single_instance = false;
    ast.attrs.iter().for_each(|attr| {
        if attr.path.is_ident("single_instance") {
            single_instance = attr.parse_args::<syn::LitBool>().unwrap().value();
        }
    });

    let next_uuid = match single_instance {
        true => quote! {
            fn next_uuid(connection: &mut redis::Connection) -> Result<String, Box<dyn std::error::Error>> {
                Ok(format!("-1:{}", counter, Self::name()))
            }
        },
        false => quote! {},
    };

    let gen = quote! {
        impl crate::adapters::redis::RedisIdentifiable for #name {
            fn name() -> String {
                #db_name.to_owned()
            }

            #next_uuid
        }
    };
    gen.into()
}
