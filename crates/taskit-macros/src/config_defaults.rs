use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Lit, Meta, parse2};

pub(crate) fn expand(input: TokenStream) -> syn::Result<TokenStream> {
    let input: DeriveInput = parse2(input)?;
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    &input,
                    "ConfigDefaults only supports named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                &input,
                "ConfigDefaults can only be derived for structs",
            ));
        }
    };

    let mut methods = Vec::new();

    for field in fields {
        let Some(field_name) = &field.ident else {
            continue;
        };

        // Look for #[default_value = "..."]
        let mut default_val = None;
        for attr in &field.attrs {
            if let Meta::NameValue(nv) = &attr.meta
                && attr.path().is_ident("default_value")
                && let syn::Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(s), ..
                }) = &nv.value
            {
                default_val = Some(s.value());
            }
        }

        let Some(default_str) = default_val else {
            continue;
        };

        let is_f64 = is_option_f64(&field.ty);

        if is_f64 {
            let default_f64: f64 = default_str
                .parse()
                .map_err(|e| syn::Error::new_spanned(field, format!("invalid f64 default: {e}")))?;
            methods.push(quote! {
                pub fn #field_name(&self) -> f64 {
                    self.#field_name.unwrap_or(#default_f64)
                }
            });
        } else {
            methods.push(quote! {
                pub fn #field_name(&self) -> &str {
                    self.#field_name.as_deref().unwrap_or(#default_str)
                }
            });
        }
    }

    Ok(quote! {
        impl #name {
            #(#methods)*
        }
    })
}

fn is_option_f64(ty: &syn::Type) -> bool {
    let syn::Type::Path(tp) = ty else {
        return false;
    };
    let Some(seg) = tp.path.segments.last() else {
        return false;
    };
    if seg.ident != "Option" {
        return false;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return false;
    };
    let Some(syn::GenericArgument::Type(syn::Type::Path(inner))) = args.args.first() else {
        return false;
    };
    inner.path.segments.last().is_some_and(|s| s.ident == "f64")
}
