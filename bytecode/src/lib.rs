use std::fmt::{Debug, Formatter};
use std::io::Write;
use std::ops::Range;
use int_enum::IntEnum;

pub type Num = fixed::types::I16F16;

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// used to mark end of constant or code
    Terminate,

    /// after parsing: this is the token id!
    /// in bytecode: offset into table
    Unresolved { id: u32 },

    /// offset into table
    Const { idx: u32 },

    Push { val: Num },

    Add,
    Sub,
    Mul,
    Dup,
    Over,
    Swap,
    Pop,
    Exec,
    Select,
    Lt,
    Gt,
    Eq,
    Not,
    RoL,
    RoLRef,
    RoR,
    RoRRef,

    /// all bytecode ops until the corresponding ArrEnd will be collected into an array
    ArrBegin,
    ArrEnd,

    ArrCat,
    ArrFirst,
    ArrLen,

    Jump { idx: u32 },
}

impl Into<OpType> for &Op {
    fn into(self) -> OpType {
        match self {
            Op::Terminate => OpType::Terminate,
            Op::Unresolved { .. } => OpType::Unresolved,
            Op::Const { .. } => OpType::Const,
            Op::Push { .. } => OpType::Push,
            Op::Add => OpType::Add,
            Op::Sub => OpType::Sub,
            Op::Mul => OpType::Mul,
            Op::Dup => OpType::Dup,
            Op::Over => OpType::Over,
            Op::Swap => OpType::Swap,
            Op::Pop => OpType::Pop,
            Op::Exec => OpType::Exec,
            Op::Select => OpType::Select,
            Op::Lt => OpType::Lt,
            Op::Gt => OpType::Gt,
            Op::Eq => OpType::Eq,
            Op::Not => OpType::Not,
            Op::RoL => OpType::RoL,
            Op::RoLRef => OpType::RoLRef,
            Op::RoR => OpType::RoR,
            Op::RoRRef => OpType::RoRRef,
            Op::ArrBegin => OpType::ArrBegin,
            Op::ArrEnd => OpType::ArrEnd,
            Op::ArrCat => OpType::ArrCat,
            Op::ArrFirst => OpType::ArrFirst,
            Op::ArrLen => OpType::ArrLen,
            Op::Jump { .. } => OpType::Jump,
        }
    }
}

impl Op {
    pub fn write<W: Write>(&self, to: &mut W) -> std::io::Result<()> {
        let ty: OpType = self.into();
        to.write_all(&[ty as u8])?;
        match self {
            Op::Unresolved { id } => to.write_all(&id.to_le_bytes())?,
            Op::Const { idx } => to.write_all(&idx.to_le_bytes())?,
            Op::Push { val } => to.write_all(&val.to_le_bytes())?,
            _ => (),
        }
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, IntEnum)]
#[repr(u8)]
pub enum OpType {
    Terminate = 0,
    Unresolved = 1,
    Const = 2,
    Push = 8,

    Add = 9,
    Sub = 10,
    Mul = 11,
    Dup = 12,
    Over = 13,
    Swap = 14,
    Pop = 15,
    Exec = 16,
    Select = 17,
    Lt = 18,
    Gt = 19,
    Eq = 20,
    Not = 21,
    RoL = 22,
    RoLRef = 23,
    RoR = 24,
    RoRRef = 25,

    ArrBegin = 26,
    ArrEnd = 27,
    ArrCat = 29,
    ArrFirst = 30,
    ArrLen = 31,

    Jump = 40,
}

impl OpType {
    pub fn has_param(&self) -> bool {
        match self {
            OpType::Unresolved => true,
            OpType::Const => true,
            OpType::Push => true,
            _ => false,
        }
    }

