
use nostd::prelude::*;
use nostd::{
    collections::HashMap,
    io::{self, Seek, Write, Read, SeekFrom}
};
use crate::*;

#[cfg(feature = "smallvec")]
type SmallVec<T, const N: usize> = smallvec::SmallVec<T,N>;
#[cfg(feature = "smallvec")]
fn empty_smallvec<T, const N: usize>() -> SmallVec<T, N> {
    smallvec::smallvec!()
}

#[cfg(not(feature = "smallvec"))]
type SmallVec<T, const N: usize> = Vec<T>;
#[cfg(not(feature = "smallvec"))]
fn empty_smallvec<T, const N: usize>() -> SmallVec<T, N> {
    vec!()
}

#[derive(Debug)]
pub enum LinkError {
    Io(io::Error),
    ByteCode(ByteCodeError),
    VersionMismatch,
    SymbolDefinedTwice(String),
    SymbolNotFound(String),
}

impl From<io::Error> for LinkError {
    fn from(e: io::Error) -> Self {
        LinkError::Io(e)
    }
}

impl From<ByteCodeError> for LinkError {
    fn from(e: ByteCodeError) -> Self {
        LinkError::ByteCode(e)
    }
}

/// after concatenating all files into one, run self_link
pub fn cat_together<W: Write + Seek + Read>(output: &mut W, input: &[u8]) -> Result<(), LinkError> {
    let input = Bytecode::try_from(input)?;

    let mut out_header = [0_u8; 16];
    output.seek(SeekFrom::Start(0))?;
    output.read_exact(&mut out_header)?;
    let out_header = Header::try_from(out_header.as_slice())?;
    if out_header.writer_version != input.header.writer_version {
        return Err(LinkError::VersionMismatch)
    }

    // need to add this to every idx in second
    let second_offset = out_header.globals_tab_off;

    output.seek(SeekFrom::Start(16 + out_header.globals_tab_off as u64))?;
    let mut out_rem = vec!();
    output.read_to_end(&mut out_rem)?;

    let mut new_data_tab = input.data_table().to_vec();
    for code in input.codes_in_data_table()? {
        for op in input.const_ops(code as u32)? {
            let (pos, op) = op?;
            let op = op.offset(second_offset as usize);
            let pos = pos - 16;

            let mut new_bytes = vec!();
            op.write(&mut new_bytes)?;
            new_data_tab[pos..pos + new_bytes.len()].copy_from_slice(new_bytes.as_slice());
        }
    }
    output.seek(SeekFrom::Start(16 + out_header.globals_tab_off as u64))?;
    output.write_all(new_data_tab.as_slice())?;

    let new_globals_begin = output.seek(SeekFrom::Current(0))?;
    let new_globals_len = input.header.globals_tab_num + out_header.globals_tab_num;
    output.write_all(&out_rem[..out_header.globals_tab_num as usize * 8])?;
    for kv in input.globals() {
        let kv = Export {
            name: kv.name + second_offset,
            const_id: kv.const_id + second_offset
        };
        kv.write(output)?;
    }

    for op in OpsIter::new(0, &out_rem[out_header.globals_tab_num as usize * 8..]) {
        let (_, op) = op?;
        op.write(output)?;
    }
    for op in input.main_ops() {
        let op = op?.1.offset(second_offset as usize);
        op.write(output)?;
    }
    Op::Terminate.write(output)?;

    output.seek(SeekFrom::Start(0))?;
    Header {
        globals_tab_num: new_globals_len,
        globals_tab_off: new_globals_begin as u32 - 16,
        ..out_header
    }.write(output)?;
    Ok(())
}

pub trait Target {
    fn allow_undeclared_symbol(&self, sym: &str) -> bool;
}

pub fn self_link<T: Target>(bin: &mut [u8], target: &T) -> Result<(), LinkError> {
    let header = Header::try_from(bin.as_ref())?;

    let mut decls = HashMap::new();
    for decl in Bytecode::from_header(bin, header.clone()).named_globals() {
        let (name,val) = decl?;
        if decls.contains_key(name) {
            Err(LinkError::SymbolDefinedTwice(name.to_string()))?;
        }
        decls.insert(unsafe{ &*(name as *const str) }, val);
    }

    let mut done = Vec::<usize>::new();

    let mut todo = vec!(
        Bytecode::from_header(bin, header.clone()).main_ops_area().as_ptr().addr() - bin.as_ptr().addr() - 16);
    todo.extend(decls.iter().map(|x| (*x.1) as usize));

    while let Some(off) = todo.pop() {
        let mut to_write = empty_smallvec::<(usize,u32), 16>();
        for op in OpsIter::new(off, &bin[16+off..]) {
            let (pos, op) = op?;
            done.push(pos);
            match op {
                Op::Unresolved { id } => {
                    let str = Bytecode::from_header(bin, header.clone()).string(id)?;
                    match decls.get(str) {
                        Some(decl_pos) => {
                            to_write.push((pos, *decl_pos));
                        }

                        None => {
                            if !target.allow_undeclared_symbol(str) {
                                Err(LinkError::SymbolNotFound(str.to_string()))?;
                            }
                        }
                    }
                }

                Op::Const { idx } => {
                    if !done.contains(&(idx as usize)) {
                        todo.push(idx as usize);
                    }
                }

                _ => ()
            }
        }

        for (pos,val) in to_write {
            let mut v = vec!();
            Op::Const { idx: val }.write(&mut v)?;
            bin[16+pos..16+pos+v.len()].copy_from_slice(v.as_slice());
        }
    }

    Ok(())
}

// TODO: add self_gc() to opt binsize
