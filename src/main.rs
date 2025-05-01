use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::{Read, Seek, Write};
use clap::{Parser, Subcommand};
use camino::Utf8PathBuf;
use std::rc::Rc;
use std::cell::RefCell;
use h6_bytecode::{Bytecode, Header, Op, linker};
use h6_compiler::{lex, parse, lower};

#[cfg(feature = "repl")]
use reedline::{Highlighter, Hinter, Validator};

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

    /// interactive playground
    Repl {
        import: Vec<Utf8PathBuf>
    },

    /// disassemble
    Dis {
        file: Utf8PathBuf
    }
}

struct HumanError {
    ty: HumanErrorTy,
    ctx: Option<String>,
}

enum HumanErrorTy {
    IOError(std::io::Error),
    LinkError(linker::LinkError),
    ByteCodeError(h6_bytecode::ByteCodeError),
    RuntimeError(h6_runtime::RuntimeErr),
    LoweringError(h6_compiler::lower::LoweringError),
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

impl From<h6_runtime::RuntimeErr> for HumanErrorTy {
    fn from(value: h6_runtime::RuntimeErr) -> Self {
        HumanErrorTy::RuntimeError(value)
    }
}

impl From<h6_compiler::lower::LoweringError> for HumanErrorTy {
    fn from(value: h6_compiler::lower::LoweringError) -> Self {
        HumanErrorTy::LoweringError(value)
    }
}

impl std::fmt::Debug for HumanErrorTy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HumanErrorTy::IOError(err) => write!(f, "I/O Error: {:?}", err),
            HumanErrorTy::LinkError(err) => write!(f, "Linker Error: {:?}", err),
            HumanErrorTy::ByteCodeError(err) => write!(f, "Bytecode Decode Error: {:?}", err),
            HumanErrorTy::RuntimeError(err) => write!(f, "{:?}", err),
            HumanErrorTy::LoweringError(err) => write!(f, "{:?}", err),
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

struct RT {
    
}

impl Default for RT {
    fn default() -> Self {
        Self {}
    }
}

fn register_runtime(rt: &mut h6_runtime::Runtime, _rtio: Rc<RefCell<RT>>) {
    use smallvec::smallvec;
    use fixed::prelude::LossyFrom;
    use h6_runtime::{Value, InSystemFn};

    // write bytes to stream
    rt.register(0, 2, Box::new(|args| {
        let mut args = args.into_iter();
        let byte = i32::lossy_from(args.next().unwrap().as_num()?) as u8;
        let stream = args.next().unwrap().as_num()?;
        if stream != 1 { panic!(); }
        std::io::stdout().write_all(&[byte]).unwrap();
        std::io::stdout().flush().unwrap();
        Ok(smallvec!())
    }));

    // read byte from stream
    rt.register(1, 1, Box::new(|args| {
        let mut args = args.into_iter();
        let stream = args.next().unwrap().as_num()?;
        if stream != 1 { panic!(); }
        let mut by = [0_u8;1];
        std::io::stdin().read_exact(&mut by).in_system_fn()?;
        let n = Value::Num(by[0].into());
        Ok(smallvec!(n))
    }));
}

#[cfg(feature = "repl")]
struct Highl {}

#[cfg(feature = "repl")]
impl Highlighter for Highl {
    fn highlight(&self, line: &str, _cursor: usize) -> reedline::StyledText {
        use chumsky::prelude::*;
        use nu_ansi_term::{Style, Color};
        use lex::TokType;

        let mut style = reedline::StyledText::new();
        style.push((Style::new(), line.to_string()));
        let (toks, _) = lex::lexer().parse(line).into_output_errors();
        if let Some(toks) = toks {
            for tk in toks {
                let (tk, span) = tk;
                let st = match (&tk).into() {
                    TokType::Num => Style::new().fg(Color::LightBlue),
                    TokType::Str => Style::new().fg(Color::LightGreen),
                    TokType::Ident => Style::new().fg(Color::Cyan),
                    TokType::Point => Style::new().fg(Color::LightYellow),
                    TokType::Op => Style::new().fg(Color::Magenta),
                    TokType::Comment => Style::new().fg(Color::DarkGray),
                    TokType::Err => Style::new().fg(Color::Red).underline(),
                };
                style.style_range(span.start, span.end, st);
            }
        }
        style
    }
}

#[cfg(feature = "repl")]
struct Hint {
    completing_curly_close: bool
}

#[cfg(feature = "repl")]
impl Default for Hint {
    fn default() -> Self {
        Hint {
            completing_curly_close: false
        }
    }
}

#[cfg(feature = "repl")]
impl Hinter for Hint {
    fn handle(
        &mut self,
        line: &str,
        _pos: usize,
        _history: &dyn reedline::History,
        _use_ansi_coloring: bool,
        _cwd: &str,
    ) -> String {
        use chumsky::prelude::*;

        self.completing_curly_close = false;
        let (toks, _) = lex::lexer().parse(line).into_output_errors();
        if let Some(toks) = toks {
            let mut ind = 0;
            for tok in toks.iter() {
                match tok.0 {
                    lex::Tok::CurlyOpen => { ind += 1; },
                    lex::Tok::CurlyClose => { ind -= 1; },
                    _ => ()
                }
            }
            if ind > 0 {
                self.completing_curly_close = true;
                return " }".to_string();
            }
        }
        String::new()
    }

