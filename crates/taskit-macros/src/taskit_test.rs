use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, ItemFn, Token, parse2, punctuated::Punctuated};

pub(crate) fn expand(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let mut func: ItemFn = parse2(item)?;

    let flags: Punctuated<Ident, Token![,]> =
        syn::parse::Parser::parse2(Punctuated::parse_terminated, attr)?;

    let mut use_tempdir = false;
    let mut use_offline = false;

    for flag in &flags {
        match flag.to_string().as_str() {
            "tempdir" => use_tempdir = true,
            "offline" => use_offline = true,
            other => {
                return Err(syn::Error::new_spanned(
                    flag,
                    format!("unknown taskit_test flag: `{other}` (expected: tempdir, offline)"),
                ));
            }
        }
    }

    // Strip injected params from the fn signature — the macro provides them.
    func.sig.inputs.clear();

    let fn_name = &func.sig.ident;
    let fn_block = &func.block;
    let fn_attrs = &func.attrs;
    let fn_vis = &func.vis;

    let offline_guard = if use_offline {
        quote! {
            if ::std::env::var("TASKIT_OFFLINE").as_deref() == Ok("1") {
                eprintln!("skipping {} (TASKIT_OFFLINE=1)", stringify!(#fn_name));
                return;
            }
        }
    } else {
        quote! {}
    };

    let (pre, _post) = if use_tempdir {
        (
            quote! {
                let _guard = ::taskit_testing::TempDirGuard::new();
                let dir = _guard.path();
            },
            quote! {},
        )
    } else {
        (quote! {}, quote! {})
    };

    let expanded = quote! {
        #(#fn_attrs)*
        #[test]
        #fn_vis fn #fn_name() {
            #offline_guard
            #pre
            // Suppress unused variable warning when dir/sh aren't used
            #[allow(unused_variables)]
            { #fn_block }
        }
    };

    Ok(expanded)
}
