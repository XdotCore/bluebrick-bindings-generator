mod parsers;

use std::fs;

use ariadne::{Label, Report, ReportKind, Source};
use chumsky::Parser;

use crate::parsers::lexer::{self, lexer};

fn main() {
    let file_name = std::env::args().nth(1).unwrap();
    let src = fs::read_to_string(&file_name).unwrap();

    fs::write("lexer.svg", lexer().debug().to_railroad_svg().to_string()).unwrap();
    
    let (output, errors) = lexer().parse(&src).into_output_errors();

    println!("Errors:");
    for error in errors {
        if lexer::try_print_err(&file_name, Source::from(src.clone()), &error) {
            continue;
        }

        Report::build(ReportKind::Error, (file_name.clone(), error.span().into_range()))
            .with_label(Label::new((file_name.clone(), error.span().into_range())))
            .with_message(error.to_string())
            .finish()
            .print((file_name.clone(), Source::from(src.clone())))
            .unwrap();
    }

    println!("Output:");
    println!("{:#?}", output);
}