    fn complete_hint(&self) -> String {
        if self.completing_curly_close {
            " }".to_string()
        } else {
            String::new()
        }
    }

    fn next_hint_token(&self) -> String {
        "".to_string()
    }
}

#[cfg(feature = "repl")]
struct Validd {}

#[cfg(feature = "repl")]
impl Validator for Validd {
    fn validate(&self, line: &str) -> reedline::ValidationResult {
        use chumsky::prelude::*;

        let (toks, _) = lex::lexer().parse(line).into_output_errors();
        if let Some(toks) = toks {
            let mut ind = 0;
            for tok in toks.iter() {
                match tok.0 {
                    lex::Tok::CurlyOpen => { ind += 1; },
                    lex::Tok::CurlyClose => { ind -= 1; },
                    _ => ()
                }
            }
            if ind > 0 {
                return reedline::ValidationResult::Incomplete;
            }
        }
        reedline::ValidationResult::Complete
    }
}

fn print_stack(bc: &Bytecode, stack: &Vec<h6_runtime::Value>) {
    if stack.len() > 1 {
        println!("bot");
    }
    for x in stack.iter() {
        println!("  {}", x.disasm(bc).unwrap_or_else(|_| "<invalid value>".to_string()));
    }
    if stack.len() > 1 {
        println!("top");
    }
}

fn dis(asm: &Bytecode) -> Result<(), HumanError> {
    let dis = h6_bytecode::disasm::Disasm::new(asm);

    let mut globals_lut = HashMap::new();
    println!("globals:");
    for global in asm.named_globals() {
        let (name, addr) = global.with_ctx("decoding")?;
        println!("  {} \tdata+{} (={})", name, addr, addr as usize + 16);
        globals_lut.insert(addr, name);
    }
    println!("");

    println!("dso references:");
    for dso in asm.dso_names().with_ctx("read dso data")? {
        let str = asm.string(dso).with_ctx("read dso data")?;
        println!("  {} \t{}", dso, str);
    }
    println!("");

    for rel_pos in asm.codes_in_data_table().with_ctx("decoding")?.into_iter() {
        let abs_pos = rel_pos + 16;
        let name = globals_lut.get(&(rel_pos as u32))
            .map(|v| *v)
            .unwrap_or("????");
        println!("data+{} (={}) : {}", rel_pos, abs_pos, name);
        println!("  {}", dis.absolute_ops(abs_pos).with_ctx("decoding")?);
        println!("");
    }

    let main_beg = asm.header.main_ops_area_begin_idx();
    println!("main (={})", main_beg);
    println!("  {}", dis.absolute_ops(main_beg).with_ctx("decoding")?);
    println!("");

    Ok(())
}

fn val_unlink(val: h6_runtime::Value, bc: &Bytecode) -> Result<h6_runtime::Value, HumanError> {
    let globals = bc.named_globals()
        .collect::<Result<Vec<_>,h6_bytecode::ByteCodeError>>()
        .unwrap();

    match val {
        h6_runtime::Value::Arr(arr) => Ok(h6_runtime::Value::Arr(arr.into_iter()
            .map(|op| {
                match op {
                    Op::Const { idx } => {
                        let v = globals.iter().find(|x| x.1 == idx)
                            .unwrap().0;
                        Op::Frontend(h6_bytecode::FrontendOp::Unresolved(v.to_string()))
                    }

                    _ => op
                }
            })
            .collect::<h6_runtime::ArrTy>())),

        h6_runtime::Value::Num(_) => Ok(val)
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
            lower::lower_full(&mut sink, exprs.iter(), false)
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
                println!("       t {}", ent);
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
                Op::Terminate.write(&mut f).with_ctx("while writing output file")?;
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

        Command::Run { input } => {
            let mut content = vec!();
            File::open(input).with_ctx("while opening input file")?
                .read_to_end(&mut content).with_ctx("while reading input file")?;
            let asm = Bytecode::try_from(content.as_slice())
                .with_ctx("while decoding input file")?;

            let mut rt = h6_runtime::Runtime::new(asm).unwrap();
            register_runtime(&mut rt, Rc::new(RefCell::new(RT::default())));

            while let Some(_) = rt.step().with_ctx("exec")? {}

            print_stack(&rt.bc, &rt.stack.into());
        }

        Command::Dis { file } => {
            let mut content = vec!();
            File::open(file).with_ctx("while opening input file")?
                .read_to_end(&mut content).with_ctx("while reading input file")?;

            let asm = Bytecode::try_from(content.as_slice())
                .with_ctx("while decoding input file")?;
            dis(&asm)?;
        }

        #[cfg(not(feature = "repl"))]
        Command::Repl { .. } => {
            eprintln!("cli was built without 'repl' feature!");
            std::process::exit(1);
        }

        #[cfg(feature = "repl")]
        Command::Repl { import } => {
            use reedline::{Reedline, DefaultPrompt, MenuBuilder};
            use std::collections::HashMap;
            use h6_compiler::parse::Expr;


            let mut stack = Vec::<h6_runtime::Value>::new();
            let mut defines = HashMap::<String, smallvec::SmallVec<Op, 8>>::new();

            for path in import.into_iter() {
                let content = std::fs::read_to_string(&path).with_ctx("reading input file")?;

                let toks = lex::lex(content.as_str())
                    .unwrap_or_else(|errs| {
                        for err in errs {
                            eprintln!("({} lexer) {:#?}", path.as_str(), err);
                        }
                        std::process::exit(1);
                    });

                let exprs = parse::parse(toks.iter().map(|x| x.0.clone()))
                    .unwrap_or_else(|errs| {
                        for err in errs {
                            eprintln!("({} parser) {:#?}", path.as_str(), err);
                        }
                        std::process::exit(1);
                    });

                for expr in exprs.into_iter() {
                    if let Some(def) = &expr.binding {
                        defines.insert(def.to_string(), expr.val.clone());
                    }
                }
            }

            // TODO: completer needs to know where tokens begin, so this gets completed too:
            // "hi"pri<tab>

            let completion_menu = Box::new(reedline::ColumnarMenu::default().with_name("completion_menu"));
            let mut keybindings = reedline::default_emacs_keybindings();
            keybindings.add_binding(
                reedline::KeyModifiers::NONE,
                reedline::KeyCode::Tab,
                reedline::ReedlineEvent::UntilFound(vec![
                    reedline::ReedlineEvent::Menu("completion_menu".to_string()),
                    reedline::ReedlineEvent::MenuNext,
                ]),
            );
            let edit_mode = Box::new(reedline::Emacs::new(keybindings));

            let mut editor = Reedline::create()
                .with_highlighter(Box::new(Highl {}))
                .with_hinter(Box::new(Hint::default()))
                .with_validator(Box::new(Validd {}))
                .with_menu(reedline::ReedlineMenu::EngineCompleter(completion_menu))
                .with_edit_mode(edit_mode);
            let prompt = DefaultPrompt::default();

            let mut ctrlc = 0;
            loop {
                let mut autocomp = vec!();
                for (sym,_) in defines.iter() {
                    autocomp.push(sym.clone());
                }
                let compl = Box::new(reedline::DefaultCompleter::new_with_wordlen(autocomp, 2));
                editor = editor.with_completer(compl);
                let sig = editor.read_line(&prompt).unwrap();
                match sig {
                    reedline::Signal::CtrlC => {
                        ctrlc += 1;
                        if ctrlc == 2 {
                            break;
                        }
                    },

                    reedline::Signal::CtrlD => break,

                    reedline::Signal::Success(text) => {
                        ctrlc = 0;

                        match lex::lex(text.as_str()) {
                            Ok(toks) => {
                                if let Ok(exprs) = parse::parse(toks.iter().map(|x| x.0.clone()))
                                    .inspect_err(|errs| {
                                        for err in errs {
                                            eprintln!("{:#?}", err);
                                        }
                                    })
                                {
                                    for e in &exprs {
                                        if let Some(e) = &e.binding {
                                            defines.remove(e.as_ref());
                                        }
                                    }
                                    let mut all = vec!();
                                    for val in stack.drain(0..) {
                                        let ops = val.into_ops();
                                        let e = h6_compiler::parse::Expr {
                                            tok_span: 0..0,
                                            val: ops.into_iter().collect(),
                                            ..Default::default()
                                        };
                                        all.push(e);
                                    }
                                    all.extend(exprs.into_iter());
                                    for (k, v) in defines.iter() {
                                        all.push(Expr {
                                            tok_span: 0..0,
                                            binding: Some(k.clone().into()),
                                            val: v.clone(),
                                            ..Default::default()
                                        });
                                    }

                                    struct TargetImpl {}

                                    impl linker::Target for TargetImpl {
                                        fn allow_undeclared_symbol(&self, _: &str) -> bool {
                                            return false;
                                        }
                                    }

                                    let mut bytes = vec!();
                                    let header = h6_compiler::lower::lower(&mut bytes, all.iter(), true).unwrap();
                                    bytes.splice(0..0, header.into_iter());

                                    if let Ok(_) = h6_bytecode::linker::self_link(bytes.as_mut_slice(), &TargetImpl {})
                                        .inspect_err(|err| {
                                            eprintln!("linker error: {:?}", err);
                                        })
                                    {
                                        let bc = Bytecode::try_from(bytes.as_slice()).unwrap();
                                        let mut rt = h6_runtime::Runtime::new(bc).unwrap();
                                        register_runtime(&mut rt, Rc::new(RefCell::new(RT::default())));

                                        while let Ok(Some(_)) = rt.step().with_ctx("exec").inspect_err(|e| { eprintln!("{:?}", e); }) {}
                                        stack = Into::<Vec<_>>::into(rt.stack)
                                            .into_iter()
                                            .map(|x| val_unlink(x, &rt.bc))
                                            .collect::<Result<Vec<h6_runtime::Value>,HumanError>>()?;
                                        print_stack(&rt.bc, &stack);

                                        defines = all.iter()
                                            .filter_map(|x| match &x.binding {
                                                Some(bind) => Some((bind.to_string(), x.val.clone())),
                                                None => None,
                                            })
                                            .collect();
                                    }
                                }
                            }

                            Err(errs) => {
                                for err in errs {
                                    eprintln!("{:#?}", err);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
