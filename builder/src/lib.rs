extern crate proc_macro;

use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

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
            // SAFETY: `unwrap` is safe because the ident of a named field cannot be `None`.
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            if type_is_option(ty) || get_builder_attribute(&field).is_some() {
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
            let ty = &field.ty;
            if type_is_option(ty) || get_builder_attribute(&field).is_some() {
                quote! {
                    #ident: <#ty as std::default::Default>::default()
                }
            } else {
                quote! {
                    #ident: std::option::Option::None
                }
            }
        });

        quote! {
            impl #struct_id {
                pub fn builder() -> #builder_id {
                    #builder_id {
                        #(#builder_fields),*
                    }
                }
            }
        }
    };

    let builder_methods_impl = {
        let builder_methods = fields.iter().map(|field| {
            let ident = field.ident.as_ref().unwrap();
            let ty = &field.ty;
            let origin_method = if let Some(inner_ty) = inner_type_of("Option", ty) {
                quote! {
                    pub fn #ident(&mut self, #ident: #inner_ty) -> &mut Self {
                        self.#ident = Some(#ident);
                        self
                    }
                }
            } else if get_builder_attribute(&field).is_some() {
                quote! {
                    pub fn #ident(&mut self, #ident: #ty) -> &mut Self {
                        self.#ident = #ident;
                        self
                    }
                }
            } else {
                quote! {
                    pub fn #ident(&mut self, #ident: #ty) -> &mut Self {
                        self.#ident = std::option::Option::Some(#ident);
                        self
                    }
                }
            };

            match extend(&field) {
                Some((true, extend_method)) => extend_method,
                Some((false, extend_method)) => {
                    quote! {
                        #origin_method
                        #extend_method
                    }
                }
                None => origin_method,
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
            if type_is_option(ty) || get_builder_attribute(&field).is_some() {
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
                pub fn build(&mut self) -> std::result::Result<#struct_id, std::boxed::Box<dyn std::error::Error>> {
                    std::result::Result::Ok(#struct_id {
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
        #build_fn_impl
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

fn get_builder_attribute(field: &syn::Field) -> Option<&syn::Attribute> {
    for attr in &field.attrs {
        if attr.path.segments.len() == 1 && attr.path.segments[0].ident == "builder" {
            return Some(attr);
        }
    }
    None
}

fn extend(field: &syn::Field) -> Option<(bool, proc_macro2::TokenStream)> {
    let ident = field.ident.as_ref().unwrap();

    fn mk_err<T: quote::ToTokens>(t: T) -> Option<(bool, proc_macro2::TokenStream)> {
        Some((
            false,
            syn::Error::new_spanned(t, "expected `builder(each = \"...\")`").to_compile_error(),
        ))
    }

    if let Some(attr) = get_builder_attribute(&field) {
        let meta = match attr.parse_meta() {
            Ok(syn::Meta::List(mut metalist)) => {
                if metalist.nested.len() != 1 {
                    return mk_err(metalist);
                }

                match metalist.nested.pop().unwrap().into_value() {
                    syn::NestedMeta::Meta(syn::Meta::NameValue(nv)) => {
                        if nv.path.get_ident().unwrap() != "each" {
                            return mk_err(metalist);
                        }
                        nv
                    }
                    meta => return mk_err(meta),
                }
            }
            Ok(meta) => return mk_err(meta),
            Err(e) => return Some((false, e.to_compile_error())),
        };

        match &meta.lit {
            syn::Lit::Str(s) => {
                let arg = syn::Ident::new(&s.value(), s.span());
                let inner_ty = inner_type_of("Vec", &field.ty).unwrap();
                let method = quote! {
                    pub fn #arg(&mut self, #arg: #inner_ty) -> &mut Self {
                        self.#ident.push(#arg);
                        self
                    }
                };
                Some((&arg == ident, method))
            }
            lit => {
                return mk_err(lit);
            }
        }
    } else {
        None
    }
}
