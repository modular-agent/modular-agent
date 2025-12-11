//! Procedural macros for agent-stream-kit.
//!
//! Provides an attribute to declare agent metadata alongside the agent type and
//! generate the registration boilerplate.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{
    Expr, ItemStruct, Meta, MetaList, Type, parse_macro_input, parse_quote, punctuated::Punctuated,
    spanned::Spanned, token::Comma,
};

#[proc_macro_attribute]
pub fn askit(attr: TokenStream, item: TokenStream) -> TokenStream {
    askit_agent(attr, item)
}

/// Declare agent metadata and generate `agent_definition` / `register` helpers.
///
/// Example:
/// ```rust,ignore
/// #[askit_agent(
///     title = "Add Int",
///     category = "Utils",
///     inputs = ["int"],
///     outputs = ["int"],
///     integer_config(
///         name = "n",
///         default = 1,
///     )
/// )]
/// struct AdderAgent { /* ... */ }
/// ```
#[proc_macro_attribute]
pub fn askit_agent(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated<Meta, Comma>::parse_terminated);
    let item_struct = parse_macro_input!(item as ItemStruct);

    match expand_askit_agent(args, item_struct) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

struct AgentArgs {
    kind: Option<Expr>,
    name: Option<Expr>,
    title: Option<Expr>,
    description: Option<Expr>,
    category: Option<Expr>,
    inputs: Vec<Expr>,
    outputs: Vec<Expr>,
    configs: Vec<ConfigSpec>,
    global_configs: Vec<ConfigSpec>,
}

#[derive(Default)]
struct CommonConfig {
    name: Option<Expr>,
    default: Option<Expr>,
    title: Option<Expr>,
    description: Option<Expr>,
    hide_title: bool,
    readonly: bool,
}

struct CustomConfig {
    name: Expr,
    default: Expr,
    type_: Expr,
    title: Option<Expr>,
    description: Option<Expr>,
    hide_title: bool,
    readonly: bool,
}

enum ConfigSpec {
    Unit(CommonConfig),
    Boolean(CommonConfig),
    Integer(CommonConfig),
    Number(CommonConfig),
    String(CommonConfig),
    Text(CommonConfig),
    Object(CommonConfig),
    Custom(CustomConfig),
}

