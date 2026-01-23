use std::fmt::{self, Debug};

use proc_macro2::TokenStream;

use crate::generator::Result;
use crate::parser::{CharRect, Parser};
use crate::tokens::{ChildToken, FileToken, Token};
use crate::tokens::general_tokens::{ListToken, TypeToken};

pub struct EnumToken {
    name: String,
    children: ListToken<ChildToken>,
    char_rect: CharRect,
}

impl Debug for EnumToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Token for EnumToken {
    fn token_type() -> &'static str { "Enum" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(Self {
            name: parser.take_word()?,
            children: parser.token_from_ctor(&|parser| ListToken::parse_no_delimiter(parser))?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement EnumToken to_rust")
    }
}

pub struct EnumTypeToken {
    r#type: TypeToken,
    char_rect: CharRect,
}

impl Debug for EnumTypeToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for EnumTypeToken {
    fn token_type() -> &'static str { "Enum Type" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        parser.expect_reserved(["="])?;

        Ok(Self {
            r#type: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement EnumTypeToken to_rust")
    }
}

pub struct EnumValuesToken {
    values: ListToken<EnumValueToken>,
    char_rect: CharRect,
}

impl Debug for EnumValuesToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for EnumValuesToken {
    fn token_type() -> &'static str { "Enum Values" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        parser.expect_reserved(["="])?;
        
        Ok(Self {
            values: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement ValuesToken to_rust")
    }
}

struct EnumValueToken {
    name: String,
    value: String,
    char_rect: CharRect,
}

impl Debug for EnumValueToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, value: {}", self.name, self.value)
    }
}

impl Token for EnumValueToken {
    fn token_type() -> &'static str { "Enum Value" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        let name = parser.take_word()?;
        parser.expect_reserved(["="])?;

        Ok(Self {
            name,
            value: parser.take_word()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement ValueToken to_rust")
    }
}
