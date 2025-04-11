pub mod linker;
pub mod disasm;

use std::fmt::{Debug, Formatter};
use std::io::Write;
use std::ops::Range;
use int_enum::IntEnum;
use std::collections::HashSet;

pub type Num = fixed::types::I16F16;

#[derive(Debug, Clone, PartialEq)]
pub enum FrontendOp {
    Unresolved(String)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    /// used to mark end of constant or code
    Terminate,

    /// offset into table
    Unresolved { id: u32 },

    /// only understood by the frontend!
    Frontend(FrontendOp),

    /// offset into table
    Const { idx: u32 },

    Push { val: Num },

    System { id: u32 },

    Add,
    Sub,
    Mul,

    /// identical to reach(0), but saves bytes
    Dup,
    Swap,
    Pop,
    Exec,

    /// if stack[0] > 0 then stack[1] else stack[2]
    Select,


    Lt,
    Gt,
    Eq,
    Not,

    /// rotate top 3 stack values left/down
    RoL,

    /// rotate top 3 stack values right/up
    RoR,

    /// copy the [down]-th stack value to the top
    Reach { down: u32 },

    /// all bytecode ops until the corresponding ArrEnd will be collected into an array
    ArrBegin,
    ArrEnd,

    /// (stack[-1] as arr) concat (stack[0] as arr)
    ArrCat,

    /// (stack[0] as arr)[0]
    ArrFirst,

    /// (stack[0] as arr)[1..]
    ArrSkip1,

    /// (stack[0] as arr).len
    ArrLen,

    /// this works on both numbers and arrays
    Pack,

    /// if the instruction sequence in the bytecode ends here, a "terminate" op is required after this
    Jump { idx: u32 },

    /// 0 is number (fixed16f16), 1 is array
    TypeId,
}

impl Op {
    pub fn offset(self, by: usize) -> Op {
        match self {
            Op::Unresolved { id } => Op::Unresolved { id: id + by as u32 },
            Op::Const { idx } => Op::Const { idx: idx + by as u32 },
            Op::Jump { idx } => Op::Jump { idx: idx + by as u32 },
            _ => self,
        }
    }
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
            Op::Swap => OpType::Swap,
            Op::Pop => OpType::Pop,
            Op::Exec => OpType::Exec,
            Op::Select => OpType::Select,
            Op::Lt => OpType::Lt,
            Op::Gt => OpType::Gt,
            Op::Eq => OpType::Eq,
            Op::Not => OpType::Not,
            Op::RoL => OpType::RoL,
            Op::RoR => OpType::RoR,
            Op::ArrBegin => OpType::ArrBegin,
            Op::ArrEnd => OpType::ArrEnd,
            Op::ArrCat => OpType::ArrCat,
            Op::ArrFirst => OpType::ArrFirst,
            Op::ArrSkip1 => OpType::ArrSkip1,
            Op::ArrLen => OpType::ArrLen,
            Op::Jump { .. } => OpType::Jump,
            Op::Reach { .. } => OpType::Reach,
            Op::System { .. } => OpType::System,
            Op::Pack => OpType::Pack,
            Op::Frontend(_) => panic!(),
            Op::TypeId => OpType::TypeId,
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
            Op::Reach { down } => to.write_all(&down.to_le_bytes())?,
            Op::System { id } => to.write_all(&id.to_le_bytes())?,
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
    TypeId = 3,
    Push = 8,

    Add = 9,
    Sub = 10,
    Mul = 11,
    Dup = 12,
    Swap = 14,
    Pop = 15,
    Exec = 16,
    Select = 17,
    Lt = 18,
    Gt = 19,
    Eq = 20,
    Not = 21,
    RoL = 22,
    RoR = 24,
    Reach = 25,

    ArrBegin = 26,
    ArrEnd = 27,
    ArrCat = 29,
    ArrFirst = 30,
    ArrLen = 31,
    ArrSkip1 = 32,
    Pack = 33,

    Jump = 40,
    System = 41,
}

impl OpType {
    pub fn has_param(&self) -> bool {
        match self {
            OpType::Unresolved => true,
            OpType::Const => true,
            OpType::Push => true,
            OpType::Reach => true,
            OpType::System => true,
            _ => false,
        }
    }

