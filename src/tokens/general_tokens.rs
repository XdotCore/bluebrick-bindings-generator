use std::fmt::{self, Debug, Display};

use itertools::Itertools;
use proc_macro2::TokenStream;

use crate::generator::{ParseError, ParseResult, Result};
use crate::{parser::{CharRect, Parser}};
use crate::tokens::{FromWord, Token};

#[derive(PartialEq)]
pub struct NameToken {
    name: String,
    char_rect: CharRect,
}

impl Display for NameToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Debug for NameToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self, f)
    }
}

impl Token for NameToken {
    fn token_type() -> &'static str { "Name" }

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
        todo!("Implement NameToken to_rust")
    }
}

#[derive(PartialEq)]
pub struct ModuleToken {
    pub name: String,
    parents: ListToken<NameToken>,
    char_rect: CharRect,
}

impl ModuleToken {
    pub fn path(&self) -> String {
        self.parents.items.iter().join("/")
    }
}

impl Debug for ModuleToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path())
    }
}

impl Token for ModuleToken {
    fn token_type() -> &'static str { "Module" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        let mut module_parts = parser.token_from_ctor(&|parser| ListToken::<NameToken>::parse_no_endings_with_delimiter(parser, "::"))?;
        let name = module_parts.items.pop().ok_or_else(|| parser.make_error(ParseError::EmptyModule))?.name;

        Ok(ModuleToken {
            name,
            parents: module_parts,
            char_rect: parser.token_char_rect()?,
        })
    }
    
    fn char_rect(&self) -> CharRect {
        self.char_rect
    }
    
    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement ModuleToken to_rust")
    }
}


pub struct ListToken<T> {
    items: Vec<T>,
    char_rect: CharRect,
}

impl<T> Debug for ListToken<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "count: {}", self.items.len())
    }
}

impl<T: PartialEq> PartialEq for ListToken<T> {
    fn eq(&self, other: &Self) -> bool {
        for (item_self, item_other) in self.items.iter().zip(&other.items) {
            if item_self != item_other {
                return false;
            }
        }
        return true;
    }
}

impl<T: Token + Debug> ListToken<T> {
    fn parse_with_full<'i, I>(parser: &mut Parser, endings: I, delimiter: Option<&str>) -> Result<Self> where I: IntoIterator<Item = (&'i str, &'i str)> {
        let endings = endings.into_iter().collect_vec();
        let closing = match endings.len() {
            0 => None,
            _ => {
                let openings = endings.iter().map(|(o, _)| *o);
                let opening = parser.expect_reserved(openings)?;
                endings.iter().find(|(o, _)| *o == opening).map(|(_, c)| *c)
            }
        };

        // return early if empty
        if let Some(closing) = closing {
            if parser.peek_reserved().as_ref().is_some_and(|next| *next == closing) {
                parser.drop_reserved()?;
                return Ok(ListToken { 
                    items: Vec::new(),
                    char_rect: parser.token_char_rect()?,
                });
            }
        }

        let mut items = Vec::new();
        loop {
            items.push(parser.token()?);

            let mut next = parser.peek_reserved();

            let had_delimiter = if let Some(delimiter) = delimiter {
                if next.as_ref().is_some_and(|next| next == delimiter) {
                    parser.drop_reserved()?;
                    next = parser.peek_reserved();
                    true
                } else if closing.is_none() {
                    break;
                } else {
                    false
                }
            } else {
                false
            };

            if let Some(closing) = closing {
                if next.as_ref().is_some_and(|next| *next == closing) {
                    parser.drop_reserved()?;
                    break;
                }
            }

            if let Some(delimiter) = delimiter {
                if !had_delimiter {
                    return Err(parser.make_error(ParseError::ExpectedDelimiter(delimiter.to_owned())))
                }
            }
        }

        Ok(ListToken {
            items,
            char_rect: parser.token_char_rect()?,
        })
    }

    pub fn parse_with_endings<'i, I>(parser: &mut Parser, endings: I) -> Result<Self> where I: IntoIterator<Item = (&'i str, &'i str)> {
        Self::parse_with_full(parser, endings, Some(","))
    }

    pub fn parse_no_endings_with_delimiter(parser: &mut Parser, delimiter: &str) -> Result<Self> {
        Self::parse_with_full(parser, [], Some(delimiter))
    }

    pub fn parse_no_delimiter(parser: &mut Parser) -> Result<Self> {
        Self::parse_with_full(parser, [("[", "]"), ("{", "}")], None)
    }
}

impl<T: Token + Debug> Token for ListToken<T> {
    fn token_type() -> &'static str { "List" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Self::parse_with_full(parser, [("[", "]"), ("{", "}")], Some(","))
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("ListToken to_rust")
    }
}

// TODO: implement function types
enum TypeTokenVariant {
    Array {
        array_len: usize,
        sub_type: Box<TypeToken>,
    },
    Pointer {
        sub_type: Box<TypeToken>,
    },
    Type {
        name: String,
        generics: Option<ListToken<TypeToken>>,
    }
}
pub struct TypeToken {
    variant: TypeTokenVariant,
    char_rect: CharRect,
}

