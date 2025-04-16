use crate::{Op, Bytecode, ByteCodeError, OpsIter};

pub struct Disasm<'bc, 'asm> {
    asm: &'bc Bytecode<'asm>,
}

// TODO: disasm shouldn't fail if bytecode semi-invalid

impl<'bc, 'asm> Disasm<'bc, 'asm> {
    pub fn new(asm: &'bc Bytecode<'asm>) -> Self {
        Self { asm }
    }

    pub fn absolute_ops(&self, pos: usize) -> Result<String, ByteCodeError> {
        let by = &self.asm.bytes[pos..];
        let ops = OpsIter::new(pos, by)
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
            Op::Runtime(rt) => format!("<rt typeid={:?} enum={}>", rt.0.type_id(), rt.0.enum_id()),

            Op::Terminate |
            Op::ArrBegin |
            Op::ArrEnd => "<wtf>".to_string(),

            Op::Unresolved { id } => format!("<unresolved: \"{}\">", self.asm.string(*id)?),
            Op::Frontend(v) => format!("<frontend: {:?}>", v),

            Op::Jump { idx } => format!("<jump: data+{}>", idx),
            Op::Const { idx } => format!("<const: data+{}>", idx), // NOT DISASSEMBLING FOR NOW BECAUSE
                                                                   // INFINITE RECURSION
            Op::Push { val } => format!("{}", val),
            Op::System { id } => format!("<system: {}>", id),
            Op::TypeId => format!("<typeid>"),
            Op::Materialize => format!("<materialize>"),

            Op::Add => format!("+"),
            Op::Sub => format!("-"),
            Op::Mul => format!("*"),
            Op::Mod => format!("%"),
            Op::Div => format!("/"),
            Op::Fract => format!("<fract>"),
            Op::Dup => format!("."),
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

            Op::Reach { down } => format!("<reach: {}>", down),

            Op::ArrCat => format!("@+"),
            Op::ArrFirst => format!("@0"),
            Op::ArrSkip1 => format!("@<"),
            Op::ArrLen => format!("@*"),
            Op::Pack => format!("_"),
        })
    }
}
