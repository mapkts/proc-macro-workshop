extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, Token};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    // eprintln!("{:#?}", ast);

    let struct_ident = &ast.ident;
    let builder_ident = format_ident!("{}Builder", struct_ident);

    let fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { named, .. }),
        ..
    }) = ast.data
    {
        named
    } else {
        unimplemented!()
    };

    let tokens = quote! {
        impl #builder_ident {
            pub fn build(&mut self) -> Result<#struct_ident, Box<dyn std::error::Error>> {
                Ok(#struct_ident {
                    executable: self.executable.clone().ok_or("field executable is not set")?,
                    args: self.args.clone().ok_or("field args is not set")?,
                    env: self.env.clone().ok_or("field env is not set")?,
                    current_dir: self.current_dir.clone().ok_or("field current_dir is not set")?,
                })
            }
        }
    };

    tokens.into()
}

fn inner_type_of<'a>(outer_type: &str, ty: &'a syn::Type) -> Option<&'a syn::Type> {
    if let syn::Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    }) = ty
    {
        if let Some(syn::PathSegment {
            ident,
            arguments: syn::PathArguments::AngleBracketed(ref inner_type),
        }) = segments.first()
        {
            if ident == outer_type {
                if let Some(syn::GenericArgument::Type(ref ty)) = inner_type.args.first() {
                    return Some(ty);
                }
            }
        }
    };
    None
}

fn define_struct_builder(
    builder_ident: &syn::Ident,
    fields: &syn::punctuated::Punctuated<syn::Field, Token![,]>,
) -> proc_macro2::TokenStream {
    let fields = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if let Some(inner_ty) = inner_type_of("Option", ty) {
            quote! { #ident: std::option::Option<#inner_ty> }
        } else {
            quote! { #ident: #ty }
        }
    });

    quote! {
        pub struct #builder_ident {
            #(#fields),*
        }
    }
}

fn impl_struct_fn(
    struct_ident: &syn::Ident,
    builder_ident: &syn::Ident,
    fields: &syn::punctuated::Punctuated<syn::Field, Token![,]>,
) -> proc_macro2::TokenStream {
    let fields = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        quote! { #ident: <#ty as core::default::Default>::default() }
    });

    quote! {
        impl #struct_ident {
            pub fn builder() -> #builder_ident {
                #(#fields),*
            }
        }
    }
}

fn impl_builder_methods(
    builder_ident: &syn::Ident,
    fields: &syn::punctuated::Punctuated<syn::Field, Token![,]>,
) -> proc_macro2::TokenStream {
    let methods = fields.iter().map(|field| {
        let ident = &field.ident;
        let ty = &field.ty;
        if let Some(ty) = inner_type_of("Option", ty) {
            quote! {
                pub fn #ident(&mut self, #ident: #ty) -> &mut Self {
                    self.#ident = #ident;
                    self
                }
            }
        } else {
            quote! {
                pub fn #ident(&mut self, #ident: #ty) -> &mut Self {
                    self.#ident = Some(#ident);
                    self
                }
            }
        }
    });

    quote! {
        impl #builder_ident {
            #(#methods)*
        }
    }
}
