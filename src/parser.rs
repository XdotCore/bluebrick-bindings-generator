use std::{fs, path::Path};

use pest::{Parser, iterators::{Pair, Pairs}};
use pest_derive::Parser;
use walkdir::WalkDir;

use crate::ast::*;
use crate::error::Error;
use crate::result::Result;

#[derive(Parser)]
#[grammar = "grammar.pest"]
pub struct BBBParser;

pub fn parse(root: &Path) -> std::result::Result<Vec<File>, String> {
    WalkDir::new(root).into_iter().filter_map(|entry| {
        let entry = entry.ok()?;
        let path = entry.path();

        let ext = path.extension()?.to_str()?;
        if ext != "bb" {
            return None;
        }
        
        let name = path.file_stem()?.to_string_lossy().to_string();
        let contents = fs::read_to_string(path).ok()?;
        let path = path.strip_prefix(root).ok()?.to_owned();
        
        Some((path, name, contents))
    }).map(|(path, name, contents)| {
        println!("{}", path.display());

        let mut ctx = Ctx::new(path.to_string_lossy().to_string());
        parse_file(&mut ctx, name, contents)
            .map_err(|_| ctx.to_string())
    }).collect()
}

fn parse_file(ctx: &mut Ctx, name: String, source: String) -> Result<File> {
    let pairs = match BBBParser::parse(Rule::File, &source) {
        Ok(pairs) => pairs,
        Err(e) => {
            let line_col = e.line_col.clone();
            ctx.add_err(Error::PestError(e), ctx.span_from_linecol(line_col));
            return Err(());
        }
    };

    let module = parse_required_tag::<ModExpr>(ctx, &pairs,
        "mod", Error::MissingModule, &ctx.span_at_start())
        .map(|expr| expr.module);

    let usings = parse_required_tag::<Vec<UseExpr>>(ctx, &pairs,
        "uses", Error::MissingUses,
        &match module.as_ref() {
            Ok(module) => module.span.after_end(),
            Err(_) => ctx.span_at_start(),
        })
        .map(|vec| vec.into_iter().map(|expr| expr.module).collect::<Vec<_>>());

    let blocks = parse_required_tag::<Vec<_>>(ctx, &pairs,
        "top_blocks", Error::MissingTopLevelBlocks,
        &match usings.as_ref().ok().and_then(|vec| vec.last()) {
            Some(last_using) => last_using.span.after_end(),
            None => ctx.span_at_start(),
        });

    let mut classes = Vec::new();
    let mut enums = Vec::new();
    let mut global_variables = Vec::new();
    let mut global_functions = Vec::new();

    let mut had_error = false;
    for block in blocks? {
        let Ok(relation_block) = RelationBlock::parse(ctx, block) else {
            had_error = true;
            continue;
        };

        had_error |= match relation_block.kind.as_str() {
            "class" => Class::parse(ctx, relation_block)
                .map(|class| classes.push(class)),
            "enum" => Enum::parse(ctx, relation_block)
                .map(|r#enum| enums.push(r#enum)),
            "globals" => Globals::parse(ctx, relation_block)
                .map(|Globals { variables, functions }| {
                    global_variables.extend(variables);
                    global_functions.extend(functions);
                }),
            kind => {
                ctx.add_err(Error::UnexpectedTopLevelBlock { kind: kind.to_owned() }, relation_block.kind.span);
                Err(())
            }
        }.is_err();
    }
    if had_error {
        return Err(());
    }

    let module = module?;
    let usings = usings?;

    Ok(File {
        name,
        module,
        usings,
        classes,
        enums,
        global_variables,
        global_functions,
    })
}

impl<'a> Parsable<'a> for WithSpan<usize> {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let string = pair.as_str();
        
        let value = if let Some(string) = string.strip_prefix("0b") {
            usize::from_str_radix(string, 2)
        } else if let Some(string) = string.strip_prefix("0o") {
            usize::from_str_radix(string, 8)
        } else if let Some(string) = string.strip_prefix("0x") {
            usize::from_str_radix(string, 16)
        } else {
            string.parse()
        }.map_err(|e| {
            ctx.add_err(Error::ParseInt(e), span.clone());
        })?;

        Ok(WithSpan {
            value,
            span,
        })
    }
}

