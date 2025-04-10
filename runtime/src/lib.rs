use h6_bytecode::{Num, Op, Bytecode, ByteCodeError};
use smallvec::SmallVec;
use std::collections::{VecDeque, HashMap};

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

#[derive(Debug, Clone)]
pub enum RuntimeErrType {
    ByteCode(ByteCodeError),
    OpNotSupportType,
    StackUnderflow,
    UnlinkedSym(u32),
    ArrIdxOutOfBounds,
    ArrOpenCloseMismatch,
    SystemFnNotFound(u32),
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

pub struct Runtime<'asm> {
    pub bc: Bytecode<'asm>,
    /// absolute byte idx
    pub next: Option<usize>,
    pub stack: Vec<Value>,
    /// return stack: absolute byte idx
    pub call_stack: Vec<usize>,
    executing_array: VecDeque<Op>,

    system: HashMap<u32, (usize, Box<dyn Fn(SmallVec<Value,4>) -> Result<SmallVec<Value,4>,RuntimeErr>>)>
}

impl<'asm, 'sysfp> Runtime<'asm> {
    pub fn new(bc: Bytecode<'asm>) -> Self {
        let next = bc.main_ops_area_begin_idx();
        Self {
            bc,
            next: Some(next),
            stack: Vec::new(),
            call_stack: Vec::new(),
            executing_array: VecDeque::new(),
            system: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: u32, num_ins: usize, fp: Box<dyn Fn(SmallVec<Value,4>) -> Result<SmallVec<Value,4>,RuntimeErr>>) -> &mut Self {
        self.system.insert(name, (num_ins, fp));
        self
    }

    fn exec_op(&mut self, op: (usize, Op)) -> Result<(bool, Option<Vec<Op>>), RuntimeErr> {
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
                let mut by = vec!();
                op.write(&mut by).unwrap();

                self.call_stack.push(byte_pos + by.len());

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
                let mut by = vec!();
                op.write(&mut by).unwrap();

                let exc = pop!().as_arr()?;
                self.next = Some(byte_pos + by.len());
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
            Op::ArrEnd => {}

            Op::ArrCat => {
                let mut a = pop!().as_arr()?;
                let b = pop!().as_arr()?;
                a.extend(b.into_iter());
                self.stack.push(Value::Arr(a));
            }

            Op::ArrSkip1 => {
                let mut a = pop!().as_arr()?;
                a.remove(0);
                self.stack.push(Value::Arr(a));
            }

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

            Op::Reach { down } => {
                let v = self.stack.get(down as usize).ok_or(RuntimeErr::from(RuntimeErrType::StackUnderflow))?;
                self.stack.push(v.clone());
            }

            Op::System { id } => {
                let (narg, fp) = self.system.get(&id).ok_or(RuntimeErr::from(RuntimeErrType::SystemFnNotFound(id)))?;
                let mut args = SmallVec::new();
                for i in 0..*narg {
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
        return Ok((false, None));
    }

    /// always executes from [Runtime::next] to the next [Op::Terminate], and then updates
    /// [Runtime::next]
    pub fn step(&mut self) -> Result<Option<()>, RuntimeErr> {
        let begin_pos = match self.next {
            Some(x) => x,
            None => { return Ok(None); }
        };        

        let mut run = |get: &mut dyn FnMut(&mut Runtime) -> Option<Result<(usize, Op), RuntimeErr>>| -> Result<bool, RuntimeErr> {
            let mut breaked = false;
            while let Some(op) = get(self) {
                let op = op?;
                if op.1 == Op::ArrBegin {
                    let mut arr = ArrTy::new();
                    let mut indent = 1;
                    while indent > 0 {
                        if let Some(op) = get(self) {
                            let op = op?;
                            match op.1 {
                                Op::ArrBegin => { indent += 1; }
                                Op::ArrEnd => { indent -= 1; }
                                _ => {}
                            }
                            if indent > 0 {
                                arr.push(op.1.clone());
                            }
                        } else {
                            Err(RuntimeErr::from(RuntimeErrType::ArrOpenCloseMismatch))?;
                        }
                    }
                    self.stack.push(Value::Arr(arr));
                } else {
                    // TODO: better debug loc here
                    let (cont, exc) = self.exec_op(op)?;
                    if let Some(exc) = exc {
                        self.executing_array.extend(exc);
                    }
                    if cont {
                        breaked = true;
                        break;
                    }
                }
            }
            Ok(breaked)
        };

        let mut breaked = run(&mut |x| x.executing_array.pop_front().map(|v| Ok((0,v))))?;
        if !breaked {
            let mut iter_pos = begin_pos;
            breaked = run(&mut |rt| {
                let mut iter = h6_bytecode::OpsIter::new(iter_pos, &rt.bc.bytes[iter_pos..]);
                let v = iter.next().map(|v| v.map_err(|e| e.into()));
                iter_pos = iter.base;
                v
            })?;
        }
        if !breaked {
            self.next = self.call_stack.pop();
        }
        Ok(Some(()))
    }
}
