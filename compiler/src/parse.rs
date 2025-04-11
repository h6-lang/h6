use std::ops::Range;
use chumsky::container::Container;
use chumsky::error::Cheap;
use chumsky::input::Stream;
use chumsky::Parser;
use smallvec::{smallvec, SmallVec};
use crate::lex::{Tok, TokStr};
use h6_bytecode::{Num, Op};

pub type SomeOps = SmallVec<Op, 8>;

#[derive(Debug, PartialEq, Clone)]
pub struct Expr<'src> {
    pub tok_span: Range<usize>,
    pub binding: Option<TokStr<'src>>,
    pub val: SomeOps
}

struct ArrayCollector(SomeOps);

impl Default for ArrayCollector {
    fn default() -> Self {
        ArrayCollector(smallvec!(Op::ArrBegin))
    }
}

impl Container<SomeOps> for ArrayCollector {
    fn push(&mut self, item: SomeOps) {
        let mut item = item;
        self.0.append(&mut item);
    }
}

impl ArrayCollector {
    fn finish(self) -> SomeOps {
        let mut out = self.0;
        out.push(Op::ArrEnd);
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
            just(Tok::L).to(Op::RoL),
            just(Tok::R).to(Op::RoR),
            just(Tok::Dollar).to(Op::Swap),
            just(Tok::At0).to(Op::ArrFirst),
            just(Tok::AtPlus).to(Op::ArrCat),
            just(Tok::AtStar).to(Op::ArrLen),
            just(Tok::AtLeft).to(Op::ArrSkip1),
            just(Tok::Pack).to(Op::Pack),
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
            .map_with(|str, ctx| Expr {
                tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                binding: None,
                val: smallvec!(Op::Frontend(h6_bytecode::FrontendOp::Unresolved(
                    str.to_string()
                )))
            });

        let num = select! { Tok::Num(num) => num }
            .map_with(|val, ctx| Expr {
                tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                binding: None,
                val: smallvec!(Op::Push { val })
            });

        // TODO: in future version of format: put strings into strtab too
        let str = select! { Tok::Str(str) => str }
            .map_with(|str, ctx| {
                let mut val = smallvec!(Op::ArrBegin);
                val.extend(str.as_bytes().iter()
                    .map(|x| Op::Push { val: Num::from(*x) }));
                val.push(Op::ArrEnd);

                Expr {
                    tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                    binding: None,
                    val
                }
            });

        let char = select! { Tok::Char(c) => c }
            .map_with(|val, ctx| Expr {
                tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                binding: None,
                val: smallvec!(Op::Push { val: (val as i16).into() })
            });

        use fixed::prelude::LossyFrom;
        let syscall = select! { Tok::Ident(i) => i }
            .filter(|x| x == "system")
            .ignore_then(select! { Tok::Num(n) => n })
            .map_with(|val, ctx| Expr {
                tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                binding: None,
                val: smallvec!(Op::System { id: i32::lossy_from(val) as u32 })
            });

        let planet = select! { Tok::RefPlanet(p) => p }
            .map_with(|val, ctx| {
                let mut tooken = 0;
                let mut ops = smallvec!();
                for (i,take) in val.into_iter().enumerate().rev() {
                    if take {
                        ops.push(Op::Reach { down: (tooken + i) as u32 });
                        tooken += 1;
                    }
                }

                Expr {
                    tok_span: SimpleSpan::<usize>::into_range(ctx.span()),
                    binding: None,
                    val: ops
                }
            });

        choice((planet, syscall, bind, op, arr, ident, num, str, char))
            .padded_by(select! { Tok::Comment(_) => () }.repeated())
            .boxed()
    });

    expr.repeated().collect()
}

pub fn parse<'src, I: Iterator<Item = Tok<'src>> + 'src>(input: I) -> Result<Vec<Expr<'src>>, Vec<Cheap>> {
    let (v, err) = parser().parse(Stream::from_iter(input)).into_output_errors();
    v.ok_or_else(|| err)
}