impl<'a> Parsable<'a> for WithSpan<String> {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        Ok(WithSpan {
            span: ctx.span_from_pair(&pair),
            value: pair.as_str().to_owned(),
        })
    }
}

impl<'a> Parsable<'a> for Module {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        Ok(Module {
            span: ctx.span_from_pair(&pair),
            names: parse_all_as(ctx, pair.into_inner())?,
        })
    }
}

impl<'a> Parsable<'a> for Type {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let pair = match pair.into_inner().next() {
            Some(pair) => pair,
            None => {
                ctx.add_err(Error::Unexpected("Found type with no inner pair.".to_owned()), span);
                return Err(());
            }
        };

        match pair.as_rule() {
            Rule::ArrayType => ArrayType::parse(ctx, pair)
                .map(|arr| Type::Array(arr)),
            Rule::PointerType => PointerType::parse(ctx, pair)
                .map(|ptr| Type::Pointer(ptr)),
            Rule::FunctionType => FunctionType::parse(ctx, pair)
                .map(|r#fn| Type::Function(r#fn)),
            Rule::NamedType => NamedType::parse(ctx, pair)
                .map(|named| Type::Named(named)),
            _ => {
                ctx.add_err(Error::Unexpected("Found type with unknown inner pair.".to_owned()), span);
                Err(())
            }
        }
    }
}

impl<'a> Parsable<'a> for ArrayType {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let count = parse_required_tag(ctx, &inner,
            "count", Error::Unexpected("Found array type with no count.".to_owned()), &span);
        let item_type = parse_required_tag(ctx, &inner,
            "type", Error::Unexpected("Found array type with no item type.".to_owned()), &span);

        let count = count?;
        let item_type = Box::new(item_type?);

        Ok(ArrayType {
            count,
            item_type,
            span,
        })
    }
}

impl<'a> Parsable<'a> for PointerType {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let inner_type = parse_required_tag(ctx, &inner,
            "type", Error::Unexpected("Found pointer type with no inner type.".to_owned()), &span);

        let inner_type = Box::new(inner_type?);

        Ok(PointerType {
            inner_type,
            span,
        })
    }
}

impl<'a> Parsable<'a> for FunctionType {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let call_conv = parse_required_tag(ctx, &inner,
            "callconv", Error::Unexpected("Found function type without call conv.".to_owned()), &span);
        let generics = parse_required_tag(ctx, &inner,
            "generics", Error::Unexpected("Found function type without generics.".to_owned()), &span);
        let args = parse_required_tag(ctx, &inner,
            "args", Error::Unexpected("Found function type without args.".to_owned()), &span);
        let ret = parse_required_tag(ctx, &inner,
            "ret", Error::Unexpected("Found function type without ret.".to_owned()), &span);
            
        let FunctionCallConv { call_conv } = call_conv?;
        let FunctionGenerics { generics } = generics?;
        let args = args?;
        let FunctionRet { ret } = ret?;

        Ok(FunctionType {
            call_conv,
            generics,
            args,
            ret,
            span,
        })
    }
}

struct FunctionCallConv {
    pub call_conv: Option<WithSpan<String>>,
}

impl<'a> Parsable<'a> for FunctionCallConv {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let inner = pair.into_inner();

        let call_conv = parse_optional_tag(ctx, &inner, 
            "name");

        let call_conv = call_conv?;

        Ok(Self {
            call_conv,
        })
    }
}

struct FunctionGenerics {
    pub generics: Option<GenericParams>,
}

impl<'a> Parsable<'a> for FunctionGenerics {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let inner = pair.into_inner();

        let generics = parse_optional_tag(ctx, &inner, 
            "generics");

        let generics = generics?;

        Ok(Self {
            generics,
        })
    }
}

