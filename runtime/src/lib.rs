use h6_bytecode::{Num, Op, Bytecode, ByteCodeError};
use smallvec::SmallVec;
use std::collections::VecDeque;

pub type ArrTy = SmallVec<Op, 4>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(Num),
    Arr(ArrTy)
}

impl Value {
    pub fn as_num(self) -> Result<Num, RuntimeErr> {
        match self {
            Value::Num(n) => Ok(n),
            _ => Err(RuntimeErr::from(RuntimeErrType::OpNotSupportType))
        }
    }

    pub fn as_arr(self) -> Result<ArrTy, RuntimeErr> {
        match self {
            Value::Arr(n) => Ok(n),
            _ => Err(RuntimeErr::from(RuntimeErrType::OpNotSupportType))
        }
    }
}

#[derive(Debug)]
pub enum RuntimeErrType {
    ByteCode(ByteCodeError),
    OpNotSupportType,
    StackUnderflow,
    UnlinkedSym(u32),
    ArrIdxOutOfBounds,
}

#[derive(Debug)]
pub struct RuntimeErr {
    pub ty: RuntimeErrType,
    pub asm_byte_pos: Option<usize>,
}

impl RuntimeErr {
    pub fn at(self, pos: usize) -> Self {
        RuntimeErr {
            asm_byte_pos: Some(pos),
            ..self
        }
    }
}

impl From<RuntimeErrType> for RuntimeErr {
    fn from(value: RuntimeErrType) -> Self {
        RuntimeErr {
            ty: value,
            asm_byte_pos: None
        }
    }
}

impl From<ByteCodeError> for RuntimeErr {
    fn from(value: ByteCodeError) -> Self {
        RuntimeErr {
            ty: RuntimeErrType::ByteCode(value),
            asm_byte_pos: None
        }
    }
}

pub struct Runtime<'asm> {
    pub bc: Bytecode<'asm>,
    /// absolute byte idx
    pub next: Option<usize>,
    pub stack: Vec<Value>,
    /// absolute byte idx
    pub call_stack: Vec<usize>,
    pub executing_array: VecDeque<Op>,
}

impl<'asm> Runtime<'asm> {
    pub fn new(bc: Bytecode<'asm>) -> Self {
        let next = bc.main_ops_area_begin_idx();
        Self {
            bc,
            next: Some(next),
            stack: Vec::new(),
            call_stack: Vec::new(),
            executing_array: VecDeque::new()
        }
    }

    /// always executes from [Runtime::next] to the next [Op::Terminate], and then updates
    /// [Runtime::next]
    pub fn step(&mut self) -> Result<Option<()>, RuntimeErr> {
        let begin_pos = match self.next {
            Some(x) => x,
            None => { return Ok(None); }
        };
        let ops_iter = h6_bytecode::OpsIter::new(begin_pos, &self.bc.bytes[begin_pos..]);

        let mut exec_op = |op| -> Result<(bool,_), RuntimeErr> {
            let (byte_pos, op) = op;

            macro_rules! pop {
                () => {
                    self.stack.pop().ok_or(RuntimeErr::from(RuntimeErrType::StackUnderflow).at(byte_pos))?
                };
            }

            macro_rules! num_bin {
                ($do:expr) => { {
                    let a = pop!().as_num().map_err(|x| x.at(byte_pos))?;
                    let b = pop!().as_num().map_err(|x| x.at(byte_pos))?;
                    let v = $do(a,b);
                    self.stack.push(v);
                } };
            }

            match op {
                Op::Terminate => {}
                Op::Unresolved { id } => Err(RuntimeErr::from(RuntimeErrType::UnlinkedSym(id)).at(byte_pos))?,
                Op::Const { idx } => {
                    self.call_stack.push(byte_pos);
                    self.next = Some(idx as usize + 16);
                    return Ok((true, None));
                }
                
                Op::Push { val } => { self.stack.push(Value::Num(val)) },

                Op::Add => num_bin!(|a,b| Value::Num(a + b)),
                Op::Sub => num_bin!(|a,b| Value::Num(a - b)),
                Op::Mul => num_bin!(|a,b| Value::Num(a * b)),

                Op::Dup => {
                    let v = pop!();
                    self.stack.push(v.clone());
                    self.stack.push(v);
                }

                Op::Over => {
                    let top = pop!();
                    let bot = pop!();
                    self.stack.push(bot.clone());
                    self.stack.push(top);
                    self.stack.push(bot);
                }

                Op::Swap => {
                    let top = pop!();
                    let bot = pop!();
                    self.stack.push(top);
                    self.stack.push(bot);
                }

                Op::Pop => {
                    pop!();
                }

                Op::Exec => {
                    let exc = pop!().as_arr()?;
                    return Ok((true, Some(exc.into_iter().collect::<Vec<_>>())));
                }

                Op::Select => {
                    let cond = pop!().as_num()?;
                    let a = pop!();
                    let b = pop!();
                    let v = if cond == 0 { b } else { a };
                    self.stack.push(v);
                }

                Op::Lt => num_bin!(|a,b| Value::Num((a < b).into())),
                Op::Gt => num_bin!(|a,b| Value::Num((a > b).into())),
                Op::Eq => num_bin!(|a,b| Value::Num((a == b).into())),

                Op::Not => {
                    let a = pop!().as_num()?;
                    let v = if a == 0 { 1_u8 } else { 0_u8 };
                    self.stack.push(Value::Num(v.into()));
                }

                Op::RoL => todo!(),
                Op::RoLRef => todo!(),
                Op::RoR => todo!(),
                Op::RoRRef => todo!(),

                // these are handled in the caller
                Op::ArrBegin |
                Op::ArrEnd => {}

                Op::ArrCat => todo!(),

                Op::ArrFirst => {
                    let a = pop!().as_arr()?;
                    let elts = a.get(0).ok_or(RuntimeErr::from(RuntimeErrType::ArrIdxOutOfBounds))?;
                    return Ok((true, Some(vec!(elts.clone()))));
                }

                Op::ArrLen => {
                    let a = pop!().as_arr()?;
                    self.stack.push(Value::Num((a.len() as i16).into()));
                }

                Op::Jump { idx } => {
                    self.next = Some(idx as usize + 16);
                    return Ok((true, None));
                }
            }
            return Ok((false, None));
        };
        let mut breaked = false;
        while let Some(op) = self.executing_array.pop_front() {
            // TODO: better debug loc here
            let (cont, exc) = exec_op((0, op))?;
            if let Some(exc) = exc {
                self.executing_array.extend(exc);
            }
            if cont {
                breaked = true;
                break;
            }
        }
        if !breaked {
            for op in ops_iter {
                let (cont, exc) = exec_op(op?)?;
                if let Some(exc) = exc {
                    self.executing_array.extend(exc);
                }
                if cont {
                    breaked = true;
                    break;
                }
            }
        }
        if !breaked {
            self.next = self.call_stack.pop();
        }
        Ok(Some(()))
    }
}
