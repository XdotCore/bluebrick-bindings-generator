use chumsky::prelude::*;

#[derive(Debug, Clone)]
pub enum Token<'src> {
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

pub fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<Token<'src>>, extra::Err<Rich<'src, char>>> {
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

    let ident = text::ascii::ident()
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
        let wrap = |open, close| {
            token.clone()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(open), just(close))
        };

        let block = wrap('{', '}')
            .map(Token::Block)
            .labelled("Block");

        let tuple = wrap('(', ')')
            .map(Token::Tuple)
            .labelled("Tuple");

        let attr = wrap('[', ']')
            .map(Token::Attr)
            .labelled("Attribute");

        let generics = wrap('<', '>')
            .map(Token::Generic)
            .labelled("Generics");

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
            .labelled("Syntax")
    });

    token
        .repeated()
        .collect()
}