impl<'a> Parsable<'a> for FunctionArg {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let name = parse_required_tag(ctx, &inner,
            "name", Error::Unexpected("Found function arg without name.".to_owned()), &span);
        let r#type = parse_required_tag(ctx, &inner,
            "type", Error::Unexpected("Found function arg without type.".to_owned()), &span);

        let name = name?;
        let r#type = r#type?;

        Ok(FunctionArg {
            name,
            r#type,
            span,
        })
    }
}

struct FunctionRet {
    pub ret: Option<Box<Type>>,
}

impl<'a> Parsable<'a> for FunctionRet {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let inner = pair.into_inner();

        let ret = parse_optional_tag(ctx, &inner, 
            "type");

        let ret = ret?.map(Box::new);

        Ok(Self {
            ret,
        })
    }
}

impl<'a> Parsable<'a> for NamedType {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let name = parse_required_tag(ctx, &inner,
            "name", Error::Unexpected("Found named type without name.".to_owned()), &span);
        let generics = parse_optional_tag(ctx, &inner,
            "generics");

        let name = name?;
        let generics = generics?;

        Ok(NamedType {
            name,
            generics,
            span,
        })
    }
}

struct Binding {
    pub name: WithSpan<String>,
    pub r#type: Type,
}

impl<'a> Parsable<'a> for Binding {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let name = parse_required_tag(ctx, &inner,
            "name", Error::Unexpected("Found binding without name.".to_owned()), &span);
        let r#type = parse_required_tag(ctx, &inner,
            "type", Error::Unexpected("Found binding without type.".to_owned()), &span);

        let name = name?;
        let r#type = r#type?;

        Ok(Binding {
            name,
            r#type,
        })
    }
}

impl<'a> Parsable<'a> for GenericParams {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        Ok(GenericParams {
            span: ctx.span_from_pair(&pair),
            names: parse_all_as(ctx, pair.into_inner())?
        })
    }
}

impl<'a> Parsable<'a> for GenericArgs {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        Ok(GenericArgs {
            span: ctx.span_from_pair(&pair),
            types: parse_all_as(ctx, pair.into_inner())?
        })
    }
}

struct TypedBlock<'a> {
    pub r#type: Type,
    pub block: Pair<'a, Rule>,
}

impl<'a> Parsable<'a> for TypedBlock<'a> {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let r#type = parse_required_tag(ctx, &inner,
            "type", Error::Unexpected("Found typed block without type.".to_owned()), &span);
        let block = parse_required_tag(ctx, &inner,
            "block", Error::Unexpected("Found typed block without block.".to_owned()), &span);

        let r#type = r#type?;
        let block = block?;

        Ok(TypedBlock {
            r#type,
            block,
        })
    }
}

struct ModExpr {
    pub module: Module,
}

impl<'a> Parsable<'a> for ModExpr {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let module = parse_required_tag(ctx, &inner, 
            "module", Error::Unexpected("Found mod expr without a module.".to_owned()), &span);

        let module = module?;

        Ok(ModExpr {
            module,
        })
    }
}

struct UseExpr {
    pub module: Module,
}

impl<'a> Parsable<'a> for UseExpr {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let module = parse_required_tag(ctx, &inner, 
            "module", Error::Unexpected("Found mod expr without a module.".to_owned()), &span);

        let module = module?;

        Ok(UseExpr {
            module,
        })
    }
}

struct Relation<'a> {
    pub left: Pair<'a, Rule>,
    pub right: Pair<'a, Rule>,
}

impl<'a> Parsable<'a> for Relation<'a> {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let left = parse_required_tag(ctx, &inner,
            "left", Error::Unexpected("Found relation with no left side.".to_owned()), &span);
        let right = parse_required_tag(ctx, &inner,
            "right", Error::Unexpected("Found relation with no right side.".to_owned()), &span);

        let left = left?;
        let right = right?;

        Ok(Self {
            left,
            right,
        })
    }
}

