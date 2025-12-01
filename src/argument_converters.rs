use anyhow::{Result, anyhow};
use twilight_model::{
    application::command::CommandOptionType,
    id::{
        Id,
        marker::{ChannelMarker, GenericMarker, RoleMarker, UserMarker},
    },
};

use crate::arguments::{ArgumentConverter, CommandOption, Error, ToOption};

use twilight_model::application::interaction::application_command::CommandOptionValue;

impl ArgumentConverter for String {
    fn convert(data: &CommandOptionValue) -> Result<Self> {
        if let CommandOptionValue::String(value) = data {
            Ok(value.clone())
        } else {
            Err(anyhow!(Error::InvalidType))
        }
    }
}

impl ToOption for String {
    fn to_option() -> CommandOption {
        CommandOption::new(CommandOptionType::String)
    }
}

// --- Numeric Types ---
macro_rules! numeric_converter {
    ($ty:ty, $variant:expr) => {
        impl ArgumentConverter for $ty {
            fn convert(data: &CommandOptionValue) -> Result<Self> {
                if let CommandOptionValue::Number(value) = data {
                    Ok(*value as $ty)
                } else {
                    Err(anyhow!(Error::InvalidType))
                }
            }
        }

        impl ToOption for $ty {
            fn to_option() -> CommandOption {
                CommandOption::new($variant)
            }
        }
    };
    ($ty:ty) => {
        numeric_converter!($ty, CommandOptionType::Number);
    };
}

// Signed types
numeric_converter!(i8, CommandOptionType::Integer);
numeric_converter!(i16, CommandOptionType::Integer);
numeric_converter!(i32, CommandOptionType::Integer);
numeric_converter!(i64, CommandOptionType::Integer);
numeric_converter!(i128, CommandOptionType::Integer);
numeric_converter!(isize, CommandOptionType::Integer);

// Unsigned types
numeric_converter!(u8, CommandOptionType::Integer);
numeric_converter!(u16, CommandOptionType::Integer);
numeric_converter!(u32, CommandOptionType::Integer);
numeric_converter!(u64, CommandOptionType::Integer);
numeric_converter!(u128, CommandOptionType::Integer);
numeric_converter!(usize, CommandOptionType::Integer);

// Floating point types
numeric_converter!(f32);
numeric_converter!(f64);

// --- Boolean Type ---
impl ArgumentConverter for bool {
    fn convert(data: &CommandOptionValue) -> Result<Self> {
        if let CommandOptionValue::Boolean(v) = data {
            Ok(*v)
        } else {
            Err(anyhow!(Error::InvalidType))
        }
    }
}

impl ToOption for bool {
    fn to_option() -> CommandOption {
        CommandOption::new(CommandOptionType::Boolean)
    }
}

// --- Char Type ---
impl ArgumentConverter for char {
    fn convert(data: &CommandOptionValue) -> Result<Self> {
        if let CommandOptionValue::String(value) = data {
            let mut chars = value.chars();
            Ok(chars.next().ok_or_else(|| anyhow!(Error::InvalidType))?)
        } else {
            Err(anyhow!(Error::InvalidType))
        }
    }
}
impl ToOption for char {
    fn to_option() -> CommandOption {
        CommandOption::new(CommandOptionType::String).max_length(1)
    }
}

// --- User ID Type ---
impl ArgumentConverter for Id<UserMarker> {
    fn convert(data: &CommandOptionValue) -> Result<Self> {
        if let CommandOptionValue::User(user) = data {
            Ok(*user)
        } else {
            Err(anyhow!(Error::InvalidType))
        }
    }
}

impl ToOption for Id<UserMarker> {
    fn to_option() -> CommandOption {
        CommandOption::new(CommandOptionType::User)
    }
}

// --- Role ID Type ---
impl ArgumentConverter for Id<RoleMarker> {
    fn convert(data: &CommandOptionValue) -> Result<Self> {
        if let CommandOptionValue::Role(role) = data {
            Ok(*role)
        } else {
            Err(anyhow!(Error::InvalidType))
        }
    }
}

impl ToOption for Id<RoleMarker> {
    fn to_option() -> CommandOption {
        CommandOption::new(CommandOptionType::Role)
    }
}

impl ArgumentConverter for Id<ChannelMarker> {
    fn convert(data: &CommandOptionValue) -> Result<Self> {
        if let CommandOptionValue::Channel(channel) = data {
            Ok(*channel)
        } else {
            Err(anyhow!(Error::InvalidType))
        }
    }
}

impl ToOption for Id<ChannelMarker> {
    fn to_option() -> CommandOption {
        // NOTE: Channel types are filtered as a part of the `command` derive macro
        CommandOption::new(CommandOptionType::Channel)
    }
}

impl ArgumentConverter for Id<GenericMarker> {
    fn convert(data: &CommandOptionValue) -> Result<Self> {
        if let CommandOptionValue::Mentionable(channel) = data {
            Ok(*channel)
        } else {
            Err(anyhow!(Error::InvalidType))
        }
    }
}

impl ToOption for Id<GenericMarker> {
    fn to_option() -> CommandOption {
        // NOTE: Mentionable can be either a user or a role
        CommandOption::new(CommandOptionType::Mentionable)
    }
}
