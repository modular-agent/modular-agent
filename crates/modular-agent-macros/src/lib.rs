#![recursion_limit = "256"]
//! Procedural macros for modular-agent-core.
//!
//! Provides the [`#[modular_agent]`](modular_agent) attribute macro to declare agent metadata
//! alongside the agent type and generate the registration boilerplate.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{
    Expr, ItemStruct, Meta, MetaList, Type, parse_macro_input, parse_quote, punctuated::Punctuated,
    spanned::Spanned, token::Comma,
};

/// Declare agent metadata and generate `agent_definition` / `register` helpers.
///
/// This macro transforms a struct into a modular agent by:
/// - Implementing `HasAgentData` trait
/// - Generating `agent_definition()` and `register()` methods
/// - Registering the agent with the inventory for automatic discovery
///
/// # Requirements
///
/// The struct must have a `data: AgentData` field.
///
/// # Attributes
///
/// ## Required
///
/// - `title = "..."` - Display title shown in the UI
/// - `category = "..."` - Category for organization (e.g., "Utils", "LLM/Chat")
///
/// ## Optional Metadata
///
/// - `name = "..."` - Override the definition name (default: `module::path::StructName`)
/// - `description = "..."` - Description text
/// - `kind = "..."` - Agent kind (default: "Agent")
/// - `hide_title` - Hide the title in the UI
///
/// ## Ports
///
/// - `inputs = ["port1", "port2", ...]` - Input port names
/// - `outputs = ["port1", "port2", ...]` - Output port names
///
/// ## Configuration
///
/// Add configuration fields using `*_config(...)` attributes. Each config type accepts:
/// - `name = "..."` (required) - Config key name
/// - `default = ...` - Default value
/// - `title = "..."` - Display title
/// - `description = "..."` - Description
/// - `hide_title` - Hide title in UI
/// - `hidden` - Hide from UI entirely
/// - `readonly` - Make read-only in UI
/// - `detail` - Show only in detail view
///
/// ### Config Types
///
/// | Type | Macro | Default | Description |
/// |------|-------|---------|-------------|
/// | Boolean | `boolean_config(...)` | `false` | True/false toggle |
/// | Integer | `integer_config(...)` | `0` | 64-bit signed integer |
/// | Number | `number_config(...)` | `0.0` | 64-bit float |
/// | String | `string_config(...)` | `""` | Single-line text |
/// | Text | `text_config(...)` | `""` | Multi-line text |
/// | Array | `array_config(...)` | `[]` | JSON array |
/// | Object | `object_config(...)` | `{}` | JSON object |
/// | Unit | `unit_config(...)` | - | Action button |
/// | Custom | `custom_config(...)` | - | Custom type with `type_ = "..."` |
///
/// ## Global Configuration
///
/// Use `*_global_config(...)` variants for configs shared across all instances
/// of this agent type (e.g., API keys).
///
/// # Example
///
/// ```rust,ignore
/// use modular_agent_core::{
///     ModularAgent, AgentContext, AgentData, AgentError, AgentSpec, AgentValue, AsAgent,
///     modular_agent, async_trait,
/// };
///
/// const PORT_INPUT: &str = "input";
/// const PORT_OUTPUT: &str = "output";
///
/// #[modular_agent(
///     title = "Add Integer",
///     category = "Math/Arithmetic",
///     description = "Adds a constant to the input value",
///     inputs = [PORT_INPUT],
///     outputs = [PORT_OUTPUT],
///     integer_config(name = "n", default = 1, title = "Add Value"),
/// )]
/// struct AddIntAgent {
///     data: AgentData,
///     n: i64,
/// }
///
/// #[async_trait]
/// impl AsAgent for AddIntAgent {
///     fn new(ma: ModularAgent, id: String, spec: AgentSpec) -> Result<Self, AgentError> {
///         let n = spec.configs.as_ref()
///             .map(|c| c.get_integer_or_default("n"))
///             .unwrap_or(1);
///         Ok(Self {
///             data: AgentData::new(ma, id, spec),
///             n,
///         })
///     }
///
///     async fn process(&mut self, ctx: AgentContext, port: String, value: AgentValue)
///         -> Result<(), AgentError>
///     {
///         if port == PORT_INPUT {
///             let result = value.as_integer().unwrap_or(0) + self.n;
///             self.output(ctx, PORT_OUTPUT.into(), AgentValue::integer(result)).await?;
///         }
///         Ok(())
///     }
/// }
/// ```
///
/// # Generated Code
///
/// The macro generates:
/// - `impl HasAgentData for StructName` - Access to agent data
/// - `StructName::DEF_NAME` - The definition name constant
/// - `StructName::def_name()` - Returns the definition name
/// - `StructName::agent_definition()` - Returns the [`AgentDefinition`]
/// - `StructName::register(ma)` - Registers with a [`ModularAgent`]
/// - Inventory submission for automatic registration
#[proc_macro_attribute]
pub fn modular_agent(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated<Meta, Comma>::parse_terminated);
    let item_struct = parse_macro_input!(item as ItemStruct);

    match expand_modular_agent(args, item_struct) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

