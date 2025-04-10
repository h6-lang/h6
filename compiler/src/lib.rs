pub mod lex;
pub mod parse;
pub mod lower;

use lex::{Spanned, Tok};

pub struct UnSpannedGetter<'x, 'src> {
    backing: &'x [Spanned<Tok<'src>>]
}

impl<'x, 'src> UnSpannedGetter<'x, 'src> {
    pub fn new(inp: &'x [Spanned<Tok<'src>>]) -> Self {
        UnSpannedGetter { backing: inp }
    }
}

impl<'x, 'src> std::ops::Index<usize> for UnSpannedGetter<'x, 'src> {
    type Output = Tok<'src>;
    
    fn index(&self, index: usize) -> &Self::Output {
        &self.backing[index].0
    }
}

