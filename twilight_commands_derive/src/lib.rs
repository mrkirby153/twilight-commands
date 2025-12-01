use proc_macro::TokenStream;

mod choices;
mod command;

#[proc_macro_derive(Command, attributes(option, command))]
pub fn command_derive(input: TokenStream) -> TokenStream {
    command::derive(input)
}

#[proc_macro_derive(Choices, attributes(choice))]
pub fn enum_choices_derive(input: TokenStream) -> TokenStream {
    choices::derive(input)
}