impl<'a> Relation<'a> {
    pub fn parse_into<L: Parsable<'a>, R: Parsable<'a>>(ctx: &mut Ctx, pair: Pair<'a, Rule>, left_rule: Option<Rule>, right_rule: Option<Rule>) -> Result<(L, R)> {
        let relation = Self::parse(ctx, pair)?;
        
        fn check_bad_rule<'a>(ctx: &mut Ctx, pair: &Pair<'a, Rule>, expected: Option<Rule>) -> bool {
            let found = pair.as_rule();

            match expected {
                Some(expected) if found == expected => {
                    ctx.add_err(Error::UnexpectedRule { expected, found }, ctx.span_from_pair(pair));
                    true
                }
                _ => false,
            }
        }

        let mut is_bad_rule = false;
        is_bad_rule |= check_bad_rule(ctx, &relation.left, left_rule);
        is_bad_rule |= check_bad_rule(ctx, &relation.right, right_rule);

        if is_bad_rule {
            return Err(())
        }

        let left = L::parse(ctx, relation.left);
        let right = R::parse(ctx, relation.right);

        let left = left?;
        let right = right?;

        Ok((left, right))
    }
}

struct RelationBlock<'a> {
    pub kind: WithSpan<String>,
    pub name: Option<WithSpan<String>>,
    pub generics: Option<GenericParams>,
    pub block: Pair<'a, Rule>,
    pub span: Span,
}

impl<'a> Parsable<'a> for RelationBlock<'a> {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let inner = pair.into_inner();

        let kind = parse_required_tag(ctx, &inner,
            "kind", Error::Unexpected("Found relation block without a kind.".to_owned()), &span);
        let args = parse_required_tag(ctx, &inner, 
            "args", Error::Unexpected("Found relation block without args.".to_owned()), &span);
        let block = parse_required_tag(ctx, &inner,
            "block", Error::Unexpected("Found relation block without a block.".to_owned()), &span);

        let kind = kind?;
        let RelationBlockArgs { name, generics } = args?;
        let block = block?;

        Ok(RelationBlock {
            kind,
            name,
            generics,
            block,
            span,
        })
    }
}

struct RelationBlockArgs {
    pub name: Option<WithSpan<String>>,
    pub generics: Option<GenericParams>,
}

impl<'a> Parsable<'a> for RelationBlockArgs {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let inner= pair.into_inner();

        let name = parse_optional_tag(ctx, &inner,
            "name");
        let generics = parse_optional_tag(ctx, &inner,
            "generics");

        let name = name?;
        let generics = generics?;

        Ok(Self {
            name,
            generics,
        })
    }
}

struct Block<'a> {
    pub pairs: Vec<Pair<'a, Rule>>,
}

impl<'a> Parsable<'a> for Block<'a> {
    fn parse(ctx: &mut Ctx, block: Pair<'a, Rule>) -> Result<Self> {
        let pairs = block.into_inner().map(|pair| {
            match pair.as_rule() {
                Rule::Comma => {
                    ctx.add_err(Error::NoCommaBlock, ctx.span_from_pair(&pair));
                    Err(())
                },
                _ => Ok(pair),
            }
        }).collect::<Result<_>>()?;

        Ok(Block {
            pairs,
        })
    }
}

