use std::ops::Range;
use chumsky::container::Container;
use chumsky::error::Cheap;
use chumsky::input::Stream;
use chumsky::Parser;
use smallvec::{smallvec, SmallVec};
use crate::bytecode::Op;
use crate::lex::{Tok, TokStr};

pub type SomeOps = SmallVec<Op, 8>;

#[derive(Debug, PartialEq, Clone)]
pub struct Expr<'src> {
    tok_span: Range<usize>,
    binding: Option<TokStr<'src>>,
    val: SomeOps
}

struct ArrayCollector(SomeOps, usize);

impl Default for ArrayCollector {
    fn default() -> Self {
        ArrayCollector(smallvec!(), 0)
    }
}

impl Container<SomeOps> for ArrayCollector {
    fn push(&mut self, item: SomeOps) {
        let mut item = item;
        self.0.append(&mut item);
        self.1 += 1;
    }
}

impl ArrayCollector {
    fn finish(self) -> SomeOps {
        let mut out = self.0;
        out.push(Op::ArrMk { len: self.1 as u32 });
        out
    }
}

pub fn parser<'src, I: Iterator<Item = Tok<'src>> + 'src>() ->
    impl Parser<'src, Stream<I>, Vec<Expr<'src>>, chumsky::extra::Err<Cheap>>
{
    use chumsky::prelude::*;

    let expr = recursive(|expr| {
        let bind = select! { Tok::Ident(str) => str }
            .then_ignore(just(Tok::Colon))
            .then(expr.clone())
            .map_with(|(name, expr): (TokStr, Expr), ctx| Expr {
                tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                binding: Some(name),
                val: expr.val
            });

        let op = choice([
            just(Tok::Dot).to(Op::Dup),
            just(Tok::Comma).to(Op::Over),
            just(Tok::Semicolon).to(Op::Pop),
            just(Tok::Exclamation).to(Op::Exec),
            just(Tok::Question).to(Op::Select),
            just(Tok::AngleOpen).to(Op::Lt),
            just(Tok::AngleClose).to(Op::Gt),
            just(Tok::Equal).to(Op::Eq),
            just(Tok::Tilde).to(Op::Not),
            just(Tok::Plus).to(Op::Add),
            just(Tok::Minus).to(Op::Sub),
            just(Tok::Mul).to(Op::Mul),
            just(Tok::RefL).to(Op::RoLRef),
            just(Tok::L).to(Op::RoL),
            just(Tok::RefR).to(Op::RoRRef),
            just(Tok::R).to(Op::RoR),
            just(Tok::Dollar).to(Op::Swap),
            just(Tok::At0).to(Op::ArrFirst),
            just(Tok::AtStar).to(Op::ArrLen),
        ]).map_with(|op, ctx| Expr {
            tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
            binding: None,
            val: smallvec!(op)
        });

        let arr = just(Tok::CurlyOpen)
            .ignore_then(expr.clone()
                .map(|x| x.val)
                .repeated()
                .collect::<ArrayCollector>()
                .map(|x| x.finish()))
            .then_ignore(just(Tok::CurlyClose))
            .map_with(|ops, ctx| Expr {
                tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                binding: None,
                val: ops
            });

        let ident = select! { Tok::Ident(str) => str }
            .map_with(|op, ctx| Expr {
                tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                binding: None,
                val: smallvec!(Op::Unresolved {
                    id: SimpleSpan::<usize>::from(ctx.span()).start as u32
                })
            });

        let num = select! { Tok::Num(num) => num }
            .map_with(|val, ctx| Expr {
                tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                binding: None,
                val: smallvec!(Op::Push { val })
            });

        choice((bind, op, arr, ident, num))
            .padded_by(select! { Tok::Comment(_) => () }.repeated())
            .boxed()
    });

    expr.repeated().collect()
}

pub fn parse<'src, I: Iterator<Item = Tok<'src>> + 'src>(input: I) -> Result<Vec<Expr<'src>>, Vec<Cheap>> {
    let (v, err) = parser().parse(Stream::from_iter(input)).into_output_errors();
    v.ok_or_else(|| err)
}