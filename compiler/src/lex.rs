use chumsky::error::Cheap;
use chumsky::Parser;
use std::fmt::{Display, Formatter};
use std::ops::Range;
use h6_bytecode::Num;

// TODO: char literals syntax: 'H 'i '! '\0 '\n

pub type TokStr<'src> = std::borrow::Cow<'src, str>;

#[derive(Clone, PartialEq)]
pub enum Tok<'src> {
    Comment(&'src str),
    Num(Num),
    Str(TokStr<'src>),
    Ident(TokStr<'src>),
    Char(char),
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
    SquareOpen,
    SquareClose,
    Equal,
    Tilde, // not
    Plus,
    Minus,
    Mul,
    L,
    R,
    Dollar,
    At0,
    AtStar,
    AtPlus,
    AtLeft,
    Pack,
    Error,
    RefPlanet(Vec<bool>),
    TypeID,
    System,
    Fract,
    Mod,
    Div,
}

#[derive(Clone, Copy)]
pub enum TokType {
    Num,
    Str,
    Ident,
    Point,
    Op,
    Comment,
    Err,
}

impl<'src> Into<TokStr<'src>> for &Tok<'src> {
    fn into(self) -> TokStr<'src> {
        match self {
            Tok::Comment(str) => (*str).into(),
            Tok::Num(num) => num.to_string().into(),
            Tok::Str(str) => str.clone(),
            Tok::Ident(str) => str.clone(),
            Tok::Char(c) => c.to_string().into(),
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
            Tok::Mod => "%".into(),
            Tok::Div => "/".into(),
            Tok::L => "l".into(),
            Tok::R => "r".into(),
            Tok::Dollar => "$".into(),
            Tok::At0 => "@0".into(),
            Tok::AtPlus => "@+".into(),
            Tok::AtStar => "@*".into(),
            Tok::AtLeft => "@<".into(),
            Tok::Pack => "_".into(),
            Tok::Error => "<ERR>".into(),
            Tok::RefPlanet(_) => "<planet>".into(),
            Tok::TypeID => "<typeid>".into(),
            Tok::System => "<system>".into(),
            Tok::Fract => "<fract>".into(),
            Tok::SquareOpen => "[".into(),
            Tok::SquareClose => "]".into(),
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
            Tok::Str(_)  |
            Tok::Char(_) => TokType::Str,
            Tok::Ident(_) => TokType::Ident,

            Tok::CurlyOpen   |
            Tok::CurlyClose  |
            Tok::SquareOpen  |
            Tok::SquareClose |
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
            Tok::L |
            Tok::R |
            Tok::Dollar |
            Tok::At0 |
            Tok::AtPlus |
            Tok::AtStar |
            Tok::AtLeft |
            Tok::Pack |
            Tok::RefPlanet(_) |
            Tok::TypeID |
            Tok::System |
            Tok::Mod |
            Tok::Div |
            Tok::Fract
            => TokType::Op,

            Tok::Error => TokType::Err,
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
    pub err: Style,
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
            comment: Style(yansi::Color::White.dim()),
            err: Style(yansi::Color::Red.into())
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
            TokType::Err => scheme.err,
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
    
    let escape = choice((
        just("\\\\").to('\\'),
        just("\\\"").to('"'),
        just("\\n").to('\n'),
    ));

    let str = choice((
        escape.clone(),
        none_of(['"'])
    ))
        .repeated()
        .collect::<String>()
        .delimited_by(just('"'), just('"'))
        .map(|x| Tok::Str(x.into()));

    let char = just('\'')
        .ignore_then(escape.clone().or(any()))
        .map(|x| Tok::Char(x));

    let comment = just("#")
        .then(any().and_is(text::newline().not())
            .repeated())
        .to_slice()
        .map(|span: &str| Tok::Comment(span));

    let planet_inner = choice((
            just("-").to(false),
            just("v").to(true),
    )).repeated().collect::<Vec<bool>>();

    let ref_planet = just("&")
        .ignore_then(planet_inner)
        .map(|v| Tok::RefPlanet(v));

    let op: Boxed<_, Tok, extra::Err<Cheap>> = choice([
        text::keyword("fract").to(Tok::Fract),
        text::keyword("system").to(Tok::System),
        text::keyword("typeid").to(Tok::TypeID),
        text::keyword("_").to(Tok::Pack),
        text::keyword("l").to(Tok::L),
        text::keyword("r").to(Tok::R),
    ]).or(choice([
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
        just("%").to(Tok::Mod),
        just("/").to(Tok::Div),
        just("{").to(Tok::CurlyOpen),
        just("}").to(Tok::CurlyClose),
        just("[").to(Tok::SquareOpen),
        just("]").to(Tok::SquareClose),
        just("$").to(Tok::Dollar),
        just("@0").to(Tok::At0),
        just("@+").to(Tok::AtPlus),
        just("@*").to(Tok::AtStar),
        just("@<").to(Tok::AtLeft),
    ])).boxed();

    let tok: Boxed<_, Tok, extra::Err<Cheap>> = choice([
        ref_planet.boxed(),
        num.boxed(),
        str.boxed(),
        comment.boxed(),
        op.boxed(),
        text::ident().map(|x: &str| Tok::Ident(x.into())).boxed(),
        char.boxed(),
    ]).boxed();

    tok.recover_with(via_parser(any::<_, extra::Err<Cheap>>().to(Tok::Error)))
        .map_with(|t: Tok, e| (t, (e.span() as SimpleSpan).into_range()))
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