impl<'a> Block<'a> {
    fn parse_into_relation_vec(ctx: &mut Ctx, block: Pair<'a, Rule>) -> Result<Vec<RelationItem<'a>>> {
        let block = Self::parse(ctx, block)?;

        let mut items = Vec::new();

        fn validate_no_args<'a>(ctx: &mut Ctx, relation_block: RelationBlock<'a>) -> Result<(WithSpan<String>, Pair<'a, Rule>)> {
            let mut has_args = false;
            if let Some(name) = relation_block.name {
                ctx.add_err(Error::UnexpectedName, name.span);
                has_args = true;
            }
            if let Some(generics) = relation_block.generics {
                ctx.add_err(Error::UnexpectedGenerics, generics.span);
                has_args = true;
            }
            if has_args {
                Err(())
            } else {
                Ok((relation_block.kind, relation_block.block))
            }
        }

        let mut had_error = false;
        for pair in block.pairs {
            had_error |= match pair.as_rule() {
                Rule::Relation => {
                    Relation::parse_into(ctx, pair, Some(Rule::Identifier), None)
                        .map(|(kind, value)| items.push(RelationItem::Value { kind, value }))
                }
                Rule::RelationBlock => RelationBlock::parse(ctx, pair)
                    .and_then(|relation_block| validate_no_args(ctx, relation_block))
                    .map(|(kind, block)| items.push(RelationItem::Block { kind, block })),
                _ => {
                    ctx.add_err(Error::Unexpected("Found non-relation item in block.".to_owned()), ctx.span_from_pair(&pair));
                    Err(())
                }
            }.is_err();
        }

        if had_error {
            Err(())
        } else {
            Ok(items)
        }
    }
}

enum RelationItem<'a> {
    Value { kind: WithSpan<String>, value: Pair<'a, Rule> },
    Block { kind: WithSpan<String>, block: Pair<'a, Rule> },
}

struct CommaBlock<'a> {
    pub pairs: Vec<Pair<'a, Rule>>,
}

impl<'a> Parsable<'a> for CommaBlock<'a> {
    fn parse(ctx: &mut Ctx, block: Pair<'a, Rule>) -> Result<Self> {
        let mut pairs = Vec::new();
        let mut expect_comma = false;

        let mut had_error = false;
        for pair in block.into_inner() {
            match pair.as_rule() {
                Rule::Comma if expect_comma => expect_comma = false,
                Rule::Comma => {
                    ctx.add_err(Error::UnexpectedComma, ctx.span_from_pair(&pair));
                    had_error = true;
                }
                _ if !expect_comma => {
                    pairs.push(pair);
                    expect_comma = true;
                }
                _ => {
                    ctx.add_err(Error::ExpectedComma, ctx.span_at_pair_end(&pair));
                    had_error = true;
                }
            }
        }

        if had_error {
            Err(())
        } else {
            Ok(CommaBlock {
                pairs,
            })
        }
    }
}

impl<'a> CommaBlock<'a> {
    fn add_into_vec<T: Parsable<'a>>(ctx: &mut Ctx, block: Pair<'a, Rule>, vec: &mut Vec<T>) -> Result<()> {
        let mut new = Vec::new();

        let mut had_error = false;
        for item in Self::parse(ctx, block)?.pairs {
            had_error = T::parse(ctx, item)
                .map(|item| new.push(item))
                .is_err();
        }

        if had_error {
            Err(())
        } else {
            Ok(vec.extend(new))
        }
    }
}

impl Class {
    fn parse(ctx: &mut Ctx, relation_block: RelationBlock) -> Result<Self> {
        let name = relation_block.name
            .ok_or_else(|| ctx.add_err(Error::MissingNameForBlock { kind: "class".to_owned() }, relation_block.kind.span.after_end()));
        let generics = relation_block.generics;
        let items = Block::parse_into_relation_vec(ctx, relation_block.block);

        let mut size = None;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut bases = Vec::new();
        let mut static_fields = Vec::new();
        let mut static_methods = Vec::new();

        let mut had_error = false;
        for item in items? {
            had_error |= match item {
                RelationItem::Value { kind, value } => match kind.as_str() {
                    "size" => size.set_once(ctx, kind, value),
                    _ => {
                        ctx.add_err(Error::UnknownRelationItem { block_kind: "class".to_owned(), kind: kind.value }, kind.span);
                        Err(())
                    }
                }
                RelationItem::Block { kind, block } => match kind.as_str() {
                    "fields" => CommaBlock::add_into_vec(ctx, block, &mut fields),
                    "methods" => CommaBlock::add_into_vec(ctx, block, &mut methods),
                    "bases" => CommaBlock::add_into_vec(ctx, block, &mut bases),
                    "statics" => Block::parse_into_relation_vec(ctx, block)
                        .and_then(|items| Globals::from_items(ctx, items))
                        .map(|Globals { variables, functions }| {
                            static_fields.extend(variables);
                            static_methods.extend(functions);
                        }),
                    _ => {
                        ctx.add_err(Error::UnknownRelationItem { block_kind: "class".to_owned(), kind: kind.value }, kind.span);
                        Err(())
                    }
                }
            }.is_err();
        }

        let name = name?;

        if had_error {
            Err(())
        } else {
            Ok(Class {
                name,
                generics,
                size,
                fields,
                methods,
                bases,
                static_fields,
                static_methods,
                span: relation_block.span,
            })
        }
    }
}

