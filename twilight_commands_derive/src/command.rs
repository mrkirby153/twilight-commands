use anyhow::Result;
use darling::FromField;
use darling::util::PathList;
use darling::{FromDeriveInput, ast::Data};
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::parse_macro_input;
use syn::{AngleBracketedGenericArguments, GenericArgument, PathArguments, Type};
use thiserror::Error;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(command), supports(struct_named, struct_unit))]
struct CommandReceiver {
    ident: syn::Ident,
    data: Data<(), OptionReceiver>,
    name: String,
    #[darling(default)]
    description: Option<String>,
}

#[derive(Debug, FromField)]
#[darling(attributes(option))]
struct OptionReceiver {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    /// Override the name of the command option
    #[darling(default)]
    name: Option<String>,
    /// Set the description of the command option
    #[darling(default)]
    description: Option<String>,
    /// For channel options, restrict to specific channel types
    #[darling(default)]
    channel_types: Option<PathList>,
}

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let receiver = match CommandReceiver::from_derive_input(&input) {
        Ok(r) => r,
        Err(e) => return e.write_errors().into(),
    };

    let fields = receiver
        .data
        .take_struct()
        .expect("only structs are supported")
        .fields;
    let options = fields
        .iter()
        .map(field_option)
        .collect::<Vec<proc_macro2::TokenStream>>();

    let field_names: Result<Vec<(String, Ident)>> = fields.iter().map(field_name).collect();

    let field_names = match field_names {
        Ok(names) => names,
        Err(e) => return darling::Error::custom(e.to_string()).write_errors().into(),
    };
    let ident = receiver.ident;

    let struct_fields = field_names.iter().map(|(name, field_ident)| {
        quote! {
            #field_ident: ::twilight_commands::arguments::parse(&options_map, #name)?
        }
    });

    let description = if let Some(desc) = &receiver.description {
        desc.as_str()
    } else {
        "No description provided"
    };

    let command_name = &receiver.name;
    let option_map_ast = if fields.is_empty() {
        quote! {}
    } else {
        quote! {
            let options_map = options
                .iter()
                .map(|opt| (opt.name.clone(), opt.value.clone()))
                .collect::<::std::collections::HashMap<_, _>>();
        }
    };

    quote! {
        #[automatically_derived]
        impl ::twilight_commands::commands::Command for #ident {
            fn options() -> Vec<::twilight_commands::arguments::CommandOption> {
                use ::twilight_commands::arguments::ToOption;
                vec![
                    #(#options),*
                ]
            }

            fn name() -> &'static str {
                #command_name
            }

            fn description() -> &'static str {
                #description
            }

            fn from_command_data(options: Vec<::twilight_model::application::interaction::application_command::CommandDataOption>) -> anyhow::Result<Self> {
                #option_map_ast
                Ok(Self {
                    #(#struct_fields,)*
                })
            }
        }
    }
    .into()
}

fn field_option(field: &OptionReceiver) -> proc_macro2::TokenStream {
    // Assert that either field_name_override or field_name is Some
    let name = match get_name(field) {
        Ok(name) => name,
        Err(e) => return e.to_compile_error(),
    };
    let default_description = "No description provided".to_string();
    let description = field.description.as_ref().unwrap_or(&default_description);
    let ty = &field.ty;

    if field.channel_types.is_some() && !validate_channel_type(ty) {
        return darling::Error::custom(
            "channel_types can only be specified for fields of type Id<ChannelMarker>",
        )
        .write_errors();
    }

    let ty = add_turbofish(ty);

    let channel_types = field.channel_types.as_ref().map(|types| {
        let types = types
            .iter()
            .map(|path| quote! { ::twilight_model::channel::ChannelType::#path });
        quote! {
            vec![#(#types),*]
        }
    });

    match channel_types {
        Some(channel_types) => {
            quote! {
                #ty::to_option().name(#name).description(#description).channel_types(#channel_types)
            }
        }
        None => {
            quote! {
                #ty::to_option().name(#name).description(#description)
            }
        }
    }
}

#[derive(Error, Debug)]
enum FieldNameError {
    #[error(transparent)]
    GetNameError(#[from] GetNameError),
    #[error("Field is missing an identifier")]
    MissingIdent,
}

fn field_name(field: &OptionReceiver) -> Result<(String, Ident)> {
    let name = get_name(field)?;
    let ident = field
        .ident
        .as_ref()
        .ok_or(FieldNameError::MissingIdent)?
        .clone();
    Ok((name, ident))
}

#[derive(Error, Debug)]
enum GetNameError {
    #[error("Unable to determine field name for option")]
    MissingFieldName,
}

/// Gets the name of an `OptionReceiver`
fn get_name(field: &OptionReceiver) -> Result<String, GetNameError> {
    if let Some(name) = &field.name {
        Ok(name.clone())
    } else if let Some(ident) = &field.ident {
        Ok(ident.to_string())
    } else {
        Err(GetNameError::MissingFieldName)
    }
}

fn add_turbofish(ty: &Type) -> proc_macro2::TokenStream {
    match ty {
        Type::Path(type_path) => {
            let path = &type_path.path;
            let segments = path.segments.iter().map(|segment| {
                let ident = &segment.ident;
                match &segment.arguments {
                    PathArguments::None => quote! { #ident },
                    PathArguments::AngleBracketed(args) => {
                        // Add turbofish for angle bracketed generics
                        let args = transform_generic_arguments(args);
                        quote! { #ident::#args }
                    }
                    PathArguments::Parenthesized(args) => {
                        // Keep parenthesized as-is (for Fn traits)
                        quote! { #ident #args }
                    }
                }
            });

            if path.leading_colon.is_some() {
                quote! { ::#(#segments)::* }
            } else {
                quote! { #(#segments)::* }
            }
        }
        _ => quote! { #ty }, // For other type variants, leave as-is
    }
}

fn transform_generic_arguments(args: &AngleBracketedGenericArguments) -> proc_macro2::TokenStream {
    let args = args.args.iter().map(|arg| {
        match arg {
            GenericArgument::Type(ty) => {
                // Recursively handle nested types
                add_turbofish(ty)
            }
            // Handle other variants as needed
            _ => quote! { #arg },
        }
    });

    quote! { <#(#args),*> }
}

fn validate_channel_type(type_: &Type) -> bool {
    match type_ {
        Type::Path(type_path) => {
            let path = &type_path.path;
            if let Some(segment) = path.segments.last()
                && segment.ident == "Id"
                && let PathArguments::AngleBracketed(args) = &segment.arguments
                && let Some(GenericArgument::Type(Type::Path(inner_type_path))) = args.args.first()
                && let Some(inner_segment) = inner_type_path.path.segments.last()
            {
                return inner_segment.ident == "ChannelMarker";
            }
            false
        }
        _ => false,
    }
}
impl GetNameError {
    fn to_compile_error(&self) -> proc_macro2::TokenStream {
        let message = self.to_string();
        darling::Error::custom(message).write_errors()
    }
}
