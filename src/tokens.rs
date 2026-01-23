mod class_tokens;
mod enum_tokens;
mod general_tokens;
mod globals_token;

use proc_macro2::TokenStream;
use std::fmt::Debug;

use crate::generator::{ParseResult, Result, ValidateError, ValidateResult};
use crate::parser::{CharRect, Parser};
use crate::tokens::class_tokens::{ClassToken, FieldsToken, ImplementsToken, OverridesToken, VirtualsToken};
use crate::tokens::enum_tokens::{EnumToken, EnumTypeToken, EnumValuesToken};
use crate::tokens::general_tokens::{FunctionsToken, ModuleToken};
use crate::tokens::globals_token::GlobalsToken;

pub trait Token : Debug {
    fn token_type() -> &'static str where Self: Sized;
    fn parse(parser: &mut Parser) -> Result<Self> where Self: Sized;
    fn char_rect(&self) -> CharRect;
    fn to_rust(&self) -> Result<TokenStream>;
}

pub enum ChildToken {
    Fields(FieldsToken),
    Implements(ImplementsToken),
    Overrides(OverridesToken),
    Virtuals(VirtualsToken),
    EnumType(EnumTypeToken),
    Values(EnumValuesToken),
    Functions(FunctionsToken),
}

impl ChildToken {
    fn get_inner(&self) -> &dyn Token {
        match self {
            Self::Fields(fields) => fields,
            Self::Implements(implements) => implements,
            Self::Overrides(overrides) => overrides,
            Self::Virtuals(virtuals) => virtuals,
            Self::EnumType(enumtype) => enumtype,
            Self::Values(values) => values,
            Self::Functions(functions) => functions,
        }
    }
}

impl Debug for ChildToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get_inner().fmt(f)
    }
}

impl Token for ChildToken {
    fn token_type() -> &'static str { "Child Token" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(match parser.expect_word(&["fields", "implements", "overrides", "virtuals", "type", "values", "functions"])?.as_str() {
            "fields" => Self::Fields(parser.token()?),
            "implements" => Self::Implements(parser.token()?),
            "overrides" => Self::Overrides(parser.token()?),
            "virtuals" => Self::Virtuals(parser.token()?),
            "type" => Self::EnumType(parser.token()?),
            "values" => Self::Values(parser.token()?),
            "functions" => Self::Functions(parser.token()?),
            child_type => todo!("Implement child type: {}", child_type),
        })
    }

    fn char_rect(&self) -> CharRect {
        self.get_inner().char_rect()
    }

    fn to_rust(&self) -> Result<TokenStream> {
        self.get_inner().to_rust()
    }
}

pub enum RootToken {
    Module(ModuleToken),
    UseModule(ModuleToken),
    Class(ClassToken),
    Enum(EnumToken),
    Globals(GlobalsToken),
}

impl RootToken {
    fn get_inner(&self) -> &dyn Token {
        match self {
            Self::Module(module) => module,
            Self::UseModule(usemodule) => usemodule,
            Self::Class(class) => class,
            Self::Enum(r#enum) => r#enum,
            Self::Globals(globals) => globals,
        }
    }
}

impl Debug for RootToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.get_inner().fmt(f)
    }
}

impl Token for RootToken {
    fn token_type() -> &'static str { "Root Token" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(match parser.expect_word(&["mod", "use", "class", "enum", "globals"])?.as_str() {
            "mod" => Self::Module(parser.token()?),
            "use" => Self::UseModule(parser.token()?),
            "class" => Self::Class(parser.token()?),
            "enum" => Self::Enum(parser.token()?),
            "globals" => Self::Globals(parser.token()?),
            file_type => todo!("Implement file root type: {}", file_type),
        })
    }

    fn char_rect(&self) -> CharRect {
        self.get_inner().char_rect()
    }

    fn to_rust(&self) -> Result<TokenStream> {
        self.get_inner().to_rust()
    }
}

pub struct FileToken {
    pub name: String,
    pub children: Vec<RootToken>,
    char_rect: CharRect,
}

impl FileToken {
    pub fn module(&self) -> crate::generator::Result<&ModuleToken> {
        match self.children.iter().find_map(|child| {
            match child {
                RootToken::Module(module) => Some(module),
                _ => None,
            }
        }) {
            Some(module) => Ok(module),
            None => Err(crate::generator::Error {
                file_name: self.name.clone(),
                char_rect: self.char_rect(),
                err: ValidateError::MissingModule { file_name: self.name.clone() }.into(),
            }),
        }
    }
}

impl Debug for FileToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Token for FileToken {
    fn token_type() -> &'static str { "File" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(Self {
            name: parser.file.name.clone(),
            children: parser.all_tokens()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        unimplemented!("Use children")
    }
}

pub trait FromWord {
    fn from_word(word: &str) -> ParseResult<Self> where Self: Sized;
}