fn expand_askit_agent(
    args: Punctuated<Meta, Comma>,
    item: ItemStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    let has_data_field = item.fields.iter().any(|f| match (&f.ident, &f.ty) {
        (Some(ident), Type::Path(tp)) if ident == "data" => tp
            .path
            .segments
            .last()
            .map(|seg| seg.ident == "AgentData")
            .unwrap_or(false),
        _ => false,
    });

    if !has_data_field {
        return Err(syn::Error::new(
            item.span(),
            "#[askit_agent] expects the struct to have a `data: AgentData` field",
        ));
    }

    let mut parsed = AgentArgs {
        kind: None,
        name: None,
        title: None,
        description: None,
        category: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        configs: Vec::new(),
        global_configs: Vec::new(),
    };

    for meta in args {
        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("kind") => {
                parsed.kind = Some(nv.value);
            }
            Meta::NameValue(nv) if nv.path.is_ident("name") => {
                parsed.name = Some(nv.value);
            }
            Meta::NameValue(nv) if nv.path.is_ident("title") => {
                parsed.title = Some(nv.value);
            }
            Meta::NameValue(nv) if nv.path.is_ident("description") => {
                parsed.description = Some(nv.value);
            }
            Meta::NameValue(nv) if nv.path.is_ident("category") => {
                parsed.category = Some(nv.value);
            }
            Meta::NameValue(nv) if nv.path.is_ident("inputs") => {
                parsed.inputs = parse_expr_array(nv.value)?;
            }
            Meta::NameValue(nv) if nv.path.is_ident("outputs") => {
                parsed.outputs = parse_expr_array(nv.value)?;
            }
            Meta::List(ml) if ml.path.is_ident("inputs") => {
                parsed.inputs = collect_exprs(ml)?;
            }
            Meta::List(ml) if ml.path.is_ident("outputs") => {
                parsed.outputs = collect_exprs(ml)?;
            }
            Meta::List(ml) if ml.path.is_ident("string_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::String(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("text_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::Text(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("boolean_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::Boolean(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("integer_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::Integer(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("number_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::Number(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("object_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::Object(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("custom_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::Custom(parse_custom_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("unit_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::Unit(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("string_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::String(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("text_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::Text(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("boolean_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::Boolean(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("integer_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::Integer(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("number_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::Number(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("object_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::Object(parse_common_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("custom_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::Custom(parse_custom_config(ml)?));
            }
            Meta::List(ml) if ml.path.is_ident("unit_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::Unit(parse_common_config(ml)?));
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "unsupported askit_agent argument",
                ));
            }
        }
    }

    let ident = &item.ident;
    let generics = item.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let data_impl = quote! {
        impl #impl_generics ::agent_stream_kit::HasAgentData for #ident #ty_generics #where_clause {
            fn data(&self) -> &::agent_stream_kit::AgentData {
                &self.data
            }

            fn mut_data(&mut self) -> &mut ::agent_stream_kit::AgentData {
                &mut self.data
            }
        }
    };

    let kind = parsed.kind.unwrap_or_else(|| parse_quote! { "Agent" });
    let name_tokens = parsed.name.map(|n| quote! { #n }).unwrap_or_else(|| {
        quote! { concat!(module_path!(), "::", stringify!(#ident)) }
    });

    let title = parsed
        .title
        .ok_or_else(|| syn::Error::new(Span::call_site(), "askit_agent: missing `title`"))?;
    let category = parsed
        .category
        .ok_or_else(|| syn::Error::new(Span::call_site(), "askit_agent: missing `category`"))?;
    let title = quote! { .title(#title) };
    let description = parsed.description.map(|d| quote! { .description(#d) });
    let category = quote! { .category(#category) };

    let inputs = if parsed.inputs.is_empty() {
        quote! {}
    } else {
        let values = parsed.inputs;
        quote! { .inputs(vec![#(#values),*]) }
    };

    let outputs = if parsed.outputs.is_empty() {
        quote! {}
    } else {
        let values = parsed.outputs;
        quote! { .outputs(vec![#(#values),*]) }
    };

    let config_calls = parsed
        .configs
        .into_iter()
        .map(|cfg| match cfg {
            ConfigSpec::Unit(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "unit_config missing `name`")
                })?;
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .unit_config_with(#name, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Boolean(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "boolean_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { false });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .boolean_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Integer(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "integer_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { 0i64 });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .integer_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Number(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "number_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { 0.0f64 });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .number_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::String(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "string_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { "" });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .string_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Text(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "text_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { "" });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .text_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Object(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "object_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| {
                    parse_quote! { ::agent_stream_kit::AgentValue::object_default() }
                });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .object_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Custom(c) => custom_config_call("custom_config_with", c),
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let global_config_calls = parsed
        .global_configs
        .into_iter()
        .map(|cfg| match cfg {
            ConfigSpec::Unit(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "unit_global_config missing `name`")
                })?;
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .unit_global_config_with(#name, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Boolean(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "boolean_global_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { false });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .boolean_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Integer(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "integer_global_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { 0i64 });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .integer_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Number(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "number_global_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { 0.0f64 });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .number_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::String(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "string_global_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { "" });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .string_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Text(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "text_global_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| parse_quote! { "" });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .text_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Object(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "object_global_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| {
                    parse_quote! { ::agent_stream_kit::AgentValue::object_default() }
                });
                let title = c.title.map(|t| quote! { let entry = entry.title(#t); });
                let description = c
                    .description
                    .map(|d| quote! { let entry = entry.description(#d); });
                let hide_title = if c.hide_title {
                    quote! { let entry = entry.hide_title(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .object_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #readonly
                        entry
                    })
                })
            }
            ConfigSpec::Custom(c) => custom_config_call("custom_global_config_with", c),
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let definition_builder = quote! {
        ::agent_stream_kit::AgentDefinition::new(
            #kind,
            #name_tokens,
            Some(::agent_stream_kit::new_agent_boxed::<#ident>),
        )
        #title
        #description
        #category
        #inputs
        #outputs
        #(#config_calls)*
        #(#global_config_calls)*
    };

    let expanded = quote! {
        #item

        #data_impl

        impl #impl_generics #ident #ty_generics #where_clause {
            pub const DEF_NAME: &'static str = #name_tokens;

            pub fn def_name() -> &'static str { Self::DEF_NAME }

            pub fn agent_definition() -> ::agent_stream_kit::AgentDefinition {
                #definition_builder
            }

            pub fn register(askit: &::agent_stream_kit::ASKit) {
                askit.register_agent_definiton(Self::agent_definition());
            }
        }

        ::agent_stream_kit::inventory::submit! {
            ::agent_stream_kit::AgentRegistration {
                build: || #definition_builder,
            }
        }
    };

    Ok(expanded)
}

fn parse_name_type_title_description(
    meta: &Meta,
    name: &mut Option<Expr>,
    type_: &mut Option<Expr>,
    title: &mut Option<Expr>,
    description: &mut Option<Expr>,
) -> bool {
    match meta {
        Meta::NameValue(nv) if nv.path.is_ident("name") => {
            *name = Some(nv.value.clone());
            true
        }
        Meta::NameValue(nv) if nv.path.is_ident("type") => {
            *type_ = Some(nv.value.clone());
            true
        }
        Meta::NameValue(nv) if nv.path.is_ident("type_") => {
            *type_ = Some(nv.value.clone());
            true
        }
        Meta::NameValue(nv) if nv.path.is_ident("title") => {
            *title = Some(nv.value.clone());
            true
        }
        Meta::NameValue(nv) if nv.path.is_ident("description") => {
            *description = Some(nv.value.clone());
            true
        }
        _ => false,
    }
}

fn parse_custom_config(list: MetaList) -> syn::Result<CustomConfig> {
    let mut name = None;
    let mut default = None;
    let mut type_ = None;
    let mut title = None;
    let mut description = None;
    let mut hide_title = false;
    let mut readonly = false;
    let nested = list.parse_args_with(Punctuated::<Meta, Comma>::parse_terminated)?;

    for meta in nested {
        if parse_name_type_title_description(
            &meta,
            &mut name,
            &mut type_,
            &mut title,
            &mut description,
        ) {
            continue;
        }

        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("default") => {
                default = Some(nv.value.clone());
            }
            Meta::Path(p) if p.is_ident("hide_title") => {
                hide_title = true;
            }
            Meta::Path(p) if p.is_ident("readonly") => {
                readonly = true;
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "custom_config supports name, default, type/type_, title, description, hide_title, readonly",
                ));
            }
        }
    }

    let name = name.ok_or_else(|| syn::Error::new(list.span(), "config missing `name`"))?;
    let default =
        default.ok_or_else(|| syn::Error::new(list.span(), "config missing `default`"))?;
    let type_ = type_.ok_or_else(|| syn::Error::new(list.span(), "config missing `type`"))?;

    Ok(CustomConfig {
        name,
        default,
        type_,
        title,
        description,
        hide_title,
        readonly,
    })
}

fn collect_exprs(list: MetaList) -> syn::Result<Vec<Expr>> {
    let values = list.parse_args_with(Punctuated::<Expr, Comma>::parse_terminated)?;
    Ok(values.into_iter().collect())
}

fn parse_expr_array(expr: Expr) -> syn::Result<Vec<Expr>> {
    if let Expr::Array(arr) = expr {
        Ok(arr.elems.into_iter().collect())
    } else {
        Err(syn::Error::new_spanned(
            expr,
            "inputs/outputs expect array expressions",
        ))
    }
}

fn parse_common_config(list: MetaList) -> syn::Result<CommonConfig> {
    let mut cfg = CommonConfig::default();
    let nested = list.parse_args_with(Punctuated::<Meta, Comma>::parse_terminated)?;

    for meta in nested {
        match meta {
            Meta::NameValue(nv) if nv.path.is_ident("name") => {
                cfg.name = Some(nv.value.clone());
            }
            Meta::NameValue(nv) if nv.path.is_ident("default") => {
                cfg.default = Some(nv.value.clone());
            }
            Meta::NameValue(nv) if nv.path.is_ident("title") => {
                cfg.title = Some(nv.value.clone());
            }
            Meta::NameValue(nv) if nv.path.is_ident("description") => {
                cfg.description = Some(nv.value.clone());
            }
            Meta::Path(p) if p.is_ident("hide_title") => {
                cfg.hide_title = true;
            }
            Meta::Path(p) if p.is_ident("readonly") => {
                cfg.readonly = true;
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "config supports name, default, title, description, hide_title, readonly",
                ));
            }
        }
    }

    if cfg.name.is_none() {
        return Err(syn::Error::new(list.span(), "config missing `name`"));
    }
    Ok(cfg)
}

fn custom_config_call(method: &str, cfg: CustomConfig) -> syn::Result<proc_macro2::TokenStream> {
    let CustomConfig {
        name,
        default,
        type_,
        title,
        description,
        hide_title,
        readonly,
    } = cfg;
    let title = title.map(|t| quote! { let entry = entry.title(#t); });
    let description = description.map(|d| quote! { let entry = entry.description(#d); });
    let hide_title = if hide_title {
        quote! { let entry = entry.hide_title(); }
    } else {
        quote! {}
    };
    let readonly = if readonly {
        quote! { let entry = entry.readonly(); }
    } else {
        quote! {}
    };
    let method_ident = format_ident!("{}", method);

    Ok(quote! {
        .#method_ident(#name, #default, #type_, |entry| {
            let entry = entry;
            #title
            #description
            #hide_title
            #readonly
            entry
        })
    })
}