impl<'a> Parsable<'a> for Variable {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let (offset, binding) = Relation::parse_into(ctx, pair, Some(Rule::Integer), Some(Rule::Binding))?;
        let Binding { name, r#type, .. } = binding;

        Ok(Variable {
            offset,
            name,
            r#type,
            span,
        })
    }
}


impl<'a> Parsable<'a> for Function {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span= ctx.span_from_pair(&pair);
        let (pointer, binding) = Relation::parse_into(ctx, pair, Some(Rule::Integer), Some(Rule::Binding))?;
        let Binding { name, r#type, .. } = binding;

        let Type::Function(r#type) = r#type else {
            ctx.add_err(Error::ExpectedTypeKind { kind: "function".to_owned() }, r#type.get_span().clone());
            return Err(())
        };

        Ok(Function {
            pointer,
            name,
            r#type,
            span,
        })
    }
}

impl<'a> Parsable<'a> for ClassBase {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let (offset, typed_block) = Relation::parse_into(ctx, pair, Some(Rule::Integer), Some(Rule::TypedBlock))?;
        let TypedBlock { r#type, block, .. } = typed_block;

        let mut vtable = None;
        let mut overrides = Vec::new();
        let mut virtuals = Vec::new();

        let mut had_error = false;
        for item in Block::parse_into_relation_vec(ctx, block)? {
            had_error |= match item {
                RelationItem::Value { kind, value } => match kind.as_str() {
                    "vtable" => vtable.set_once(ctx, kind, value),
                    _ => {
                        ctx.add_err(Error::UnknownRelationItem { block_kind: "class base".to_owned(), kind: kind.value }, kind.span);
                        Err(())
                    }
                },
                RelationItem::Block { kind, block } => match kind.as_str() {
                    "overrides" => CommaBlock::add_into_vec(ctx, block, &mut overrides),
                    "virtuals" => CommaBlock::add_into_vec(ctx, block, &mut virtuals),
                    _ => {
                        ctx.add_err(Error::UnknownRelationItem { block_kind: "class base".to_owned(), kind: kind.value }, kind.span);
                        Err(())
                    }
                }
            }.is_err();
        }

        let r#type = match r#type {
            Type::Named(named) => Ok(named),
            _ => {
                ctx.add_err(Error::ExpectedTypeKind { kind: "named type".to_owned() }, r#type.get_span().clone());
                Err(())
            }
        };

        let r#type = r#type?;

        if had_error {
            Err(())
        } else {
            Ok(ClassBase {
                offset,
                r#type,
                vtable,
                overrides,
                virtuals,
                span,
            })
        }
    }
}

impl<'a> Parsable<'a> for Override {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let (pointer, name) = Relation::parse_into(ctx, pair, Some(Rule::Integer), Some(Rule::Identifier))?;

        Ok(Override {
            pointer,
            name,
            span,
        })
    }
}

impl<'a> Parsable<'a> for Virtual {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let (index, function) = Relation::parse_into(ctx, pair, Some(Rule::Integer), Some(Rule::Binding))?;

        Ok(Virtual {
            index,
            function,
            span,
        })
    }
}

struct Globals {
    pub variables: Vec<Variable>,
    pub functions: Vec<Function>,
}

