extern crate proc_macro;

use lazy_static::lazy_static;
use proc_macro::TokenStream;
use quote::quote;
use std::collections::HashMap;
use std::sync::Mutex;
use syn::DeriveInput;
use syn::{self, DataStruct, Ident};

struct SafeDataStruct {
    inner: DataStruct,
    impl_type: ImplType,
}
unsafe impl Sync for SafeDataStruct {}
unsafe impl Send for SafeDataStruct {}

impl SafeDataStruct {
    pub fn new(data_struct: DataStruct, impl_type: ImplType) -> Self {
        Self {
            inner: data_struct,
            impl_type,
        }
    }
}

impl Into<DataStruct> for SafeDataStruct {
    fn into(self) -> DataStruct {
        self.inner
    }
}

enum ImplType {
    InsertWriter,
    OutputReader,
    Identifiable,
    Updater,
}

lazy_static! {
    static ref DB_STRUCTS: Mutex<HashMap<Ident, SafeDataStruct>> = Mutex::new(HashMap::new());
}

#[proc_macro_derive(RedisInsertWriter, attributes(name))]
pub fn insert_writer_derive(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = syn::parse(input).unwrap();
    insert_new_struct(&ast);
    impl_insert_writer(&ast)
}

#[proc_macro_derive(RedisOutputReader, attributes(uuid))]
pub fn output_reader_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    insert_new_struct(&ast);
    impl_output_reader(&ast)
}

#[proc_macro_derive(RedisIdentifiable, attributes(name, single_instance))]
pub fn identifiable_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    insert_new_struct(&ast);
    impl_identifiable(&ast)
}

#[proc_macro_derive(RedisUpdater, attributes(name))]
pub fn updater_derive(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).unwrap();
    for (key, strct) in DB_STRUCTS.lock().unwrap().iter() {}
    impl_updater(&ast)
}

fn impl_insert_writer(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let data = match &ast.data {
        syn::Data::Struct(data) => data,
        _ => panic!("Only structs are supported"),
    };

    let db_name = get_name_attr(ast);

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

    let db_name = get_name_attr(ast);

    let mut single_instance = false;
    ast.attrs.iter().for_each(|attr| {
        if attr.path.is_ident("single_instance") {
            single_instance = attr.parse_args::<syn::LitBool>().unwrap().value();
        }
    });

    let next_uuid = match single_instance {
        true => quote! {
            fn next_uuid(connection: &mut redis::Connection) -> Result<String, Box<dyn std::error::Error>> {
                Ok(format!("-1:{}", Self::name()))
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

fn impl_updater(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    let data = match &ast.data {
        syn::Data::Struct(data) => data,
        _ => panic!("Only structs are supported"),
    };

    let sets: Vec<proc_macro2::TokenStream> = data.fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        quote! {
            if self.#field_name.is_some() {
                self.#field_name.clone().unwrap().write(pipe, format!("{uuid}:{}", stringify!(#field_name)).as_str())?;
            }
        }
    }).collect();

    let mut updater_name = format!("{}Updater", name.to_string());
    ast.attrs.iter().for_each(|attr| {
        if attr.path.is_ident("update_struct") {
            updater_name = attr.parse_args::<syn::LitStr>().unwrap().value();
        }
    });

    let updater_ident = Ident::new(&updater_name, proc_macro2::Span::call_site());

    let gen = quote! {
            impl crate::adapters::redis::RedisUpdater<#name> for #updater_ident {
                fn update(&self, pipe: &mut redis::Pipeline, uuid: &str) -> Result<(), Box<dyn std::error::Error>> {
                    use crate::adapters::redis::RedisInsertWriter;
                    #(#sets)*
                    Ok(())
                }
            }
    };
    gen.into()
}

fn get_name_attr(ast: &syn::DeriveInput) -> String {
    for attr in ast.attrs.iter() {
        if attr.path.is_ident("name") {
            return attr.parse_args::<syn::LitStr>().unwrap().value();
        }
    }
    let name = &ast.ident;
    format!("{}s", name.to_string().to_lowercase())
}

fn insert_new_struct(ast: &syn::DeriveInput) {
    let mut db_structs = DB_STRUCTS.lock().unwrap();
    if db_structs.get(&ast.ident).is_none() {
        let safe_struct = SafeDataStruct::new(
            match ast.data {
                syn::Data::Struct(ref data) => data.clone(),
                _ => panic!("Only structs are supported"),
            },
            ImplType::InsertWriter,
        );
        db_structs.insert(ast.ident.clone(), safe_struct);
    } else {
        panic!("Duplicate definition of struct: {}", ast.ident.to_string());
    }
}
