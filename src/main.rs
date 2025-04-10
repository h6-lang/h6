use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use clap::{Parser, Subcommand};
use camino::Utf8PathBuf;
use h6_bytecode::{Bytecode, Op};
use h6_compiler::{lex, parse, lower, UnSpannedGetter};

#[derive(Parser, Debug)]
#[clap(name = "h6", version)]
pub struct App {
    // insert global options here

    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// compile to bytecode file
    Compile {
        #[clap(short = 'o')]
        output: Utf8PathBuf,

        input: Utf8PathBuf,
    },

    #[clap(alias = "link")]
    Ld {
        inputs: Vec<Utf8PathBuf>,

        #[clap(short = 'o')]
        output: Utf8PathBuf,
    },

    Run {
        input: Utf8PathBuf,
    },

    /// list symbols in bytecode file
    Nm {
        input: Utf8PathBuf,
    },
}


fn main() {
    let args = App::parse();

    match args.command {
        Command::Compile { input, output } => {
            let content = std::fs::read_to_string(input).unwrap();

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

            let mut sink = File::create(output).unwrap();
            lower::lower_full(&mut sink, &UnSpannedGetter::new(toks.as_slice()), exprs.iter())
                .unwrap();
        }

        Command::Nm { input } => {
            let mut content = vec!();
            File::open(input).unwrap()
                .read_to_end(&mut content).unwrap();
            let asm = Bytecode::try_from(content.as_slice())
                .unwrap();

            let mut defines = HashSet::new();
            for global in asm.named_globals() {
                let (name, pos) = global.unwrap();
                defines.insert(name);
                println!("{:#06x} T {}", pos, name);
            }

            let mut discovered = HashSet::new();

            fn rec<'a, I: Iterator<Item=Result<Op, h6_bytecode::ByteCodeError>>>(discovered: &mut HashSet<&'a str>, asm: &'a Bytecode, iter: I) {
                for op in iter {
                    let op = op.unwrap();
                    match op {
                        Op::Unresolved { id } => {
                            let st = asm.string(id).unwrap();
                            if !discovered.contains(&st) {
                                discovered.insert(st);
                                let decl = asm.named_globals().find(|x| x.unwrap().0 == st);
                                if let Some(decl) = decl {
                                    let decl = decl.unwrap().1;
                                    rec(discovered, asm, asm.const_ops(decl).unwrap());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            rec(&mut discovered, &asm, asm.globals()
                .map(|x| x.const_id)
                .flat_map(|x| asm.const_ops(x).unwrap()));
            rec(&mut discovered, &asm, asm.main_ops());

            for ent in discovered {
                if !defines.contains(ent) {
                    println!("       t {}", ent);
                }
            }
        }

        _ => panic!("unimplemented")
    }
}