    /// input slice can be longer than required
    /// returns weather or not had param
    pub fn read(bytes: &[u8]) -> Result<(bool, Op), ByteCodeError> {
        let opty = OpType::try_from(*bytes.get(0).ok_or(ByteCodeError::NotEnoughBytes)?)
            .map_err(|_| ByteCodeError::UnknownOpcode)?;

        let arg = bytes.get(1..5)
            .map(|x| {
                let mut a = [0_u8;4];
                a.clone_from_slice(x);
                a
            });

        Ok((opty.has_param(), match opty {
            OpType::Terminate => Op::Terminate,
            OpType::Unresolved => Op::Unresolved { id: u32::from_le_bytes(arg.ok_or(ByteCodeError::NotEnoughBytes)?) },
            OpType::Const => Op::Const { idx: u32::from_le_bytes(arg.ok_or(ByteCodeError::NotEnoughBytes)?) },
            OpType::Push => Op::Push { val: Num::from_le_bytes(arg.ok_or(ByteCodeError::NotEnoughBytes)?) },
            OpType::Add => Op::Add,
            OpType::Sub => Op::Sub,
            OpType::Mul => Op::Mul,
            OpType::Dup => Op::Dup,
            OpType::Over => Op::Over,
            OpType::Swap => Op::Swap,
            OpType::Pop => Op::Pop,
            OpType::Exec => Op::Exec,
            OpType::Select => Op::Select,
            OpType::Lt => Op::Lt,
            OpType::Gt => Op::Gt,
            OpType::Eq => Op::Eq,
            OpType::Not => Op::Not,
            OpType::RoL => Op::RoL,
            OpType::RoLRef => Op::RoLRef,
            OpType::RoR => Op::RoR,
            OpType::RoRRef => Op::RoRRef,
            OpType::ArrBegin => Op::ArrBegin,
            OpType::ArrEnd => Op::ArrEnd,
            OpType::ArrCat => Op::ArrCat,
            OpType::ArrFirst => Op::ArrFirst,
            OpType::ArrLen => Op::ArrLen,
            OpType::Jump => Op::Jump { idx: u32::from_le_bytes(arg.ok_or(ByteCodeError::NotEnoughBytes)?) },
        }))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    /// offset into str tab
    pub name: u32,

    /// offset into const array
    pub const_id: u32,
}

/// header (16 bytes)
///   magic:   4 * u8 = "H6H6"
///   min_reader_version: u8     = 1
///   reserved: u8 = 0
///   globals table num entries: u16_le
///   offset to globals table in code tab: u32_le
///   reserved: u32 = 0
///
/// code = string = const table:
///   multiple of either:
///     string:
///       utf8, null terminated
///     code / constant:
///       multiple ops
///       "Terminate" op
///
/// globals table:
///   multiple entries:
///     name:  u32_le (byte offset into string table)
///     value: u32_le (byte offset into const table)
/// executing code (kinda like main() function)
///   multiple ops
///   "Terminate" op
///
///
///
/// op:
///   id: u8
///   for specific ops:
///     param: u32_le
///
pub struct Bytecode<'asm> {
    bytes: &'asm [u8],

    globals_tab_num: u16,

    /// relative to code/string/const table!!
    globals_tab_off: u32,
}

/// until terminate
pub struct OpsIter<'asm> {
    bytes: Result<Option<&'asm [u8]>, ByteCodeError>
}

impl<'asm> OpsIter<'asm> {
    pub fn new(bytes: &'asm [u8]) -> Self {
        Self {
            bytes: Ok(Some(bytes)),
        }
    }
}

impl<'asm> Iterator for OpsIter<'asm> {
    type Item = Result<Op, ByteCodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.bytes {
            Ok(opt_bytes) => match opt_bytes {
                None => None,

                Some(bytes) => {
                    match OpType::read(bytes) {
                        Ok((had_param, op)) => {
                            if op == Op::Terminate {
                                self.bytes = Ok(None);
                                None
                            } else {
                                if had_param {
                                    self.bytes = Ok(Some(&bytes[5..]));
                                } else {
                                    self.bytes = Ok(Some(&bytes[1..]));
                                }
                                Some(Ok(op))
                            }
                        }

                        Err(e) => {
                            self.bytes = Err(e.clone());
                            Some(Err(e))
                        },
                    }
                }
            }

            Err(e) => {
                Some(Err(e))
            }
        }
    }
}

