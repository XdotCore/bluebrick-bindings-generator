use chumsky::prelude::*;

#[derive(Debug, Clone)]
pub enum Token<'src> {
    Int(usize),
    Ident(&'src str),
    PathSep,
    Assign,
    Declare,
    Comma,
    BlockOpen,
    BlockClose,
    TupleOpen,
    TupleClose,
    AttrOpen,
    AttrClose,
    GenericsOpen,
    GenericsClose,
}

fn lexer<'src>() -> impl Parser<'src, &'src str, Vec<Result<Spanned<Token<'src>>, Spanned<char>>>, extra::Err<Rich<'src, char>>> {
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

    let ident = text::unicode::ident()
        .map(Token::Ident)
        .labelled("Identifier");

    let symbol = |symbol, token| {
        just(symbol)
            .to(token)
            .labelled(symbol)
    };

    let path_sep = symbol("::", Token::PathSep);
    let assign = symbol("=", Token::Assign);
    let declare = symbol(":", Token::Declare);
    let comma = symbol(",", Token::Comma);

    let block_open = symbol("{", Token::BlockOpen);
    let block_close = symbol("}", Token::BlockClose);

    let tuple_open = symbol("(", Token::TupleOpen);
    let tuple_close = symbol(")", Token::TupleClose);

    let attr_open = symbol("[", Token::AttrOpen);
    let attr_close = symbol("]", Token::AttrClose);

    let generics_open = symbol("<", Token::GenericsOpen);
    let generics_close = symbol(">", Token::GenericsClose);

    choice((
        int,
        ident,
        path_sep,
        assign,
        declare,
        comma,
        block_open,
        block_close,
        tuple_open,
        tuple_close,
        attr_open,
        attr_close,
        generics_open,
        generics_close,
    ))
        .padded()
        .spanned()
        .map(|token| Ok(token))
        .or(
            any()
                .spanned()
                .map(|c| Err(c))
        )
        .repeated()
        .collect()
}