    /// input slice can be longer than required
    /// returns weather or not had param
    pub fn read(bytes: &[u8]) -> Result<(bool, Op), ByteCodeError> {
        let opty = OpType::try_from(*bytes.get(0).ok_or(ByteCodeError::NotEnoughBytes)?)
            .map_err(|_| ByteCodeError::UnknownOpcode(bytes[0]))?;

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
            OpType::Swap => Op::Swap,
            OpType::Pop => Op::Pop,
            OpType::Exec => Op::Exec,
            OpType::Select => Op::Select,
            OpType::Lt => Op::Lt,
            OpType::Gt => Op::Gt,
            OpType::Eq => Op::Eq,
            OpType::Not => Op::Not,
            OpType::RoL => Op::RoL,
            OpType::RoR => Op::RoR,
            OpType::ArrBegin => Op::ArrBegin,
            OpType::ArrEnd => Op::ArrEnd,
            OpType::ArrCat => Op::ArrCat,
            OpType::ArrFirst => Op::ArrFirst,
            OpType::ArrLen => Op::ArrLen,
            OpType::ArrSkip1 => Op::ArrSkip1,
            OpType::Pack => Op::Pack,
            OpType::Reach => Op::Reach { down: u32::from_le_bytes(arg.ok_or(ByteCodeError::NotEnoughBytes)?) },
            OpType::Jump => Op::Jump { idx: u32::from_le_bytes(arg.ok_or(ByteCodeError::NotEnoughBytes)?) },
            OpType::System => Op::System { id: u32::from_le_bytes(arg.ok_or(ByteCodeError::NotEnoughBytes)?) },
            OpType::TypeId => Op::TypeId,
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

impl Export {
    pub fn write<W: Write>(&self, out: &mut W) -> std::io::Result<()> {
        let mut bytes = [0_u8;8];
        bytes[0..4].copy_from_slice(self.name.to_le_bytes().as_slice());
        bytes[4..8].copy_from_slice(self.const_id.to_le_bytes().as_slice());
        out.write_all(bytes.as_slice())?;
        Ok(())
    }
}

pub const VERSION: u8 = 1;

/// header (16 bytes)
///   magic:   4 * u8 = "H6H6"
///   min_reader_version: u8     = 1
///   writer_version: u8 = 0
///   globals table num entries: u16_le
///   offset to globals table in code tab: u32_le
///   reserved: u32 = 0
///
#[derive(Clone, Debug)]
pub struct Header {
    pub min_reader_version: u8,
    pub writer_version: u8,

    pub globals_tab_num: u16,

    /// relative to code/string/const table!!
    pub globals_tab_off: u32,
}

impl Header {
    pub fn main_ops_area_begin_idx(&self) -> usize {
        16 + self.globals_tab_off as usize + self.globals_tab_num as usize * 8
    }

    pub fn serialize(&self) -> [u8;16] {
        let mut header = [
            'H' as u8, '6' as u8, 'H' as u8, '6' as u8,
            self.min_reader_version,
            self.writer_version,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ];

        header[6..8].copy_from_slice(&self.globals_tab_num.to_le_bytes());
        header[8..12].copy_from_slice(&self.globals_tab_off.to_le_bytes());

        header
    }

    pub fn write<W: std::io::Write>(&self, to: &mut W) -> std::io::Result<()> {
        to.write_all(self.serialize().as_slice())
    }
}

impl Default for Header {
    fn default() -> Self {
        Header {
            min_reader_version: VERSION,
            writer_version: VERSION,
            globals_tab_off: 0,
            globals_tab_num: 0,
        }
    }
}

impl<'asm> TryFrom<&'asm [u8]> for Header {
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

        let min_reader_version = *value.get(4).ok_or(ByteCodeError::NotEnoughBytes)?;
        let writer_version = *value.get(5).ok_or(ByteCodeError::NotEnoughBytes)?;
        if VERSION < min_reader_version {
            Err(ByteCodeError::UnsupportedVersion)?;
        }

        let globals_tab_num = u16::from_le_bytes(get_bytes(value, 6..8)?);
        let globals_tab_off = u32::from_le_bytes(get_bytes(value, 8..12)?);

        Ok(Self {
            min_reader_version,
            writer_version,
            globals_tab_num,
            globals_tab_off
        })
    }
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
    pub bytes: &'asm [u8],
    pub header: Header,
}

/// until terminate
pub struct OpsIter<'asm> {
    pub base: usize,
    bytes: Result<Option<&'asm [u8]>, ByteCodeError>
}

