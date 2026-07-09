mod config_defaults;
mod taskit_test;

use proc_macro::TokenStream;

/// Composable test attribute macro.
///
/// ```ignore
/// #[taskit_test]              // equivalent to #[test]
/// #[taskit_test(tempdir)]     // run in tempdir, inject `dir: &Path`
/// #[taskit_test(offline)]     // skip when TASKIT_OFFLINE=1
/// #[taskit_test(tempdir, offline)]  // both
/// ```
#[proc_macro_attribute]
pub fn taskit_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    taskit_test::expand(attr.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Derive macro for generating getter methods on config structs
/// with `Option<T>` fields annotated with `#[default_value = "..."]`.
///
/// ```ignore
/// #[derive(ConfigDefaults)]
/// struct FlowConfig {
///     #[default_value = "main"]
///     main_branch: Option<String>,
///     #[default_value = "80.0"]
///     threshold: Option<f64>,
/// }
/// // Generates:
/// //   fn main_branch(&self) -> &str
/// //   fn threshold(&self) -> f64
/// ```
#[proc_macro_derive(ConfigDefaults, attributes(default_value))]
pub fn derive_config_defaults(input: TokenStream) -> TokenStream {
    config_defaults::expand(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}
