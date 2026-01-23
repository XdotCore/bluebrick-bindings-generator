use std::fmt::{self, Debug};

use proc_macro2::TokenStream;

use crate::generator::{Result, ParseResult};
use crate::parser::{CharRect, Parser};
use crate::tokens::{ChildToken, FileToken, Token};
use crate::tokens::general_tokens::{FunctionToken, ListToken, TypeToken, VariableToken};

pub struct ClassToken {
    name: String,
    generics: Option<ListToken<ClassGenericToken>>,
    children: ListToken<ChildToken>,
    char_rect: CharRect,
}

impl Debug for ClassToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generic = self.generics.as_ref().map(|_| " generic").unwrap_or_default();
        write!(f, "{}{}", self.name, generic)
    }
}

impl Token for ClassToken {
    fn token_type() -> &'static str { "Class" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(Self {
            name: parser.take_word()?,
            generics: parser.if_reserved("<", &|parser| {
                parser.token_from_ctor(&|parser| ListToken::parse_with_endings(parser, [("<", ">")]))
            })?,
            children: parser.token_from_ctor(&|parser| ListToken::parse_no_delimiter(parser))?,
            char_rect: parser.token_char_rect()?,
        })
    }
    
    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement ClassToken to_rust")
    }
}

struct ClassGenericToken {
    name: String,
    char_rect: CharRect,
}

impl Debug for ClassGenericToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Token for ClassGenericToken {
    fn token_type() -> &'static str { "Class Generic" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(Self {
            name: parser.take_word()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement GenericToken to_rust")
    }
}

pub struct FieldsToken {
    fields: ListToken<FieldToken>,
    char_rect: CharRect,
}

impl Debug for FieldsToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for FieldsToken {
    fn token_type() -> &'static str { "Fields" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        parser.expect_reserved(["="])?;
        
        Ok(FieldsToken {
            fields: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }
    
    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement FieldsToken to_rust")
    }
}

struct FieldToken {
    offset: u32,
    variable: VariableToken,
    char_rect: CharRect,
}

impl Debug for FieldToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "offset: {}", self.offset)
    }
}

impl Token for FieldToken {
    fn token_type() -> &'static str { "Field" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        let offset = parser.expect_num()?;
        parser.expect_reserved(["="])?;

        Ok(Self {
            offset,
            variable: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("FieldToken to_rust")
    }
}

pub struct ImplementsToken {
    classes: ListToken<BaseClassToken>,
    char_rect: CharRect,
}

impl Debug for ImplementsToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for ImplementsToken {
    fn token_type() -> &'static str { "Implements" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        parser.expect_reserved(["="])?;
        
        Ok(Self {
            classes: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }
    
    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement ImplementsToken to_rust")
    }
}

struct BaseClassToken {
    r#type: TypeToken,
    vtable: Option<usize>,
    char_rect: CharRect,
}

impl Debug for BaseClassToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.vtable {
            Some(vtable) => write!(f, "vtable: {}", vtable),
            None => write!(f, "()"),
        }
    }
}

impl Token for BaseClassToken {
    fn token_type() -> &'static str { "Base Class" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(Self {
            r#type: parser.token()?,
            vtable: parser.if_reserved("=", &|parser| {
                parser.drop_reserved()?;
                Ok(parser.expect_num()?)
            })?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement BaseClassToken")
    }
}

pub struct OverridesToken {
    classes: ListToken<ClassOverrideToken>,
    char_rect: CharRect,
}

impl Debug for OverridesToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for OverridesToken {
    fn token_type() -> &'static str { "Overrides" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        parser.expect_reserved(["="])?;
        
        Ok(Self {
            classes: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }
    
    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement OverridesToken to_rust")
    }
}

struct ClassOverrideToken {
    class: TypeToken,
    functions: ListToken<FunctionOverrideToken>,
    char_rect: CharRect,
}

impl Debug for ClassOverrideToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for ClassOverrideToken {
    fn token_type() -> &'static str { "Class Override" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        let class = TypeToken::parse(parser)?;
        parser.expect_reserved(["="])?;
        
        Ok(Self {
            class,
            functions: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement ClassOverrideToken to_rust")
    }
}

struct FunctionOverrideToken {
    name: String,
    char_rect: CharRect,
}

impl Debug for FunctionOverrideToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Token for FunctionOverrideToken {
    fn token_type() -> &'static str { "Function Override" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(Self {
            name: parser.take_word()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement FunctionOverrideToken to_rust")
    }
}

pub struct VirtualsToken {
    fields: ListToken<VirtualToken>,
    char_rect: CharRect,
}

impl Debug for VirtualsToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for VirtualsToken {
    fn token_type() -> &'static str { "Virtuals" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        parser.expect_reserved(["="])?;

        Ok(VirtualsToken {
            fields: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }
    
    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement VirtualsToken to_rust")
    }
}

struct VirtualToken {
    index: u32,
    function: FunctionToken,
    char_rect: CharRect,
}

impl Debug for VirtualToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "index: {}", self.index)
    }
}

impl Token for VirtualToken {
    fn token_type() -> &'static str { "Virtual" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        let index = parser.expect_num::<u32>()?;
        parser.expect_reserved(["="])?;

        Ok(Self {
            index,
            function: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement VirtualToken to_rust")
    }
}