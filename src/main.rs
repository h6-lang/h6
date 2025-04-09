mod lex;

pub type Num = fixed::types::I16F16;

fn main() {
    let file = "tests/01.h6";
    let content = std::fs::read_to_string(file).unwrap();

    let toks = lex::lex(content.as_str())
        .unwrap_or_else(|errs| {
            for err in errs {
                eprintln!("{:#?}", err);
            }
            std::process::exit(1);
        });

    for tok in toks {
        println!("{}", tok.0.highlight());
    }
}
