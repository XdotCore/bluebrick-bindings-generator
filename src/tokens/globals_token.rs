use std::fmt::{self, Debug};
use proc_macro2::TokenStream;

use crate::generator::{Result};
use crate::parser::{CharRect, Parser};
use crate::tokens::general_tokens::ListToken;
use crate::tokens::{ChildToken, FileToken, Token};

pub struct GlobalsToken {
    children: ListToken<ChildToken>,
    char_rect: CharRect,
}

impl Debug for GlobalsToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for GlobalsToken {
    fn token_type() -> &'static str { "Globals" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(Self {
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