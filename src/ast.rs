use std::{fmt::Display, ops::{Deref, DerefMut}};

use itertools::Itertools;

#[derive(Debug, Default, Clone)]
pub struct Span {
    pub file: String,
    pub input: String,
    pub start: (usize, usize),
    pub end: (usize, usize),
}

impl Span {
    pub fn after_end(&self) -> Self {
        let mut end = self.end;
        end.1 += 1;

        Self {
            file: self.file.clone(),
            start: end,
            end,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct WithSpan<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Deref for WithSpan<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for WithSpan<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T: Display> Display for WithSpan<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)
    }
}

#[derive(Debug)]
pub struct File {
    pub name: String,
    pub module: Module,
    pub usings: Vec<Module>,
    pub classes: Vec<Class>,
    pub enums: Vec<Enum>,
    pub global_variables: Vec<Variable>,
    pub global_functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub names: Vec<WithSpan<String>>,
    pub span: Span,
}

impl Display for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.names.iter().format("::").to_string())
    }
}

impl Module {
    pub fn get_names(&self) -> Vec<String> {
        self.names.iter()
            .map(|w| w.value.clone())
            .collect()
    }
}

#[derive(Debug)]
pub struct Class {
    pub name: WithSpan<String>,
    pub generics: Option<GenericParams>,
    pub size: Option<WithSpan<usize>>,
    pub fields: Vec<Variable>,
    pub methods: Vec<Function>,
    pub bases: Vec<ClassBase>,
    pub static_fields: Vec<Variable>,
    pub static_methods: Vec<Function>,
    pub span: Span,
}

// TODO: implement const args?
#[derive(Debug, Clone)]
pub struct GenericParams {
    pub names: Vec<WithSpan<String>>,
    pub span: Span,
}

impl Display for GenericParams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}>", self.names.iter().map(ToString::to_string).intersperse(", ".to_owned()).collect::<String>())
    }
}

#[derive(Debug, Clone)]
pub struct GenericArgs {
    pub types: Vec<Type>,
    pub span: Span,
}

impl Display for GenericArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<{}>", self.types.iter().map(ToString::to_string).intersperse(", ".to_owned()).collect::<String>())
    }
}

#[derive(Debug)]
pub struct Variable {
    pub offset: WithSpan<usize>,
    pub name: WithSpan<String>,
    pub r#type: Type,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Type {
    Array(ArrayType),
    Pointer(PointerType),
    Function(FunctionType),
    Named(NamedType),
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Array(array) => Display::fmt(array, f),
            Self::Pointer(pointer) => Display::fmt(pointer, f),
            Self::Function(function) => Display::fmt(function, f),
            Self::Named(named) => Display::fmt(named, f),
        }
    }
}

impl Type {
    pub fn get_span(&self) -> &Span {
        match self {
            Self::Array(array) => &array.span,
            Self::Pointer(pointer) => &pointer.span,
            Self::Function(function) => &function.span,
            Self::Named(named) => &named.span,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ArrayType {
    pub count: WithSpan<usize>,
    pub item_type: Box<Type>,
    pub span: Span,
}

impl Display for ArrayType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "arr {} {}", self.count, self.item_type)
    }
}

#[derive(Debug, Clone)]
pub struct PointerType {
    pub inner_type: Box<Type>,
    pub span: Span,
}

impl Display for PointerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ptr {}", self.inner_type)
    }
}

#[derive(Debug, Clone)]
pub struct FunctionType {
    pub call_conv: Option<WithSpan<String>>,
    pub generics: Option<GenericParams>,
    pub args: Vec<FunctionArg>,
    pub ret: Option<Box<Type>>,
    pub span: Span,
}

impl Display for FunctionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fn")?;
        if let Some(generics) = &self.generics {
            write!(f, "{generics}")?;
        }
        write!(f, " ({})", self.args.iter().map(ToString::to_string).intersperse(", ".to_owned()).collect::<String>())?;
        if let Some(ret) = &self.ret {
            write!(f, " -> {ret}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FunctionArg {
    pub name: WithSpan<String>,
    pub r#type: Type,
    pub span: Span,
}

impl Display for FunctionArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.r#type)
    }
}

#[derive(Debug, Clone)]
pub struct NamedType {
    pub name: Module,
    pub generics: Option<GenericArgs>,
    pub span: Span,
}

impl Display for NamedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(generics) = &self.generics {
            write!(f, "{generics}")?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Function {
    pub pointer: WithSpan<usize>,
    pub name: WithSpan<String>,
    pub r#type: FunctionType,
    pub span: Span,
}

#[derive(Debug)]
pub struct ClassBase {
    pub offset: WithSpan<usize>,
    pub r#type: NamedType,
    pub vtable: Option<WithSpan<usize>>,
    pub overrides: Vec<Override>,
    pub virtuals: Vec<Virtual>,
    pub span: Span,
}

// TODO: generics? is that possible?
#[derive(Debug)]
pub struct Override {
    pub pointer: WithSpan<usize>,
    pub name: WithSpan<String>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Virtual {
    pub index: WithSpan<usize>,
    pub function: Function,
    pub span: Span,
}

#[derive(Debug)]
pub struct Enum {
    pub name: WithSpan<String>,
    pub r#type: WithSpan<String>, // only int types
    pub values: Vec<EnumValue>,
    pub span: Span,
}

#[derive(Debug)]
pub struct EnumValue {
    pub value: WithSpan<usize>,
    pub name: WithSpan<String>,
    pub span: Span,
}
