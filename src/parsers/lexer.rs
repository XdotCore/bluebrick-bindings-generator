use std::{collections::HashMap, sync::LazyLock};

use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::{error::RichPattern, prelude::*};

#[derive(Debug, Clone)]
pub enum Token<'src> {
    Error,
    Int(usize),
    Ident(Vec<&'src str>),
    Assign,
    Declare,
    Comma,
    Block(Vec<Token<'src>>),
    Tuple(Vec<Token<'src>>),
    Attr(Vec<Token<'src>>),
    Generic(Vec<Token<'src>>),
}

type Wrap = (char, char);
const BLOCK: Wrap =    ('{', '}');
const TUPLE: Wrap =    ('(', ')');
const ATTR: Wrap =     ('[', ']');
const GENERICS: Wrap = ('<', '>');
const BLOCK_LABEL: &str =    "Block";
const TUPLE_LABEL: &str =    "Tuple";
const ATTR_LABEL: &str =     "Attribute";
const GENERICS_LABEL: &str = "Generics";
const LABEL_TO_WRAP: LazyLock<HashMap<String, (Wrap, [Wrap; 3])>> = LazyLock::new(|| {
    [
        (BLOCK_LABEL.to_owned(),    (BLOCK,    [TUPLE, ATTR, GENERICS])),
        (TUPLE_LABEL.to_owned(),    (TUPLE,    [ATTR, GENERICS, BLOCK])),
        (ATTR_LABEL.to_owned(),     (ATTR,     [GENERICS, BLOCK, TUPLE])),
        (GENERICS_LABEL.to_owned(), (GENERICS, [BLOCK, TUPLE, ATTR])),
    ].into()
});

pub fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<Token<'src>>, extra::Err<Rich<'src, char, SimpleSpan<usize, ()>>>> {
    let int_with_base = |base, radix| {
        just(base)
            .ignore_then(
                text::digits(radix)
                    .to_slice()
                    .map(move |s: &str| usize::from_str_radix(s, radix).unwrap())
            )
    };

    let int = choice((
        int_with_base("0x", 16),
        int_with_base("0o", 8),
        int_with_base("0b", 2),
        int_with_base("", 10),
    ))
        .map(Token::Int)
        .labelled("Int");

    // todo: unicode?
    let ident = text::unicode::ident()
        .separated_by(just("::"))
        .at_least(1)
        .collect::<Vec<_>>()
        .map(Token::Ident)
        .labelled("Identifier");

    let assign = just('=')
        .to(Token::Assign)
        .labelled("Assignment");

    let declare = just(':')
        .to(Token::Declare)
        .labelled("Declaration");

    let comma = just(',')
        .to(Token::Comma)
        .labelled("Comma");

    let token = recursive(|token| {
        let wrap = |(open, close), label| {
            token.clone()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(
                    just(open),
                    just(close)
                        .ignored()
                        .recover_with(via_parser(end()))
                        .recover_with(skip_then_retry_until(any().ignored(), one_of("})]>").ignored().or(end()))),
                )
                .labelled(label)
                .as_context()
        };

        let block = wrap(BLOCK, BLOCK_LABEL)
            .map(Token::Block);

        let tuple = wrap(TUPLE, TUPLE_LABEL)
            .map(Token::Tuple);

        let attr = wrap(ATTR, ATTR_LABEL)
            .map(Token::Attr);

        let generics = wrap(GENERICS, GENERICS_LABEL)
            .map(Token::Generic);

        choice((
            int,
            ident,
            assign,
            declare,
            comma,
            block,
            tuple,
            attr,
            generics,
        ))
            .padded()
            /*.recover_with(via_parser(nested_delimiters(
                BLOCK.0, BLOCK.1,
                [TUPLE, ATTR, GENERICS],
                |_| Token::Error,
            )))
            .recover_with(via_parser(nested_delimiters(
                TUPLE.0, TUPLE.1,
                [ATTR, GENERICS, BLOCK],
                |_| Token::Error,
            )))
            .recover_with(via_parser(nested_delimiters(
                ATTR.0, ATTR.1,
                [GENERICS, BLOCK, TUPLE],
                |_| Token::Error,
            )))
            .recover_with(via_parser(nested_delimiters(
                GENERICS.0, GENERICS.1,
                [BLOCK, TUPLE, ATTR],
                |_| Token::Error,
            )))*/
            .recover_with(skip_then_retry_until(
                any().ignored(),
                one_of("})]>").ignored().or(end()),
            ))
    });

    token
        .repeated()
        .collect()
}

pub fn try_print_err<'src>(file_name: &str, source: Source, err: &Rich<'src, char, SimpleSpan<usize, ()>>) -> bool {
    match err.contexts().next() {
        Some((RichPattern::Label(label), span)) => {
            let label = label.to_string();
            let (open, close) = LABEL_TO_WRAP[&label].0;

            let err_span = (file_name, err.span().clone().into_range());
            let open_span = (file_name, span.start..(span.start + 1));

            let err_color = Color::Red;
            let warn_color = Color::Yellow;
            let sugg_color = Color::Cyan;

            let open_quoted = format!("'{open}'").fg(err_color);
            let open = open.fg(err_color);
            let close_quoted = format!("'{close}'").fg(sugg_color);
            let close = close.fg(sugg_color);
            
            let report = Report::build(
                    ReportKind::Error,
                    err_span.clone()
                )
                .with_message(format!("Unclosed delimiter {open_quoted}, expected {close_quoted}"))
                .with_label(
                    Label::new(open_span.clone())
                        .with_message(format!("Delimiter {open} is never closed"))
                        .with_color(err_color)
                );

            let report = match err.found() {
                Some(found) => {
                    let found = found.fg(warn_color);

                    report.with_label(
                        Label::new(err_span.clone())
                            .with_message(format!("Must be closed before this {found}"))
                            .with_color(warn_color)
                    )
                }
                None => {
                    report.with_label(
                        Label::new(err_span.clone())
                            .with_message(format!("Must be closed before end of file"))
                            .with_color(warn_color)
                    )
                }
            };

            report.finish()
                .print((file_name, source))
                .unwrap();

            true
        }
        _ => false
    }
}
