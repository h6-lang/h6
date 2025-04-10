use crate::bytecode::*;
use crate::parse::Expr;

/// writes a bytecode assembly into the sink
pub fn lower<'src, W: std::io::Write>(sink: &mut W, exprs: Vec<Expr<'src>>) {
    // we keep track of all previously defined bindings.
    // this will keep references to later-defined bindings as Unresolved, which the runtime's linker will resolve
    
    
}