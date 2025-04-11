use h6_bytecode::{Num, Op, Bytecode, ByteCodeError, OpsIter};
use smallvec::SmallVec;
use std::collections::HashMap;

pub type ArrTy = SmallVec<Op, 4>;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(Num),
    Arr(ArrTy)
}

impl Value {
    pub fn disasm<'asm>(&self, bc: &Bytecode<'asm>) -> Result<String, ByteCodeError> {
        use fixed::prelude::LossyFrom;

        match self {
            Value::Num(n) => Ok(format!("{}", n)),

            Value::Arr(arr) => {
                let mut st = arr.iter()
                    .filter_map(|x| match x { Op::Push { val } => Some(val), _ => None })
                    .map(|x| i16::lossy_from(*x) as u8 as char)
                    .filter(|x| x.is_ascii() && !x.is_control())
                    .collect::<String>();

                if st.len() == arr.len() {
                    st.insert(0, '"');
                    st.push('"');
                    Ok(st)
                } else {
                    let dis = h6_bytecode::disasm::Disasm::new(bc);
                    dis.arr(arr.len(), arr.iter().map(|x| x.clone()))
                }
            }
        }
    }

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

#[derive(Debug, Clone)]
pub enum RuntimeErrType {
    ByteCode(ByteCodeError),
    OpNotSupportType,
    StackUnderflow,
    UnlinkedSym(u32),
    ArrIdxOutOfBounds,
    ArrOpenCloseMismatch,
    SystemFnNotFound(u32),
    SystemFnErr(String),
}

#[derive(Debug, Clone)]
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

pub trait InSystemFn<V> {
    fn in_system_fn(self) -> Result<V, RuntimeErr>;
}

impl<V, E: std::fmt::Debug> InSystemFn<V> for Result<V,E> {
    fn in_system_fn(self) -> Result<V, RuntimeErr> {
        self.map_err(|v| RuntimeErr::from(RuntimeErrType::SystemFnErr(format!("{:?}", v))))
    }
}

pub struct Runtime<'asm> {
    pub bc: Bytecode<'asm>,
    /// absolute byte idx
    pub next: Option<usize>,
    pub stack: Vec<Value>,

    system: HashMap<u32, (usize, Box<dyn Fn(SmallVec<Value,4>) -> Result<SmallVec<Value,4>,RuntimeErr>>)>
}

