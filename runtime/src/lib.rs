use h6_bytecode::{Num, Op, Bytecode, ByteCodeError, OpsIter};
use smallvec::SmallVec;
use std::collections::{HashMap, VecDeque};

pub type ArrTy = SmallVec<Op, 4>;

#[derive(Debug)]
enum SpecialOp {
    Push(Value)
}

impl h6_bytecode::RuntimeOp for SpecialOp {
    fn enum_id(&self) -> usize {
        match self {
            SpecialOp::Push(_) => 0
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(Num),
    Arr(ArrTy)
}

impl Value {
    /// for [Value::Num], generates: [Push(val)]
    /// for [Value::Arr], generates: [BeginArr, ..., EndArr]
    pub fn into_ops(self) -> ArrTy {
        match self {
            Value::Num(v) => smallvec::smallvec!(Op::Push { val: v }),
            Value::Arr(v) => {
                let mut v = v;
                v.insert(0, Op::ArrBegin);
                v.push(Op::ArrEnd);
                v
            }
        }
    }

    pub fn rt_ty_id(&self) -> u8 {
        match self {
            Value::Num(_) => 0,
            Value::Arr(_) => 1,
        }
    }

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
    pub stack: Vec<Value>,
    pub todo: VecDeque<Op>,

    system: HashMap<u32, (usize, Box<dyn Fn(SmallVec<Value,4>) -> Result<SmallVec<Value,4>,RuntimeErr>>)>
}

impl<'asm, 'sysfp> Runtime<'asm> {
    pub fn new(bc: Bytecode<'asm>) -> Result<Self, RuntimeErr> {
        let begin = bc.header.main_ops_area_begin_idx();
        let mut o = Self {
            bc,
            stack: Vec::new(),
            todo: VecDeque::new(),
            system: HashMap::new(),
        };
        o.exec_ops(begin)?;
        Ok(o)
    }

    pub fn register(&mut self, name: u32, num_ins: usize, fp: Box<dyn Fn(SmallVec<Value,4>) -> Result<SmallVec<Value,4>,RuntimeErr>>) -> &mut Self {
        self.system.insert(name, (num_ins, fp));
        self
    }

    fn exec_iter<I: Iterator<Item = Result<(usize, Op), E>>, E>(&mut self, iter: I) -> Result<(), RuntimeErr>
        where RuntimeErr: From<E>
    {
        let mut todo = vec!();
        let mut iter = iter;
        while let Some(op) = iter.next() {
            let (_, op) = op?;
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
                todo.push(Op::Runtime(h6_bytecode::RuntimeOpWrapper(std::rc::Rc::new(SpecialOp::Push(Value::Arr(arr))))));
            } else {
                todo.push(op);
            }
        }
        for x in todo.into_iter().rev() {
            self.todo.push_front(x);
        }
        Ok(())
    }

    fn exec_ops(&mut self, at: usize) -> Result<(), RuntimeErr> {
        self.exec_iter(OpsIter::new(at, &self.bc.bytes[at..]))
    }

    fn exec_arr(&mut self, arr: ArrTy) -> Result<(), RuntimeErr> {
        self.exec_iter(arr.into_iter().map(|x| Ok::<(usize,Op),RuntimeErr>((0,x))))
    }

    fn arr_first_elem_len<I: Iterator<Item = Op>>(arr: I) -> Result<usize, RuntimeErr> {
        let mut arr = arr;
        if let Some(op) = arr.next() {
            if op == Op::ArrBegin {
                let mut len = 1;
                let mut ind = 1;
                while ind > 0 {
                    let op = arr.next()
                        .ok_or(RuntimeErr::from(RuntimeErrType::ArrOpenCloseMismatch))?;
                    match op {
                        Op::ArrBegin => { ind += 1; }
                        Op::ArrEnd => { ind -= 1; }
                        _ => {}
                    }
                    len += 1;
                }
                Ok(len)
            } else {
                Ok(1)
            }
        } else {
            Ok(0)
        }
    }

    fn exec_op(&mut self, op: (usize, Op)) -> Result<(), RuntimeErr> {
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
            Op::Runtime(rt) => {
                let op = rt.0.as_ref().as_any().downcast_ref::<SpecialOp>().unwrap();
                match op {
                    SpecialOp::Push(v) => {
                        self.stack.push(v.clone());
                    }
                }
            }

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

            // TODO: both skip and first: need exec arr and get oops

            Op::ArrSkip1 => {
                let mut a = pop!().as_arr()?;
                let len = Self::arr_first_elem_len(a.iter().map(|x| x.clone()))?;
                a.drain(0..len);
                self.stack.push(Value::Arr(a));
            }

            Op::ArrFirst => {
                let mut a = pop!().as_arr()?;
                let len = Self::arr_first_elem_len(a.iter().map(|x| x.clone()))?;
                if len == 0 {
                    Err(RuntimeErr::from(RuntimeErrType::ArrIdxOutOfBounds))?;
                }
                a.truncate(len);
                return self.exec_arr(a);
            }

            Op::ArrLen => {
                let a = pop!().as_arr()?;
                self.stack.push(Value::Num((a.len() as i16).into()));
            }

            Op::Jump { idx } => {
                return self.exec_ops(idx as usize + 16);
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
                let v = pop!();
                self.stack.push(Value::Arr(v.into_ops()));
            }

            Op::TypeId => {
                let v = pop!();
                self.stack.push(Value::Num(v.rt_ty_id().into()));
            }
        }
        return Ok(());
    }

    /// always executes one instruction at a time
    pub fn step(&mut self) -> Result<Option<()>, RuntimeErr> {
        match self.todo.pop_front() {
            Some(op) => {
                Ok(Some(self.exec_op((0, op))?))
            }

            None => {
                Ok(None)
            }
        }
    }
}