impl Globals {
    fn parse(ctx: &mut Ctx, relation_block: RelationBlock) -> Result<Self> {
        let mut has_args = false;
        if let Some(name) = relation_block.name {
            ctx.add_err(Error::UnexpectedName, name.span);
            has_args = true;
        }
        if let Some(generics) = relation_block.generics {
            ctx.add_err(Error::UnexpectedGenerics, generics.span);
            has_args = true;
        }
        if has_args {
            Err(())
        } else {
            Block::parse_into_relation_vec(ctx, relation_block.block)
                .and_then(|items| Globals::from_items(ctx, items))
        }
    }

    fn from_items(ctx: &mut Ctx, items: Vec<RelationItem>) -> Result<Self> {
        let mut variables = Vec::new();
        let mut functions = Vec::new();

        let mut had_error = false;
        for item in items {
            had_error |= match item {
                RelationItem::Value { kind, .. } => {
                    ctx.add_err(Error::UnknownRelationItem { block_kind: "globals".to_owned(), kind: kind.value }, kind.span);
                    Err(())
                }
                RelationItem::Block { kind, block } => match kind.as_str() {
                    "variables" => CommaBlock::add_into_vec(ctx, block, &mut variables),
                    "functions" => CommaBlock::add_into_vec(ctx, block, &mut functions),
                    _ => {
                        ctx.add_err(Error::UnknownRelationItem { block_kind: "globals".to_owned(), kind: kind.value }, kind.span);
                        Err(())
                    }
                }
            }.is_err();
        };

        if had_error {
            Err(())
        } else {
            Ok(Self {
                variables,
                functions,
            })
        }
    }
}

impl Enum {
    fn parse(ctx: &mut Ctx, relation_block: RelationBlock) -> Result<Self> {
        let name = relation_block.name
            .ok_or_else(|| ctx.add_err(Error::MissingNameForBlock { kind: "enum".to_owned() }, relation_block.kind.span.after_end()));
        let no_generics = match relation_block.generics {
            Some(generics) => {
                ctx.add_err(Error::UnexpectedGenerics, generics.span);
                Err(())
            }
            None => Ok(())
        };
        let items = Block::parse_into_relation_vec(ctx, relation_block.block);

        let mut r#type = None;
        let mut values = Vec::new();

        let mut had_error = false;
        for item in items? {
            had_error |= match item {
                RelationItem::Value { kind, value } => match kind.as_str() {
                    "type" => r#type.set_once(ctx, kind, value),
                    _ => {
                        ctx.add_err(Error::UnknownRelationItem { block_kind: "enum".to_owned(), kind: kind.value }, kind.span);
                        Err(())
                    }
                }
                RelationItem::Block { kind, block } => match kind.as_str() {
                    "values" => CommaBlock::add_into_vec(ctx, block, &mut values),
                    _ => {
                        ctx.add_err(Error::UnknownRelationItem { block_kind: "enum".to_owned(), kind: kind.value }, kind.span);
                        Err(())
                    }
                }
            }.is_err();
        }

        let Some(r#type) = r#type else {
            ctx.add_err(Error::MissingEnumType, name
                .map(|name| name.span)
                .unwrap_or(relation_block.kind.span.after_end()));
            return Err(());
        };

        let name = name?;
        no_generics?;

        if had_error {
            Err(())
        } else {
            Ok(Self {
                name,
                r#type,
                values,
                span: relation_block.span,
            })
        }
    }
}

impl<'a> Parsable<'a> for EnumValue {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        let span = ctx.span_from_pair(&pair);
        let (value, name) = Relation::parse_into(ctx, pair, Some(Rule::Integer), Some(Rule::Identifier))?;

        Ok(EnumValue {
            value,
            name,
            span,
        })
    }
}

trait Parsable<'a> {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self>
        where Self : Sized;
}

impl<'a, T: Parsable<'a>> Parsable<'a> for Vec<T> {
    fn parse(ctx: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        parse_all_as(ctx, pair.into_inner())
    }
}

