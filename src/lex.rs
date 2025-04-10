use crate::Num;
use chumsky::error::Cheap;
use chumsky::Parser;
use std::fmt::{Display, Formatter};
use std::ops::Range;

// TODO: char literals syntax: 'H 'i '! '\0 '\n

pub type TokStr<'src> = std::borrow::Cow<'src, str>;

#[derive(Clone, PartialEq)]
pub enum Tok<'src> {
    Comment(&'src str),
    Num(Num),
    Str(TokStr<'src>),
    Ident(TokStr<'src>),
    Colon,
    CurlyOpen,
    CurlyClose,
    Dot,
    Comma,
    Semicolon,
    Exclamation,
    Question,
    AngleOpen,
    AngleClose,
    Equal,
    Tilde, // not
    Plus,
    Minus,
    Mul,
    RefL,
    L,
    RefR,
    R,
    Dollar,
    At0,
    AtStar,
}

#[derive(Clone, Copy)]
pub enum TokType {
    Num,
    Str,
    Ident,
    Point,
    Op,
    Comment,
}

impl<'src> Into<TokStr<'src>> for &Tok<'src> {
    fn into(self) -> TokStr<'src> {
        match self {
            Tok::Comment(str) => (*str).into(),
            Tok::Num(num) => num.to_string().into(),
            Tok::Str(str) => str.clone(),
            Tok::Ident(str) => str.clone(),
            Tok::CurlyOpen => "{".into(),
            Tok::CurlyClose => "}".into(),
            Tok::Colon => ":".into(),
            Tok::Dot => ".".into(),
            Tok::Comma => ",".into(),
            Tok::Semicolon => ";".into(),
            Tok::Exclamation => "!".into(),
            Tok::Question => "?".into(),
            Tok::AngleOpen => "<".into(),
            Tok::AngleClose => ">".into(),
            Tok::Equal => "=".into(),
            Tok::Tilde => "~".into(),
            Tok::Plus => "+".into(),
            Tok::Minus => "-".into(),
            Tok::Mul => "*".into(),
            Tok::RefL => "&l".into(),
            Tok::L => "l".into(),
            Tok::RefR => "&r".into(),
            Tok::R => "r".into(),
            Tok::Dollar => "$".into(),
            Tok::At0 => "@0".into(),
            Tok::AtStar => "@*".into(),
        }
    }
}

impl<'src> Display for Tok<'src> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", <&Tok<'src> as Into<TokStr<'src>>>::into(self))
    }
}

impl<'src> Into<TokType> for &Tok<'src> {
    fn into(self) -> TokType {
        match self {
            Tok::Comment(_) => TokType::Comment,
            Tok::Num(_) => TokType::Num,
            Tok::Str(_) => TokType::Str,
            Tok::Ident(_) => TokType::Ident,

            Tok::CurlyOpen |
            Tok::CurlyClose |
            Tok::Colon => TokType::Point,

            Tok::Dot |
            Tok::Comma |
            Tok::Semicolon |
            Tok::Exclamation |
            Tok::Question |
            Tok::AngleOpen |
            Tok::AngleClose |
            Tok::Equal |
            Tok::Tilde |
            Tok::Plus |
            Tok::Minus |
            Tok::Mul |
            Tok::RefL |
            Tok::L |
            Tok::RefR |
            Tok::R |
            Tok::Dollar |
            Tok::At0 |
            Tok::AtStar
            => TokType::Op,
        }
    }
}

#[cfg(feature = "color")]
#[derive(Clone, Copy)]
pub struct Style(yansi::Style);

#[cfg(all(feature = "color", feature = "serde"))]
impl<'de> serde::Deserialize<'de> for Style {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        #[derive(serde::Deserialize)]
        enum Attrib {
            Bold,
            Dim,
            Italic,
            Underline,
            Blink,
            RapidBlink,
            Invert,
            Conceal,
            Strike,
        }

        impl Into<yansi::Attribute> for Attrib {
            fn into(self) -> yansi::Attribute {
                match self {
                    Attrib::Bold => yansi::Attribute::Bold,
                    Attrib::Dim => yansi::Attribute::Dim,
                    Attrib::Italic => yansi::Attribute::Italic,
                    Attrib::Underline => yansi::Attribute::Underline,
                    Attrib::Blink => yansi::Attribute::Blink,
                    Attrib::RapidBlink => yansi::Attribute::RapidBlink,
                    Attrib::Invert => yansi::Attribute::Invert,
                    Attrib::Conceal => yansi::Attribute::Conceal,
                    Attrib::Strike => yansi::Attribute::Strike,
                }
            }
        }

        struct Color(yansi::Color);

        impl<'de> serde::Deserialize<'de> for Color {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>
            {
                let s = String::deserialize(deserializer)?;
                match s.as_str() {
                    "black" => Ok(Color(yansi::Color::Black)),
                    "red" => Ok(Color(yansi::Color::Red)),
                    "green" => Ok(Color(yansi::Color::Green)),
                    "yellow" => Ok(Color(yansi::Color::Yellow)),
                    "blue" => Ok(Color(yansi::Color::Blue)),
                    "magenta" => Ok(Color(yansi::Color::Magenta)),
                    "cyan" => Ok(Color(yansi::Color::Cyan)),
                    "white" => Ok(Color(yansi::Color::White)),
                    _ => Err(serde::de::Error::custom("invalid color")),
                }
            }
        }

