use std::{fmt::Display, num::ParseIntError, ops::Deref, ptr};

use itertools::Itertools;

use crate::{ast::NamedType, parser::Rule, typelib::ModuleName};

#[derive(Debug)]
pub enum Error {
    PestError(pest::error::Error<Rule>),
    UnexpectedTopLevelBlock { kind: String },
    UnexpectedRule { expected: Rule, found: Rule },
    MissingModule,
    MissingUses,
    MissingTopLevelBlocks,
    NoCommaBlock,
    UnexpectedComma,
    ExpectedComma,
    ParseInt(ParseIntError),
    UnexpectedName,
    UnexpectedGenerics,
    MissingNameForBlock { kind: String },
    UnknownRelationItem { block_kind: String, kind: String },
    DuplicateProperty { kind: String },
    ExpectedTypeKind { kind: String },
    MissingEnumType,
    AmbiguousModule { name: ModuleName, possible: Vec<ModuleName> },
    AmbiguousType { name: NamedType, possible: Vec<ModuleName> },
    UnknownType { r#type: NamedType },
    UnknownCallConv { name: String },
    Unexpected(String),
}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Error::PestError(e)                             => format!("Pest Error {e}."),
            Error::UnexpectedTopLevelBlock { kind }         => format!("Unexpected top-level block \"{kind}\"."),
            Error::UnexpectedRule { expected, found }       => format!("Expected {expected:?} but found {found:?}."),
            Error::MissingModule                            => format!("Missing module declaration at beginning of file."),
            Error::MissingUses                              => format!("They'll never believe you..."),
            Error::MissingTopLevelBlocks                    => format!("Help will never come..."),
            Error::NoCommaBlock                             => format!("This block does not use commas."),
            Error::UnexpectedComma                          => format!("Unexpected comma."),
            Error::ExpectedComma                            => format!("Expected comma here."),
            Error::ParseInt(e)                              => format!("Failed to parse integer because: {e}."),
            Error::UnexpectedName                           => format!("Unexpected name on item."),
            Error::UnexpectedGenerics                       => format!("Unexpected generics on item."),
            Error::MissingNameForBlock { kind }             => format!("Expected name for {kind}."),
            Error::UnknownRelationItem { block_kind, kind } => format!("Unknown item {kind} in {block_kind} block."),
            Error::DuplicateProperty { kind }               => format!("Duplicate property {kind}."),
            Error::ExpectedTypeKind { kind }                => format!("Expected a {kind} type."),
            Error::MissingEnumType                          => format!("Enum needs a type (i32, u8, etc.)."),
            Error::AmbiguousModule { name, possible }       => format!("Module {name} is ambiguous between [ {} ].", possible.iter().map(ToString::to_string).intersperse(", ".to_owned()).collect::<String>()),
            Error::AmbiguousType { name, possible }         => format!("Type {name} is ambiguous between modules [ {} ].", possible.iter().map(ToString::to_string).intersperse(", ".to_owned()).collect::<String>()),
            Error::UnknownType { r#type }                   => format!("Unknown type {type}."),
            Error::UnknownCallConv { name }                 => format!("Unknown calling convention {name}."),
            Error::Unexpected(msg)                          => format!("Generator bug: {msg}"),
        })
    }
}
