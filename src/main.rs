mod lex;
mod parse;
mod bytecode;
mod lower;

pub type Num = fixed::types::I16F16;

fn main() {
    let file = "tests/01.h6";
    let content = std::fs::read_to_string(file).unwrap();

    let toks = lex::lex(content.as_str())
        .unwrap_or_else(|errs| {
            for err in errs {
                eprintln!("(lexer) {:#?}", err);
            }
            std::process::exit(1);
        });

    let exprs = parse::parse(toks.into_iter().map(|x| x.0))
        .unwrap_or_else(|errs| {
            for err in errs {
                eprintln!("(parser) {:#?}", err);
            }
            std::process::exit(1);
        });

    println!("{:#?}", exprs);
}
