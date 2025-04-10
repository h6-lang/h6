use std::fs::File;
use crate::lex::{Spanned, Tok};

mod lex;
mod parse;
mod bytecode;
mod lower;

pub type Num = fixed::types::I16F16;

struct UnSpannedGetter<'x, 'src> {
    backing: &'x [Spanned<Tok<'src>>]
}

impl<'x, 'src> std::ops::Index<usize> for UnSpannedGetter<'x, 'src> {
    type Output = Tok<'src>;
    
    fn index(&self, index: usize) -> &Self::Output {
        &self.backing[index].0
    }
}

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

    let exprs = parse::parse(toks.iter().map(|x| x.0.clone()))
        .unwrap_or_else(|errs| {
            for err in errs {
                eprintln!("(parser) {:#?}", err);
            }
            std::process::exit(1);
        });

    let mut sink = File::create("out.h6b").unwrap();
    lower::lower_full(&mut sink, &UnSpannedGetter { backing: toks.as_slice() }, exprs.iter())
        .unwrap();
}