impl<'asm> Bytecode<'asm> {
    pub fn globals(&self) -> impl Iterator<Item = Export> {
        (0..self.globals_tab_num)
            .map(move |idx| {
                let offset = 16 + self.globals_tab_off as usize + idx as usize * 8;

                let mut bytes = [0_u8;4];
                bytes.clone_from_slice(&self.bytes[offset..offset+4]);
                let name = u32::from_le_bytes(bytes);
                bytes.clone_from_slice(&self.bytes[offset+4..offset+8]);
                let const_id = u32::from_le_bytes(bytes);

                Export { name, const_id }
            })
    }

    pub fn named_globals(&self) -> impl Iterator<Item = Result<(&'asm str, u32), ByteCodeError>> {
        self.globals().map(|x|
            self.string(x.name).map(|y| (y, x.const_id)))
    }

    pub fn string(&self, off: u32) -> Result<&'asm str, ByteCodeError> {
        let sl = self.data_table().get(off as usize..)
            .ok_or(ByteCodeError::ElementNotFound)?;
        let term = sl.iter().position(|&b| b == 0).ok_or(ByteCodeError::InvalidStringEncoding)?;
        std::str::from_utf8(&sl[0..term]).map_err(|_| ByteCodeError::InvalidStringEncoding)
    }

    pub fn data_table(&self) -> &'asm [u8] {
        &self.bytes[16..16+self.globals_tab_off as usize]
    }

    pub fn const_ops(&self, off: u32) -> Result<OpsIter<'asm>, ByteCodeError> {
        let ops_slice = self.data_table().get((off as usize)..)
            .ok_or(ByteCodeError::ElementNotFound)?;
        Ok(OpsIter::new(ops_slice))
    }

    pub fn main_ops(&self) -> OpsIter<'asm> {
        OpsIter::new(&self.bytes[16 + self.globals_tab_off as usize + self.globals_tab_num as usize * 8..])
    }
}

#[derive(Copy, Clone)]
pub enum ByteCodeError {
    InvalidMagic,
    UnsupportedVersion,
    NotEnoughBytes,
    ElementNotFound,
    InvalidStringEncoding,
    UnknownOpcode,
}

impl Debug for ByteCodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ByteCodeError::InvalidMagic => write!(f, "Invalid magic"),
            ByteCodeError::UnsupportedVersion => write!(f, "Unsupported version"),
            ByteCodeError::NotEnoughBytes => write!(f, "Not enough bytes"),
            ByteCodeError::ElementNotFound => write!(f, "Element not found"),
            ByteCodeError::InvalidStringEncoding => write!(f, "Invalid string encoding"),
            ByteCodeError::UnknownOpcode => write!(f, "Unknown opcode"),
        }
    }
}

impl<'asm> TryFrom<&'asm [u8]> for Bytecode<'asm> {
    type Error = ByteCodeError;

    fn try_from(value: &'asm [u8]) -> Result<Self, Self::Error> {
        fn get_bytes<const L: usize>(from: &[u8], range: Range<usize>) -> Result<[u8;L], ByteCodeError> {
            let slice = from.get(range).ok_or(ByteCodeError::NotEnoughBytes)?;
            let mut zero = [0_u8;L];
            zero.copy_from_slice(slice);
            Ok(zero)
        }

        if !value.get(0..4).ok_or(ByteCodeError::NotEnoughBytes)?
            .iter().zip(['H','6','H','6'].iter()).all(|(a,b)| *a == *b as u8)
        {
            Err(ByteCodeError::InvalidMagic)?;
        }

        let version = *value.get(4).ok_or(ByteCodeError::NotEnoughBytes)?;
        if version != 1 {
            Err(ByteCodeError::UnsupportedVersion)?;
        }

        let globals_tab_num = u16::from_le_bytes(get_bytes(value, 6..8)?);
        let globals_tab_off = u32::from_le_bytes(get_bytes(value, 8..12)?);

        Ok(Self {
            bytes: value,
            globals_tab_num,
            globals_tab_off
        })
    }
}
