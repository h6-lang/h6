use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Seek, Write};
use clap::{Parser, Subcommand};
use camino::Utf8PathBuf;
use h6_bytecode::{Bytecode, Header, Op, linker};
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

        #[clap(long, action)]
        allow_unresolved: bool,

        /// only concatenate assemblies into output. do not perform linking
        #[clap(long, action)]
        cat_only: bool,
    },

    Run {
        input: Utf8PathBuf,
    },

    /// list symbols in bytecode file
    Nm {
        input: Utf8PathBuf,
    },
}

struct HumanError {
    ty: HumanErrorTy,
    ctx: Option<String>,
}

enum HumanErrorTy {
    IOError(std::io::Error),
    LinkError(linker::LinkError),
    ByteCodeError(h6_bytecode::ByteCodeError),
}

impl From<std::io::Error> for HumanErrorTy {
    fn from(value: std::io::Error) -> Self {
        HumanErrorTy::IOError(value)
    }
}

impl From<linker::LinkError> for HumanErrorTy {
    fn from(value: linker::LinkError) -> Self {
        HumanErrorTy::LinkError(value)
    }
}

impl From<h6_bytecode::ByteCodeError> for HumanErrorTy {
    fn from(value: h6_bytecode::ByteCodeError) -> Self {
        HumanErrorTy::ByteCodeError(value)
    }
}

impl std::fmt::Debug for HumanErrorTy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HumanErrorTy::IOError(err) => write!(f, "I/O Error: {:?}", err),
            HumanErrorTy::LinkError(err) => write!(f, "Linker Error: {:?}", err),
            HumanErrorTy::ByteCodeError(err) => write!(f, "Bytecode Decode Error: {:?}", err),
        }
    }
}

impl std::fmt::Debug for HumanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.ctx {
            Some(ctx) => write!(f, "{}: {:?}", ctx, self.ty),
            None => write!(f, "{:?}", self.ty),
        }
    }
}

impl From<HumanErrorTy> for HumanError {
    fn from(value: HumanErrorTy) -> Self {
        HumanError { ty: value, ctx: None }
    }
}

trait WithCtx<V> {
    fn with_ctx<S: Into<String>>(self, ctx: S) -> Result<V, HumanError>;
}

impl<V, T: Sized + Into<HumanErrorTy>> WithCtx<V> for Result<V,T> {
    fn with_ctx<S: Into<String>>(self, ctx: S) -> Result<V, HumanError> {
        self.map_err(|err| HumanError { ty: err.into(), ctx: Some(ctx.into()) })
    }
}

fn main() -> Result<(), HumanError> {
    better_panic::install();
    let args = App::parse();

    match args.command {
        Command::Compile { input, output } => {
            let content = std::fs::read_to_string(input).with_ctx("could not open input file")?;

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

            let mut sink = File::create(output).with_ctx("while creating output file")?;
            lower::lower_full(&mut sink, &UnSpannedGetter::new(toks.as_slice()), exprs.iter())
                .with_ctx("while writing output file")?;
        }

        Command::Nm { input } => {
            let mut content = vec!();
            File::open(input).with_ctx("while opening input file")?
                .read_to_end(&mut content).with_ctx("while reading input file")?;
            let asm = Bytecode::try_from(content.as_slice())
                .with_ctx("while decoding input file")?;

            let mut defines = HashSet::new();
            for global in asm.named_globals() {
                let (name, pos) = global.with_ctx("while reading input file")?;
                defines.insert(name);
                println!("{:#06x} T {}", pos, name);
            }

            let mut discovered = HashSet::new();

            for code in asm.codes_in_data_table().unwrap() {
                for op in asm.const_ops(code as u32).unwrap() {
                    let op = op.unwrap().1;
                    match op {
                        Op::Unresolved { id } => {
                            discovered.insert(asm.string(id).unwrap());
                        }
                        _ => ()
                    }
                }
            }

            for ent in discovered {
                if !defines.contains(ent) {
                    println!("       t {}", ent);
                }
            }
        }

        Command::Ld { inputs, output, allow_unresolved, cat_only } => {
            let mut inputs = inputs;
            if let Some(pos) = inputs.iter().position(|x| x == &output) {
                inputs.remove(pos);
            } else {
                let mut f = File::create(&output).with_ctx("while creating output file")?;
                let header = Header::default();
                header.write(&mut f).with_ctx("while writing output file")?;
                f.flush().unwrap();
            }

            let mut out = std::fs::OpenOptions::new()
                .write(true)
                .create(false)
                .truncate(false)
                .read(true)
                .open(&output)
                .with_ctx("while opening output file")?;
            for inp in inputs.into_iter() {
                let mut inp_data = vec!();
                File::open(&inp).with_ctx("while opening input file")?
                    .read_to_end(&mut inp_data)
                    .with_ctx("while reading input file")?;

                linker::cat_together(&mut out, inp_data.as_slice())
                    .with_ctx("while linking")?;
            }

            struct TargetImpl {
                allow_unresolved: bool,
            }

            impl linker::Target for TargetImpl {
                fn allow_undeclared_symbol(&self, _: &str) -> bool {
                    return self.allow_unresolved;
                }
            }

            if !cat_only {
                out.rewind().unwrap();
                let mut bytes = vec!();
                out.read_to_end(&mut bytes).unwrap();

                linker::self_link(&mut bytes, &TargetImpl { allow_unresolved }).with_ctx("while linking")?;

                out.rewind().unwrap();
                out.write_all(bytes.as_slice()).unwrap();
            }
        }

        _ => panic!("unimplemented")
    }

    Ok(())
}
