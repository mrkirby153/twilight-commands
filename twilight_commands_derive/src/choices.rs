use darling::ast::Data;
use proc_macro::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::Ident;
use syn::parse_macro_input;

use darling::FromDeriveInput;
use darling::FromVariant;

#[derive(FromDeriveInput)]
#[darling(attributes(choice), supports(enum_unit))]
struct ChoicesEnumReceiver {
    ident: Ident,
    data: Data<ChoiceVariant, ()>,
}

#[derive(FromVariant)]
#[darling(attributes(choice))]
struct ChoiceVariant {
    ident: Ident,
    #[darling(default)]
    name: Option<String>,
    #[darling(default)]
    value: Option<String>,
}

impl ChoicesEnumReceiver {
    fn variants(&self) -> Vec<&ChoiceVariant> {
        self.data.as_ref().take_enum().expect("should be an enum")
    }
}

pub fn derive(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    let receiver = match ChoicesEnumReceiver::from_derive_input(&input) {
        Ok(val) => val,
        Err(err) => return TokenStream::from(err.write_errors()),
    };

    let enum_name = &receiver.ident;
    let variants = receiver
        .variants()
        .iter()
        .map(|variant| {
            (
                variant.ident.clone(),
                variant
                    .name
                    .clone()
                    .unwrap_or_else(|| variant.ident.to_string()),
                variant
                    .value
                    .clone()
                    .unwrap_or_else(|| variant.ident.to_string()),
            )
        })
        .collect::<Vec<_>>();

    if receiver.variants().len() > 25 {
        return TokenStream::from(
            darling::Error::custom("Enums with more than 25 variants are not supported")
                .write_errors(),
        );
    }

    // Assert that all variants have unique values
    let mut seen_values = std::collections::HashSet::new();
    for (_ident, _name, value) in &variants {
        if !seen_values.insert(value) {
            return TokenStream::from(
                darling::Error::custom(format!("Duplicate choice value found: {}", value))
                    .write_errors(),
            );
        }
    }

    let command_option_choices = variants.iter().map(|(_ident, name, value)| {
        quote! {
            ::twilight_model::application::command::CommandOptionChoice {
                name: #name.to_string(),
                value: ::twilight_model::application::command::CommandOptionChoiceValue::String(#value.to_string()),
                name_localizations: None,
            }
        }
    });

    let argument_converter_matches = variants.iter().map(|(ident, _name, value)| {
        quote! {
            #value => Ok(#enum_name::#ident)
        }
    });

    quote! {
        #[automatically_derived]
        impl ::twilight_commands::arguments::ToOption for #enum_name {
            fn to_option() -> ::twilight_commands::arguments::CommandOption {
                ::twilight_commands::arguments::CommandOption::new(
                    ::twilight_model::application::command::CommandOptionType::String
                ).choices(vec![
                    #(#command_option_choices),*
                ])
            }
        }

        #[automatically_derived]
        impl ::twilight_commands::arguments::ArgumentConverter for #enum_name {
            fn convert(data: &::twilight_model::application::interaction::application_command::CommandOptionValue) -> ::anyhow::Result<Self> {
                if let ::twilight_model::application::interaction::application_command::CommandOptionValue::String(value) = data {
                    match value.as_str() {
                        #(#argument_converter_matches),*,
                        _ => Err(::anyhow::anyhow!(::twilight_commands::arguments::Error::InvalidType))
                    }
                } else {
                    Err(::anyhow::anyhow!(::twilight_commands::arguments::Error::InvalidType))
                }
            }
        }
    }
    .into()
}