impl<'asm, 'sysfp> Runtime<'asm> {
    pub fn new(bc: Bytecode<'asm>) -> Self {
        let next = bc.header.main_ops_area_begin_idx();
        Self {
            bc,
            next: Some(next),
            stack: Vec::new(),
            system: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: u32, num_ins: usize, fp: Box<dyn Fn(SmallVec<Value,4>) -> Result<SmallVec<Value,4>,RuntimeErr>>) -> &mut Self {
        self.system.insert(name, (num_ins, fp));
        self
    }

    fn exec_iter<I: Iterator<Item = Result<(usize, Op), E>>, E>(&mut self, iter: I) -> Result<bool, RuntimeErr>
        where RuntimeErr: From<E>
    {
        let mut breaked = false;
        let mut iter = iter;
        while let Some(op) = iter.next() {
            let (pos,op) = op?;
            if op == Op::ArrBegin {
                let mut arr = SmallVec::new();
                let mut ind = 1;
                while ind > 0 {
                    let op = match iter.next() {
                        Some(x) => x?,
                        None => Err(RuntimeErrType::ArrOpenCloseMismatch)?,
                    }.1;
                    if op == Op::ArrBegin {
                        ind += 1;
                    }
                    if op == Op::ArrEnd {
                        ind -= 1;
                    }
                    if ind > 0 {
                        arr.push(op);
                    }
                }
                self.stack.push(Value::Arr(arr));
            } else {
                // TODO: better debug loc
                breaked = self.exec_op((pos, op))?;
                if breaked {
                    break;
                }
            }
        }
        Ok(breaked)
    }

    fn exec_ops(&mut self, at: usize) -> Result<bool, RuntimeErr> {
        self.exec_iter(OpsIter::new(at, &self.bc.bytes[at..]))
    }

    fn exec_arr(&mut self, arr: ArrTy) -> Result<bool, RuntimeErr> {
        self.exec_iter(arr.into_iter().map(|x| Ok::<(usize,Op),RuntimeErr>((0,x))))
    }

    fn exec_op(&mut self, op: (usize, Op)) -> Result<bool, RuntimeErr> {
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
                let v = $do(b,a);
                self.stack.push(v);
            } };
        }

        match op {
            Op::Frontend(_) => panic!(),

            Op::Terminate => {},
            Op::Unresolved { id } => Err(RuntimeErr::from(RuntimeErrType::UnlinkedSym(id)).at(byte_pos))?,
            Op::Const { idx } => {
                return self.exec_ops(idx as usize + 16);
            }
            
            Op::Push { val } => {
                self.stack.push(Value::Num(val))
            },

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
                let mut by = vec!();
                op.write(&mut by).unwrap();

                let exc = pop!().as_arr()?;
                return self.exec_arr(exc);
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

            Op::RoL => {
                let t0 = pop!();
                let t1 = pop!();
                let t2 = pop!();

                self.stack.push(t1);
                self.stack.push(t0);
                self.stack.push(t2);
            }

            Op::RoR => {
                let t0 = pop!();
                let t1 = pop!();
                let t2 = pop!();

                self.stack.push(t0);
                self.stack.push(t2);
                self.stack.push(t1);
            }

            // these are handled in the caller
            Op::ArrBegin |
            Op::ArrEnd => panic!(),

            Op::ArrCat => {
                let b = pop!().as_arr()?;
                let mut a = pop!().as_arr()?;
                a.extend(b.into_iter());
                self.stack.push(Value::Arr(a));
            }

            Op::ArrSkip1 => {
                let mut a = pop!().as_arr()?;
                a.remove(0);
                self.stack.push(Value::Arr(a));
            }

            Op::ArrFirst => {
                let mut a = pop!().as_arr()?;
                let elt = a.get_mut(0)
                    .ok_or(RuntimeErr::from(RuntimeErrType::ArrIdxOutOfBounds))?
                    .to_owned();
                return self.exec_op((0, elt));
            }

            Op::ArrLen => {
                let a = pop!().as_arr()?;
                self.stack.push(Value::Num((a.len() as i16).into()));
            }

            Op::Jump { idx } => {
                self.next = Some(idx as usize + 16);
                return Ok(true);
            }

            Op::Reach { down } => {
                let pos = self.stack.len()
                    .checked_sub(1)
                    .and_then(|x| x.checked_sub(down as usize))
                    .ok_or(RuntimeErr::from(RuntimeErrType::StackUnderflow))?;
                let v = self.stack.get(pos).ok_or(RuntimeErr::from(RuntimeErrType::StackUnderflow))?;
                self.stack.push(v.clone());
            }

            Op::System { id } => {
                let (narg, fp) = self.system.get(&id).ok_or(RuntimeErr::from(RuntimeErrType::SystemFnNotFound(id)))?;
                let mut args = SmallVec::new();
                for _ in 0..*narg {
                    args.push(pop!());
                }
                let outs = fp(args)?;
                self.stack.extend(outs.into_iter());
            }

            Op::Pack => {
                let v = pop!().as_num()?;
                let arr = smallvec::smallvec!(Op::Push { val: v });
                self.stack.push(Value::Arr(arr));
            }
        }
        return Ok(false);
    }

    /// always executes from [Runtime::next] to the next [Op::Terminate], and then updates
    /// [Runtime::next]
    pub fn step(&mut self) -> Result<Option<()>, RuntimeErr> {
        let begin_pos = match self.next {
            Some(x) => x,
            None => { return Ok(None); }
        };

        let breaked = self.exec_ops(begin_pos)?;
        if !breaked {
            self.next = None;
        }
        Ok(Some(()))
    }
}