impl TypeToken {
    fn print(&self) {
        match &self.variant {
            TypeTokenVariant::Array { array_len, sub_type, .. } => {
                print!("array {} ", array_len);
                sub_type.print();
            }
            TypeTokenVariant::Pointer { sub_type, .. } => {
                print!("ptr ");
                sub_type.print();
            }
            TypeTokenVariant::Type { name, generics, .. } => {
                print!("{}", name);
                if let Some(generics) = generics {
                    for generic in &generics.items {
                        generic.print();
                    }
                }
            }
        }
    }
}

impl Debug for TypeToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.variant {
            TypeTokenVariant::Array { array_len, .. } => {
                write!(f, "array, length: {}", array_len)
            }
            TypeTokenVariant::Pointer { .. } => {
                write!(f, "pointer")
            }
            TypeTokenVariant::Type { name, generics, .. } => {
                let generic = generics.as_ref().map(|_| " generic").unwrap_or_default();
                write!(f, "{name}{generic}")
            }
        }
    }
}

impl Token for TypeToken {
    fn token_type() -> &'static str { "Type" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        Ok(match parser.take_word()?.as_str() {
            "arr" => {
                Self {
                    variant: TypeTokenVariant::Array {
                        array_len: parser.expect_num()?,
                        sub_type: Box::new(parser.token()?),
                    },
                    char_rect: parser.token_char_rect()?,
                }
            }
            "ptr" => {
                Self {
                    variant: TypeTokenVariant::Pointer {
                        sub_type: Box::new(parser.token()?),
                    },
                    char_rect: parser.token_char_rect()?,
                }
            }
            name => {
                Self {
                    variant: TypeTokenVariant::Type {
                        name: name.to_owned(),
                        generics: parser.if_reserved("<", &|parser| {
                            parser.token_from_ctor(&|parser| ListToken::parse_with_endings(parser, [("<", ">")]))
                        })?,
                    },
                    char_rect: parser.token_char_rect()?,
                }
            },
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("TypeToken to_rust")
    }
}
pub struct VariableToken {
    name: String,
    r#type: TypeToken,
    char_rect: CharRect,
}

impl Debug for VariableToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl Token for VariableToken {
    fn token_type() -> &'static str { "Variable" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        let name = parser.take_word()?;
        parser.expect_reserved([":"])?;
        
        Ok(Self {
            name,
            r#type: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("VariableToken to_rust")
    }
}

pub struct FunctionsToken {
    functions: ListToken<NonVirtualFunctionToken>,
    char_rect: CharRect,
}

impl Debug for FunctionsToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "()")
    }
}

impl Token for FunctionsToken {
    fn token_type() -> &'static str { "Functions" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        parser.expect_reserved(["="])?;

        Ok(Self {
            functions: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement FunctionsToken to_rust")
    }
}

// TODO: make more elegant name
struct NonVirtualFunctionToken {
    function: FunctionToken,
    ptr: usize,
    char_rect: CharRect,
}

impl Debug for NonVirtualFunctionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ptr: {}", self.ptr)
    }
}

impl Token for NonVirtualFunctionToken {
    fn token_type() -> &'static str { "Non-virtual Function" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        let function = FunctionToken::parse(parser)?;
        parser.expect_reserved(["="])?;

        Ok(Self {
            function,
            ptr: parser.expect_num()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement NonVirtualFunction to_rust")
    }
}

#[derive(Debug)]
enum CallingConvention {
    Cdecl,
    Stdcall,
    Fastcall,
    Thiscall,
}

impl FromWord for CallingConvention {
    fn from_word(word: &str) -> ParseResult<Self> {
        match word {
            "cdecl" => Ok(CallingConvention::Cdecl),
            "stdcall" => Ok(CallingConvention::Stdcall),
            "fastcall" => Ok(CallingConvention::Fastcall),
            "thiscall"  => Ok(CallingConvention::Thiscall),
            _ => Err(ParseError::InvalidCallConv(word.to_owned())),
        }
    }
}

pub struct FunctionToken {
    name: String,
    calling_convention: CallingConvention,
    parameters: ListToken<VariableToken>,
    return_type: TypeToken,
    char_rect: CharRect,
}

impl Debug for FunctionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, calling convention: {:?}", self.name, self.calling_convention)
    }
}

impl FunctionToken {
    fn print(&self) {
        print!("{}: {:?} (", self.name, self.calling_convention);

        let mut it = self.parameters.items.iter().peekable();
        while let Some(param) = it.next() {
            print!("{}: ", param.name);
            param.r#type.print();
            if it.peek().is_some() {
                print!(", ");
            }
        }

        print!(") ");
        self.return_type.print();
        println!("");
    }
}

impl Token for FunctionToken {
    fn token_type() -> &'static str { "Function" }

    fn parse(parser: &mut Parser) -> Result<Self> {
        let name = parser.take_word()?;
        parser.expect_reserved([":"])?;

        Ok(Self {
            name,
            calling_convention: parser.from_word()?,
            parameters: parser.token_from_ctor(&|parser| ListToken::parse_with_endings(parser, [("(", ")")]))?,
            return_type: parser.token()?,
            char_rect: parser.token_char_rect()?,
        })
    }

    fn char_rect(&self) -> CharRect {
        self.char_rect
    }

    fn to_rust(&self) -> Result<TokenStream> {
        todo!("Implement FunctionToken to_rust")
    }
}
