use std::collections::HashMap;
use itertools::Itertools;
use crate::lex::TokStr;
use crate::parse::Expr;
use h6_bytecode::*;

pub trait Position {
    fn pos(&self) -> usize;
}

impl<T> Position for Vec<T> {
    fn pos(&self) -> usize {
        self.len()
    }
}

pub struct PosWriter<'a, W: std::io::Write> {
    pos: usize,
    sink: &'a mut W
}

impl<'a, W: std::io::Write> PosWriter<'a, W> {
    pub fn new(pos: usize, sink: &'a mut W) -> Self {
        Self { pos, sink }
    }
}

impl<W: std::io::Write> std::io::Write for PosWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.pos += buf.len();
        self.sink.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}

impl<W: std::io::Write> Position for PosWriter<'_, W> {
    fn pos(&self) -> usize {
        self.pos
    }
}

#[derive(Debug)]
pub enum SrcError {
    NotSupported
}

impl SrcError {
    fn at(self, span: std::ops::Range<usize>) -> LoweringError {
        LoweringError::CodeError { span, err: self }
    }
}

pub enum LoweringError {
    IoError(std::io::Error),
    CodeError {
        span: std::ops::Range<usize>,
        err: SrcError
    },
}

impl std::fmt::Debug for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(err) => err.fmt(f),
            Self::CodeError { span, err } => write!(f, "At {:?}: {:?}", span, err),
        }
    }
}

impl From<std::io::Error> for LoweringError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value)
    }
}

pub fn lower_full<'src: 'l, 'l, W, I>(sink: &mut W, exprs: I, pic: bool) -> Result<(), LoweringError>
where W: std::io::Write + std::io::Seek,
      I: Iterator<Item = &'l Expr<'src>>,
{
    let begin = sink.stream_position()?;
    sink.write_all(&[0_u8;16])?;
    // the Position getter should NOT INCLUDE THE 16B HEADER
    let header = lower(&mut PosWriter::new(0, sink), exprs, pic)?;
    sink.seek(std::io::SeekFrom::Start(begin))?;
    sink.write_all(&header)?;
    Ok(())
}

/// writes a bytecode assembly WITHOUT THE HEADER
/// after calling this, the HEADER HAS TO BE PREPENDED to the generated bytes
/// the Position getter should NOT INCLUDE THE 16B HEADER
pub fn lower<'src: 'l, 'l, W, I>(sink: &mut W, exprs: I, pic: bool) -> Result<[u8;16], LoweringError>
where W: std::io::Write + Position,
      I: Iterator<Item = &'l Expr<'src>>
{
    // we keep track of all previously defined bindings.
    // this will keep references to later-defined bindings as Unresolved, which the runtime's linker will resolve

    let mut globals = HashMap::<TokStr, u32>::new();
    let mut main_ops = Vec::<Op>::new();

    let resolve = |sink: &mut W, globals: &HashMap<TokStr, u32>, str: &str| -> std::io::Result<Op> {
        let resv = if pic {
            None
        } else {
            globals.get(str)
        };
        match resv {
            Some(pos) => Ok(Op::Const { idx: *pos }),

            None => {
                let p = sink.pos() as u32;
                sink.write_all(str.as_bytes())
                    .and_then(|_| sink.write_all(&[0_u8]))?;
                Ok(Op::Unresolved { id: p })
            }
        }
    };

    for expr in exprs {
        let mut write_ops = expr.val.iter()
            .map(|x| {
                match x {
                    Op::Frontend(FrontendOp::Unresolved(id)) => resolve(sink, &globals, id.as_str()),
                    x => Ok(x.clone())
                }
            }).collect::<std::io::Result<Vec<_>>>()?;

        match &expr.binding {
            Some(name) => {
                let p = sink.pos() as u32;

                for op in write_ops {
                    op.write(sink)?;
                }
                Op::Terminate.write(sink)?;

                globals.insert(name.clone(), p);
            }

            None => {
                main_ops.append(&mut write_ops);
            }
        }
    }

    let globals_tab_num = globals.len();

    let globals = globals
        .into_iter()
        .map(|(k,v)| {
            let p = sink.pos();
            sink.write_all(k.as_bytes())
                .and_then(|_| sink.write_all(&[0_u8]))
                .map(|_| {
                    let mut o = [0_u8; 8];
                    o[0..4].copy_from_slice((p as u32).to_le_bytes().as_slice());
                    o[4..8].copy_from_slice(&v.to_le_bytes().as_slice());
                    o
                })
        })
        .process_results(|x|
            x.flatten().collect::<Vec<_>>())?;

    let globals_tab_off = sink.pos();
    sink.write_all(&globals)?;

    for op in main_ops {
        op.write(sink)?;
    }
    Op::Terminate.write(sink)?;

    let header = Header {
        globals_tab_num: globals_tab_num as u16,
        globals_tab_off: globals_tab_off as u32,
        ..Default::default()
    };
    Ok(header.serialize())
}