impl<'a> Parsable<'a> for Pair<'a, Rule> {
    fn parse(_: &mut Ctx, pair: Pair<'a, Rule>) -> Result<Self> {
        Ok(pair)
    }
}

fn parse_all_as<'a, T: Parsable<'a>>(
    ctx: &mut Ctx,
    pairs: Pairs<'a, Rule>,
) -> Result<Vec<T>> {
    pairs.map(|pair| T::parse(ctx, pair))
        .collect()
}

fn parse_required_tag<'a, T: Parsable<'a>>(
    ctx: &mut Ctx, 
    pairs: &Pairs<'a, Rule>, 
    tag: &'a str,
    err_if_missing: Error,
    span_if_missing: &Span,
) -> Result<T> {
    match pairs.find_first_tagged(tag) {
        Some(pair) => T::parse(ctx, pair),
        None => {
            ctx.add_err(err_if_missing, span_if_missing.clone());
            Err(())
        }
    }
}

fn parse_optional_tag<'a, T: Parsable<'a>>(
    ctx: &mut Ctx,
    pairs: &Pairs<'a, Rule>,
    tag: &'a str,
) -> Result<Option<T>> {
    pairs.find_first_tagged(tag)
        .map(|pair: Pair<'_, Rule>| T::parse(ctx, pair))
        .transpose()
}

pub struct Ctx {
    file: String,
    errs: Vec<WithSpan<Error>>,
}

impl Ctx {
    pub fn new(file: String) -> Self {
        Ctx {
            file,
            errs: Vec::new(),
        }
    }

    pub fn add_err(&mut self, err: Error, span: Span) {
        self.errs.push(WithSpan { value: err, span })
    }

    pub fn convert_span<'a>(&self, span: pest::Span<'a>) -> Span {
        Span {
            file: self.file.clone(),
            input: span.as_str().to_owned(),
            start: span.start_pos().line_col(),
            end: span.end_pos().line_col(),
        }
    }

    pub fn span_from_linecol(&self, value: pest::error::LineColLocation) -> Span {
        let (start, end) = match value {
            pest::error::LineColLocation::Pos(pos) => (pos, pos),
            pest::error::LineColLocation::Span(start, end) => (start, end),
        };
        Span {
            file: self.file.clone(),
            start,
            end,
            ..Default::default()
        }
    }

    pub fn span_from_pair<'a>(&self, pair: &Pair<'a, Rule>) -> Span {
        self.convert_span(pair.as_span())
    }

    pub fn span_at_pair_end<'a>(&self, pair: &Pair<'a, Rule>) -> Span {
        let mut end = pair.as_span().end_pos().line_col();
        end.1 += 1;
        self.to_span(end)
    }

    pub fn to_span(&self, value: (usize, usize)) -> Span {
        Span {
            file: self.file.clone(),
            start: value,
            end: value,
            ..Default::default()
        }
    }

    pub fn span_at_start(&self) -> Span {
        Span {
            file: self.file.clone(),
            ..Default::default()
        }
    }
}

impl ToString for Ctx {
    fn to_string(&self) -> String {
        let file = &self.file;
        let mut msg = String::new();

        for WithSpan { value: err, span: Span { input, start: (line, col), .. } } in &self.errs {
            msg.push_str(&format!("error: {err}\n    at {file}:{line}:{col}\n    | {input}\n\n"));
        }

        msg
    }
}

trait SetOnceTrait<'a> {
    fn set_once(&mut self, ctx: &mut Ctx, kind: WithSpan<String>, value: Pair<'a, Rule>) -> Result<()>;
}

impl<'a, T: Parsable<'a>> SetOnceTrait<'a> for Option<T> {
    fn set_once(&mut self, ctx: &mut Ctx, kind: WithSpan<String>, value: Pair<'a, Rule>) -> Result<()> {
        if self.is_some() {
            ctx.add_err(Error::DuplicateProperty { kind: kind.value }, kind.span);
            Err(())
        } else {
            T::parse(ctx, value)
                .map(|new_size| *self = Some(new_size))
        }
    }
}