impl<'asm> OpsIter<'asm> {
    pub fn new(base: usize, bytes: &'asm [u8]) -> Self {
        Self {
            base,
            bytes: Ok(Some(bytes)),
        }
    }
}

impl<'asm> Iterator for OpsIter<'asm> {
    type Item = Result<(usize, Op), ByteCodeError>;

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
                                let p = self.base;
                                if had_param {
                                    self.bytes = Ok(Some(&bytes[5..]));
                                    self.base += 5;
                                } else {
                                    self.bytes = Ok(Some(&bytes[1..]));
                                    self.base += 1;
                                }
                                Some(Ok((p, op)))
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
    /// this is free
    pub fn from_header(bytes: &'asm [u8], header: Header) -> Self {
        Self { bytes, header }
    }

    pub fn globals(&self) -> impl Iterator<Item = Export> {
        (0..self.header.globals_tab_num)
            .map(move |idx| {
                let offset = idx as usize * 8;

                let mut bytes = [0_u8;4];
                bytes.clone_from_slice(&self.globals_table()[offset..offset+4]);
                let name = u32::from_le_bytes(bytes);
                bytes.clone_from_slice(&self.globals_table()[offset+4..offset+8]);
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

    pub fn const_ops(&self, off: u32) -> Result<OpsIter<'asm>, ByteCodeError> {
        let ops_slice = self.data_table().get((off as usize)..)
            .ok_or(ByteCodeError::ElementNotFound)?;
        Ok(OpsIter::new(16 + off as usize, ops_slice))
    }

    pub fn main_ops(&self) -> OpsIter<'asm> {
        OpsIter::new(self.header.main_ops_area_begin_idx(), self.main_ops_area())
    }

    pub fn data_table(&self) -> &'asm [u8] {
        &self.bytes[16..16+self.header.globals_tab_off as usize]
    }

    pub fn globals_table(&self) -> &'asm [u8] {
        &self.bytes[16+self.header.globals_tab_off as usize..]
    }

    pub fn main_ops_area(&self) -> &'asm [u8] {
        &self.bytes[self.header.main_ops_area_begin_idx()..]
    }

    /// output locations are relative to [self.data_table()]
    pub fn codes_in_data_table(&self) -> Result<HashSet<usize>, ByteCodeError> {
        fn rec<'a, I: Iterator<Item=Result<(usize, Op), ByteCodeError>>>(bc: &Bytecode, out: &mut HashSet<usize>, iter: I) -> Result<(),ByteCodeError> {
            for op in iter {
                let op = op?.1;
                match op {
                    Op::Const { idx } |
                    Op::Jump { idx }  => {
                        if !out.contains(&(idx as usize)) {
                            out.insert(idx as usize);
                            rec(bc, out, bc.const_ops(idx)?)?;
                        }
                    }

                    _ => ()
                }
            }
            Ok(())
        }

        let mut out = HashSet::new();
        rec(self, &mut out, self.main_ops())?;
        for global in self.globals() {
            out.insert(global.const_id as usize);
            rec(self, &mut out, self.const_ops(global.const_id)?)?;
        }
        Ok(out)
    }
}

#[derive(Copy, Clone)]
pub enum ByteCodeError {
    InvalidMagic,
    UnsupportedVersion,
    NotEnoughBytes,
    ElementNotFound,
    InvalidStringEncoding,
    ArrEndMismatch,
    UnknownOpcode(u8),
}

impl Debug for ByteCodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ByteCodeError::InvalidMagic => write!(f, "Invalid magic"),
            ByteCodeError::UnsupportedVersion => write!(f, "Unsupported version"),
            ByteCodeError::NotEnoughBytes => write!(f, "Not enough bytes"),
            ByteCodeError::ElementNotFound => write!(f, "Element not found"),
            ByteCodeError::InvalidStringEncoding => write!(f, "Invalid string encoding"),
            ByteCodeError::UnknownOpcode(val) => write!(f, "Unknown opcode {:#x}", val),
            ByteCodeError::ArrEndMismatch => write!(f, "Different amount of ArrBegin compared to ArrEnd"),
        }
    }
}

impl<'asm> TryFrom<&'asm [u8]> for Bytecode<'asm> {
    type Error = ByteCodeError;

    /// if your need to call this multiple times but with the same header, use [Bytecode::from_header] instead
    fn try_from(value: &'asm [u8]) -> Result<Self, Self::Error> {
        Ok(Bytecode {
            bytes: value,
            header: Header::try_from(value)?
        })
    }
}
