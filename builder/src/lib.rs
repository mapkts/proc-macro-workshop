#![allow(unused)]
extern crate proc_macro;

use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder)]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    // eprintln!("{:#?}", ast);

    let struct_id = &ast.ident;
    let builder_id = Ident::new(&format!("{}Builder", struct_id), Span::call_site());

    let fields = if let syn::Data::Struct(syn::DataStruct {
        fields:
            syn::Fields::Named(syn::FieldsNamed {
                named: punct_fields,
                ..
            }),
        ..
    }) = &ast.data
    {
        punct_fields
    } else {
        unimplemented!()
    };

    let builder_struct = {
        let builder_fields = fields.iter().map(|field| {
            // SAFETY: The `unwrap` here is safe because the ident of a named field cannot be `None`.
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            if type_is_option(ty) {
                quote! {
                    #ident: #ty
                }
            } else {
                quote! {
                    #ident: std::option::Option<#ty>
                }
            }
        });
        quote! {
            pub struct #builder_id {
                #(#builder_fields),*
            }
        }
    };

    let builder_fn_impl = {
        let builder_fields = fields.iter().map(|field| {
            let ident = field.ident.as_ref().unwrap();
            quote! {
                #ident: None,
            }
        });

        quote! {
            impl #struct_id {
                pub fn builder() -> #builder_id {
                    #builder_id {
                        #(#builder_fields)*
                    }
                }
            }
        }
    };

    let builder_methods_impl = {
        let builder_methods = fields.iter().map(|field| {
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            if let Some(inner_ty) = inner_type_of("Option", ty) {
                quote! {
                    pub fn #ident(&mut self, #ident: #inner_ty) -> &mut Self {
                        self.#ident = Some(#ident);
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
            impl #builder_id {
                #(#builder_methods)*
            }
        }
    };

    let build_fn_impl = {
        let builder_fields = fields.iter().map(|field| {
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            if type_is_option(ty) {
                quote! {
                    #ident: self.#ident.clone()
                }
            } else {
                quote! {
                    #ident: self.#ident.clone().ok_or(format!("field {} is not set", stringify!(#ident)).as_str())?
                }
            }
        });

        quote! {
            impl #builder_id {
                pub fn build(&mut self) -> Result<#struct_id, Box<dyn std::error::Error>> {
                    Ok(#struct_id {
                        #(#builder_fields),*
                    })
                }
            }
        }
    };

    let tokens = quote! {
        #builder_struct

        #builder_fn_impl

        #builder_methods_impl
        // impl #builder_id {
        //     pub fn executable(&mut self, executable: String) -> &mut Self {
        //         self.executable = Some(executable);
        //         self
        //     }
        //     pub fn args(&mut self, args: Vec<String>) -> &mut Self {
        //         self.args = Some(args);
        //         self
        //     }
        //     pub fn env(&mut self, env: Vec<String>) -> &mut Self {
        //         self.env = Some(env);
        //         self
        //     }
        //     pub fn current_dir(&mut self, current_dir: String) -> &mut Self {
        //         self.current_dir = Some(current_dir);
        //         self
        //     }
        // }

        #build_fn_impl
        // impl #builder_id {
        //     pub fn build(&mut self) -> Result<#struct_id, Box<dyn std::error::Error>> {
        //         Ok(#struct_id {
        //             executable: self.executable.clone().ok_or("field executable is not set")?,
        //             args: self.args.clone().ok_or("field args is not set")?,
        //             env: self.env.clone().ok_or("field env is not set")?,
        //             current_dir: self.current_dir.clone().ok_or("field current_dir is not set")?,
        //         })
        //     }
        // }
    };

    tokens.into()
}

fn inner_type_of<'a>(outer_type: &str, ty: &'a syn::Type) -> std::option::Option<&'a syn::Type> {
    if let syn::Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    }) = ty
    {
        if let Some(syn::PathSegment {
            ident,
            arguments:
                syn::PathArguments::AngleBracketed(syn::AngleBracketedGenericArguments { args, .. }),
        }) = segments.first()
        {
            if ident == outer_type {
                if let Some(syn::GenericArgument::Type(inner_ty)) = args.first() {
                    return Some(inner_ty);
                }
            }
        }
    };
    None
}

fn type_is_option<'a>(ty: &'a syn::Type) -> bool {
    if let syn::Type::Path(syn::TypePath {
        path: syn::Path { segments, .. },
        ..
    }) = ty
    {
        if let Some(syn::PathSegment { ident, .. }) = segments.first() {
            if ident == "Option" {
                return true;
            }
        }
    };
    false
}