struct AgentArgs {
    kind: Option<Expr>,
    name: Option<Expr>,
    title: Option<Expr>,
    hide_title: bool,
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
    hidden: bool,
    readonly: bool,
    detail: bool,
}

struct CustomConfig {
    name: Expr,
    default: Expr,
    type_: Expr,
    title: Option<Expr>,
    description: Option<Expr>,
    hide_title: bool,
    hidden: bool,
    readonly: bool,
    detail: bool,
}

enum ConfigSpec {
    Unit(CommonConfig),
    Boolean(CommonConfig),
    Integer(CommonConfig),
    Number(CommonConfig),
    String(CommonConfig),
    Text(CommonConfig),
    Array(CommonConfig),
    Object(CommonConfig),
    Custom(CustomConfig),
}

fn expand_modular_agent(
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
            "#[modular_agent] expects the struct to have a `data: AgentData` field",
        ));
    }

    let mut parsed = AgentArgs {
        kind: None,
        name: None,
        title: None,
        hide_title: false,
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
            Meta::Path(p) if p.is_ident("hide_title") => {
                parsed.hide_title = true;
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
            Meta::List(ml) if ml.path.is_ident("array_config") => {
                parsed
                    .configs
                    .push(ConfigSpec::Array(parse_common_config(ml)?));
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
            Meta::List(ml) if ml.path.is_ident("array_global_config") => {
                parsed
                    .global_configs
                    .push(ConfigSpec::Array(parse_common_config(ml)?));
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
                    "unsupported modular_agent argument",
                ));
            }
        }
    }

    let ident = &item.ident;
    let generics = item.generics.clone();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let data_impl = quote! {
        impl #impl_generics ::modular_agent_core::HasAgentData for #ident #ty_generics #where_clause {
            fn data(&self) -> &::modular_agent_core::AgentData {
                &self.data
            }

            fn mut_data(&mut self) -> &mut ::modular_agent_core::AgentData {
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
        .ok_or_else(|| syn::Error::new(Span::call_site(), "modular_agent: missing `title`"))?;
    let category = parsed
        .category
        .ok_or_else(|| syn::Error::new(Span::call_site(), "modular_agent: missing `category`"))?;
    let title = quote! { .title(#title) };
    let hide_title = if parsed.hide_title {
        quote! { .hide_title() }
    } else {
        quote! {}
    };
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .unit_config_with(#name, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .boolean_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .integer_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .number_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .string_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .text_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
                        entry
                    })
                })
            }
            ConfigSpec::Array(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "array_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| {
                    parse_quote! { ::modular_agent_core::AgentValue::array_default() }
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .array_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
                        entry
                    })
                })
            }
            ConfigSpec::Object(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "object_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| {
                    parse_quote! { ::modular_agent_core::AgentValue::object_default() }
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .object_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .unit_global_config_with(#name, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .boolean_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .integer_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .number_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .string_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .text_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
                        entry
                    })
                })
            }
            ConfigSpec::Array(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "array_global_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| {
                    parse_quote! { ::modular_agent_core::AgentValue::array_default() }
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .array_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
                        entry
                    })
                })
            }
            ConfigSpec::Object(c) => {
                let name = c.name.ok_or_else(|| {
                    syn::Error::new(Span::call_site(), "object_global_config missing `name`")
                })?;
                let default = c.default.unwrap_or_else(|| {
                    parse_quote! { ::modular_agent_core::AgentValue::object_default() }
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
                let hidden = if c.hidden {
                    quote! { let entry = entry.hidden(); }
                } else {
                    quote! {}
                };
                let readonly = if c.readonly {
                    quote! { let entry = entry.readonly(); }
                } else {
                    quote! {}
                };
                let detail = if c.detail {
                    quote! { let entry = entry.detail(); }
                } else {
                    quote! {}
                };
                Ok(quote! {
                    .object_global_config_with(#name, #default, |entry| {
                        let entry = entry;
                        #title
                        #description
                        #hide_title
                        #hidden
                        #readonly
                        #detail
                        entry
                    })
                })
            }
            ConfigSpec::Custom(c) => custom_config_call("custom_global_config_with", c),
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let definition_builder = quote! {
        ::modular_agent_core::AgentDefinition::new(
            #kind,
            #name_tokens,
            Some(::modular_agent_core::new_agent_boxed::<#ident>),
        )
        #title
        #hide_title
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

            pub fn agent_definition() -> ::modular_agent_core::AgentDefinition {
                #definition_builder
            }

            pub fn register(ma: &::modular_agent_core::ModularAgent) {
                ma.register_agent_definiton(Self::agent_definition());
            }
        }

        ::modular_agent_core::inventory::submit! {
            ::modular_agent_core::AgentRegistration {
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
    let mut hidden = false;
    let mut readonly = false;
    let mut detail = false;
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
            Meta::Path(p) if p.is_ident("hidden") => {
                hidden = true;
            }
            Meta::Path(p) if p.is_ident("readonly") => {
                readonly = true;
            }
            Meta::Path(p) if p.is_ident("detail") => {
                detail = true;
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "custom_config supports name, default, type/type_, title, description, hide_title, hidden, readonly, detail",
                ));
            }
        }
    }

    let name = name.ok_or_else(|| syn::Error::new(list.span(), "config missing `name`"))?;
    // Config names starting with `_` are reserved for stale key migration
    if let Expr::Lit(ref lit) = name
        && let syn::Lit::Str(ref s) = lit.lit
        && s.value().starts_with('_')
    {
        return Err(syn::Error::new_spanned(
            &name,
            "config name must not start with `_` (reserved for stale key migration)",
        ));
    }
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
        hidden,
        readonly,
        detail,
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
                // Config names starting with `_` are reserved for stale key migration
                if let Expr::Lit(ref lit) = nv.value
                    && let syn::Lit::Str(ref s) = lit.lit
                    && s.value().starts_with('_')
                {
                    return Err(syn::Error::new_spanned(
                        &nv.value,
                        "config name must not start with `_` (reserved for stale key migration)",
                    ));
                }
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
            Meta::Path(p) if p.is_ident("hidden") => {
                cfg.hidden = true;
            }
            Meta::Path(p) if p.is_ident("readonly") => {
                cfg.readonly = true;
            }
            Meta::Path(p) if p.is_ident("detail") => {
                cfg.detail = true;
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "config supports name, default, title, description, hide_title, hidden, readonly, detail",
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
        hidden,
        readonly,
        detail,
    } = cfg;
    let title = title.map(|t| quote! { let entry = entry.title(#t); });
    let description = description.map(|d| quote! { let entry = entry.description(#d); });
    let hide_title = if hide_title {
        quote! { let entry = entry.hide_title(); }
    } else {
        quote! {}
    };
    let hidden = if hidden {
        quote! { let entry = entry.hidden(); }
    } else {
        quote! {}
    };
    let readonly = if readonly {
        quote! { let entry = entry.readonly(); }
    } else {
        quote! {}
    };
    let detail = if detail {
        quote! { let entry = entry.detail(); }
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
            #hidden
            #readonly
            #detail
            entry
        })
    })
}
