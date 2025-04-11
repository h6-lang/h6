use crate::{Op, Bytecode, ByteCodeError, OpsIter};

pub struct Disasm<'bc, 'asm> {
    asm: &'bc Bytecode<'asm>,
}

impl<'bc, 'asm> Disasm<'bc, 'asm> {
    pub fn new(asm: &'bc Bytecode<'asm>) -> Self {
        Self { asm }
    }

    pub fn constant(&self, pos: u32) -> Result<String, ByteCodeError> {
        let pos = pos as usize;
        let by = &self.asm.data_table()[pos..];
        let ops = OpsIter::new(pos + 16, by)
            .map(|vp| vp.map(|(_,b)| b))
            .collect::<Result<Vec<Op>, _>>()?;
        Ok(self.ops(ops.into_iter())?)
    }

    pub fn arr<I: Iterator<Item = Op>>(&self, len: usize, iter: I) -> Result<String, ByteCodeError> {
        let mut out = String::new();
        if len == 0 {
            out.push_str("{} ");
        } else {
            out.push_str("{ ");
            out.push_str(self.ops(iter)?.as_str());
            out.push_str("} ");
        }
        Ok(out)
    }

    pub fn ops<I: Iterator<Item = Op>>(&self, iter: I) -> Result<String, ByteCodeError> {
        let mut iter = iter;
        let mut out = String::new();

        while let Some(op) = iter.next() {
            if op == Op::ArrBegin {
                let mut items = Vec::new();
                let mut ind = 1;

                while ind > 0 {
                    let item = iter.next().ok_or(ByteCodeError::ArrEndMismatch)?;
                    match item {
                        Op::ArrBegin => { ind += 1; }
                        Op::ArrEnd => { ind -= 1; }
                        _ => {}
                    }

                    if ind > 0 {
                        items.push(item);
                    }
                }

                out.push_str(self.arr(items.len(), items.into_iter())?.as_str());
            } else {
                out.push_str(self.op(&op)?.as_str());
                out.push_str(" ");
            }
        }

        Ok(out)
    } 

    pub fn op(&self, op: &Op) -> Result<String, ByteCodeError> {
        Ok(match op {
            Op::Terminate |
            Op::Unresolved { .. } |
            Op::ArrBegin |
            Op::ArrEnd |
            Op::Frontend(_) => "".to_string(),

            Op::Jump { idx } => format!("jump{}", idx),
            Op::Const { idx } => format!("const{}", idx), // NOT DISASSEMBLING FOR NOW BECAUSE
                                                          // INFINITE RECURSION
            Op::Push { val } => format!("{}", val),
            Op::System { id } => format!("system{}", id),

            Op::Add => format!("+"),
            Op::Sub => format!("-"),
            Op::Mul => format!("*"),
            Op::Dup => format!("."),
            Op::Over => format!(","),
            Op::Swap => format!("$"),
            Op::Pop => format!(";"),
            Op::Exec => format!("!"),
            Op::Select => format!("?"),
            Op::Lt => format!("<"),
            Op::Gt => format!(">"),
            Op::Eq => format!("="),
            Op::Not => format!("~"),
            Op::RoL => format!("l"),
            Op::RoR => format!("r"),

            Op::Reach { down } => format!("reach{}", down),

            Op::ArrCat => format!("@+"),
            Op::ArrFirst => format!("@0"),
            Op::ArrSkip1 => format!("@<"),
            Op::ArrLen => format!("@*"),
            Op::Pack => format!("_"),
        })
    }
}