        #[derive(serde::Deserialize)]
        struct Inner {
            fg: Color,
            bg: Option<Color>,
            attrs: Vec<Attrib>,
        }

        impl Into<yansi::Style> for Inner {
            fn into(self) -> yansi::Style {
                let mut style = yansi::Style::new();
                style = style.fg(self.fg.0);
                if let Some(bg) = self.bg {
                    style = style.bg(bg.0);
                }
                for attr in self.attrs {
                    style = style.attr(attr.into());
                }
                style
            }
        }

        let inner = Inner::deserialize(deserializer)?;
        Ok(Style(inner.into()))
    }
}

#[cfg(feature = "color")]
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub struct ColorScheme {
    pub number: Style,
    pub string: Style,
    pub identifier: Style,
    pub point: Style,
    pub op: Style,
    pub comment: Style,
}

#[cfg(feature = "color")]
impl Default for ColorScheme {
    fn default() -> Self {
        ColorScheme {
            number: Style(yansi::Color::Blue.into()),
            string: Style(yansi::Color::Green.into()),
            identifier: Style(yansi::Color::Cyan.into()),
            point: Style(yansi::Color::Yellow.into()),
            op: Style(yansi::Color::Magenta.into()),
            comment: Style(yansi::Color::White.dim())
        }
    }
}

impl<'src> Tok<'src> {
    pub fn highlight(&self) -> String {
        #[cfg(feature = "color")]
        return self.highlight_with(&ColorScheme::default());
        #[cfg(not(feature = "color"))]
        return self.into().into();
    }

    #[cfg(feature = "color")]
    pub fn highlight_with(&self, scheme: &ColorScheme) -> String {
        use yansi::Paint;

        let style = match self.into() {
            TokType::Num => scheme.number,
            TokType::Str => scheme.string,
            TokType::Ident => scheme.identifier,
            TokType::Point => scheme.point,
            TokType::Op => scheme.op,
            TokType::Comment => scheme.comment,
        };

        self.to_string().paint(style.0).to_string()
    }
}

pub type Spanned<T> = (T, Range<usize>);

pub fn lexer<'src>() ->
    impl Parser<'src, &'src str, Vec<Spanned<Tok<'src>>>, chumsky::extra::Err<Cheap>>
{
    use chumsky::prelude::*;

    let num = just("+").to(false)
        .or(just("-").to(true))
        .or_not()
        .map(|x| x.unwrap_or(false))
        .then(text::int(10)
            .then(just('.')
                .then(text::int(10))
                .or_not())
            .to_slice()
            .map(|slice: &str| slice.parse::<Num>().unwrap()))
        .map(|(sign, num)| Tok::Num(if sign { -num } else { num }));

    let str = choice((
        just("\\\\").to('\\'),
        just("\\\"").to('"'),
        just("\\n").to('\n'),
        none_of(['"'])
    ))
        .repeated()
        .collect::<String>()
        .delimited_by(just('"'), just('"'))
        .map(|x| Tok::Str(x.into()));

    let comment = just("#")
        .then(any().and_is(text::newline().not())
            .repeated())
        .to_slice()
        .map(|span: &str| Tok::Comment(span));

    let op: Boxed<_, Tok, extra::Err<Cheap>> = choice((
        just(":").to(Tok::Colon),
        just(".").to(Tok::Dot),
        just(",").to(Tok::Comma),
        just(";").to(Tok::Semicolon),
        just("!").to(Tok::Exclamation),
        just("?").to(Tok::Question),
        just("<").to(Tok::AngleOpen),
        just(">").to(Tok::AngleClose),
        just("=").to(Tok::Equal),
        just("~").to(Tok::Tilde),
        just("+").to(Tok::Plus),
        just("-").to(Tok::Minus),
        just("*").to(Tok::Mul),
        just("&l").to(Tok::RefL),
        text::keyword("l").to(Tok::L),
        just("&r").to(Tok::RefR),
        text::keyword("r").to(Tok::R),
        just("{").to(Tok::CurlyOpen),
        just("}").to(Tok::CurlyClose),
        just("$").to(Tok::Dollar),
        text::keyword("@0").to(Tok::At0),
        text::keyword("@*").to(Tok::AtStar),
    )).boxed();

    let tok: Boxed<_, Tok, extra::Err<Cheap>> = choice([
        num.boxed(),
        str.boxed(),
        comment.boxed(),
        text::ident().map(|x: &str| Tok::Ident(x.into())).boxed(),
        op.boxed()
    ]).boxed();

    tok.map_with(|t: Tok, e| (t, (e.span() as SimpleSpan).into_range()))
        .padded()
        .repeated()
        .collect()
        .then_ignore(end())
        .boxed()
}

pub fn lex(input: &str) -> Result<Vec<Spanned<Tok>>, Vec<Cheap>> {
    let (v, err) = lexer().parse(input).into_output_errors();
    v.ok_or_else(|| err)
}