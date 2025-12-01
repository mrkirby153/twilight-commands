use anyhow::Result;
use darling::FromField;
use darling::{FromDeriveInput, ast::Data};
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{GenericArgument, PathArguments, Type, parse_macro_input};
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
    #[darling(default)]
    name: Option<String>,
    #[darling(default)]
    description: Option<String>,
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
    let ty = match get_inner_option_type(&field.ty) {
        Some(inner) => quote! {
            Option::<#inner>
        },
        None => quote! {
            #ty
        },
    };

    quote! {
        #ty::to_option().name(#name).description(#description)
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

/// Returns the inner option type or `None` if this is not an option
fn get_inner_option_type(ty: &syn::Type) -> Option<&syn::Type> {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Option"
        && let PathArguments::AngleBracketed(angle_bracketed) = &segment.arguments
        && let Some(GenericArgument::Type(inner_type)) = angle_bracketed.args.first()
    {
        return Some(inner_type);
    }
    None
}

impl GetNameError {
    fn to_compile_error(&self) -> proc_macro2::TokenStream {
        let message = self.to_string();
        darling::Error::custom(message).write_errors()
    }
}
