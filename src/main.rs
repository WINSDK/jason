#![allow(dead_code)]

use std::fmt;

mod tests;

const MAX_DEPTH: usize = 256;

struct Error {
    offset: usize,
    msg: String,
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.msg)?;
        f.write_str(" at position ")?;
        self.offset.fmt(f)
    }
}

trait Parsing<'src>: Sized {
    fn parse(ctx: &mut Context<'src>) -> Result<Self, Error>;
}

trait Failing: Sized {
    fn failing(self, ctx: &mut Context) -> Self;
}

impl Failing for Error {
    fn failing(self, ctx: &mut Context) -> Self {
        ctx.is_failing = true;
        self
    }
}

impl<T, E> Failing for Result<T, E> {
    fn failing(self, ctx: &mut Context) -> Self {
        ctx.is_failing = self.is_err();
        self
    }
}

impl<T> Failing for Option<T> {
    fn failing(self, ctx: &mut Context) -> Self {
        ctx.is_failing = self.is_none();
        self
    }
}

#[derive(Debug)]
struct Context<'src> {
    src: &'src str,
    offset: usize,
    depth: usize,
    is_failing: bool,
}

impl<'src> Context<'src> {
    fn new(src: &'src str) -> Self {
        Self {
            src,
            offset: 0,
            depth: 0,
            is_failing: false,
        }
    }

    fn src(&self) -> &'src str {
        &self.src[self.offset..]
    }

    fn descent(&mut self) -> Result<(), Error> {
        self.depth += 1;

        if self.depth == MAX_DEPTH {
            self.failing("reached recursion depth")
        } else {
            Ok(())
        }
    }

    fn ascent(&mut self) {
        self.depth -= 1;
    }

    fn error<T>(&self, msg: &str) -> Result<T, Error> {
        Err(Error {
            offset: self.offset,
            msg: msg.to_string(),
        })
    }

    fn failing<T>(&mut self, msg: &str) -> Result<T, Error> {
        self.is_failing = true;
        self.error(msg)
    }

    fn peek(&mut self) -> Option<char> {
        self.src().chars().next()
    }

    fn consume(&mut self, chr: char) -> Result<(), Error> {
        match self.src().chars().next() {
            Some(got) if got == chr => {
                self.offset += 1;
                Ok(())
            }
            Some(got) => self.error(&format!("expected '{chr}' got '{got}'")),
            None => self.error(&format!("expected '{chr}' got EOF")),
        }
    }

    fn consume_slice(&mut self, s: &str) -> Result<(), Error> {
        match self.src().get(..s.len()) {
            Some(got) if got == s => {
                self.offset += s.len();
                Ok(())
            }
            Some(got) => self.error(&format!("expected '{s}' got '{got}'")),
            None => self.error(&format!("expected '{s}' got EOF")),
        }
    }

    fn consume_whitespace(&mut self) {
        // all forms of accepted whitespace
        while let Some('\u{0020}' | '\u{000a}' | '\u{000d}' | '\u{0009}') = self.peek() {
            self.offset += 1;
        } 
    }

    fn consume_str(&mut self) -> Result<&'src str, Error> {
        self.consume('"')?;
        let start = self.offset;

        loop {
            if let Some('"') = self.peek() {
                break;
            }

            match self.src().chars().next() {
                Some(..) => self.offset += 1,
                None => return self.error("reached EOF on string"),
            }
        }

        self.offset += 1;
        Ok(&self.src[start..self.offset])
    }

    fn consume_int(&mut self) -> Result<isize, Error> {
        let is_neg = self.consume('-').is_ok();
        let mut int = 0isize;

        match self.peek().map(|chr| chr as u8) {
            Some(digit @ b'0'..=b'9') => {
                int = match int.checked_add((digit - b'0') as isize) {
                    Some(int) => int,
                    None => return self.failing("integer too large")
                };

                self.offset += 1;
            }
            _ => return self.error("integer didn't contain any digits"),
        }

        while let Some(digit @ b'0'..=b'9') = self.peek().map(|chr| chr as u8) {
            int = match int.checked_mul(10) {
                Some(int) => int,
                None => return self.failing("integer too large")
            };

            int = match int.checked_add((digit - b'0') as isize) {
                Some(int) => int,
                None => return self.failing("integer too large")
            };

            self.offset += 1;
        }

        if is_neg {
            int = match int.checked_mul(-1) {
                Some(int) => int,
                None => return self.failing("integer too large")
            };
        }

        Ok(int)
    }
}

#[derive(Debug)]
enum Value<'src> {
    Object(Object<'src>),
    Array(Array<'src>),
    String(&'src str),
    Number(Number),
    True,
    False,
    Null,
}

impl<'src> Value<'src> {
    fn parse_inner(ctx: &mut Context<'src>) -> Result<Self, Error> {
        match Object::parse(ctx) {
            Ok(object) => return Ok(Value::Object(object)),
            Err(err) if ctx.is_failing => return Err(err),
            Err(..) => {}
        }

        match Number::parse(ctx) {
            Ok(number) => return Ok(Value::Number(number)),
            Err(err) if ctx.is_failing => return Err(err),
            Err(..) => {}
        }

        match Array::parse(ctx) {
            Ok(array) => return Ok(Value::Array(array)),
            Err(err) if ctx.is_failing => return Err(err),
            Err(..) => {}
        }

        if let Ok(str) = ctx.consume_str() {
            return Ok(Value::String(str));
        }

        if let Ok(()) = ctx.consume_slice("true") {
            return Ok(Value::True);
        }

        if let Ok(()) = ctx.consume_slice("false") {
            return Ok(Value::False);
        }

        if let Ok(()) = ctx.consume_slice("null") {
            return Ok(Value::Null);
        }

        ctx.failing("unknown value kind")
    }
}

impl<'src> Parsing<'src> for Value<'src> {
    fn parse(ctx: &mut Context<'src>) -> Result<Self, Error> {
        ctx.consume_whitespace();
        let this = Self::parse_inner(ctx);
        ctx.consume_whitespace();
        this
    }
}

#[derive(Debug)]
enum Number {
    Int(isize),
    Frac { int: isize, frac: isize },
    Exp { int: isize, exp: isize },
    FracExp { int: isize, frac: isize, exp: isize },
}

impl Parsing<'_> for Number {
    fn parse(ctx: &mut Context) -> Result<Self, Error> {
        let int = ctx.consume_int()?;
        let mut opt_frac = None;

        // fractional
        if let Some('.') = ctx.peek() {
            ctx.offset += 1;
            let frac = ctx.consume_int().failing(ctx)?;

            if frac.is_negative() {
                return ctx.failing("can't have a negative fraction");
            }

            opt_frac = Some(frac);
        }

        // scientific notation
        if let Some('e' | 'E') = ctx.peek() {
            ctx.offset += 1;
            let is_neg = match ctx.peek() {
                Some('-') => true,
                Some(..) => false,
                None => return ctx.failing("missing sign in E-notation"),
            };
            ctx.offset += 1;

            let mut exp = ctx.consume_int()?;
            if exp.is_negative() {
                return ctx.failing("unknown sign in E-notation");
            }

            if is_neg {
                exp = match exp.checked_mul(-1) {
                    Some(exp) => exp,
                    None => return ctx.failing("exponent too large")
                };
            }

            if let Some(frac) = opt_frac {
                return Ok(Self::FracExp { int, frac, exp });
            } else {
                return Ok(Self::Exp { int, exp });
            }
        }

        if let Some(frac) = opt_frac {
            Ok(Self::Frac { frac, int })
        } else {
            Ok(Self::Int(int))
        }
    }
}

#[derive(Debug)]
struct Object<'src> {
    items: Vec<(&'src str, Value<'src>)>,
}

impl<'src> Parsing<'src> for Object<'src> {
    fn parse(ctx: &mut Context<'src>) -> Result<Self, Error> {
        ctx.consume('{')?;
        ctx.consume_whitespace();

        // empty object
        if ctx.consume('}').is_ok() {
            return Ok(Self { items: Vec::new() });
        }

        let mut items = Vec::new();

        loop {
            ctx.consume_whitespace();
            let key = ctx.consume_str().failing(ctx)?;
            ctx.consume_whitespace();

            ctx.consume(':').failing(ctx)?;

            ctx.descent()?;
            let val = Value::parse(ctx).failing(ctx)?;
            ctx.ascent();

            items.push((key, val));


            match ctx.peek() {
                Some(',') => {}
                Some('}') => break,
                _ => return ctx.failing("missing ',' delimiter"),
            }

            ctx.offset += 1;
        }

        ctx.offset += 1;
        Ok(Self { items })
    }
}

#[derive(Debug)]
struct Array<'src> {
    items: Vec<Value<'src>>,
}

impl<'src> Parsing<'src> for Array<'src> {
    fn parse(ctx: &mut Context<'src>) -> Result<Self, Error> {
        ctx.consume('[')?;
        ctx.consume_whitespace();

        // empty array
        if ctx.consume(']').is_ok() {
            return Ok(Self { items: Vec::new() });
        }

        let mut items = Vec::new();

        loop {
            ctx.descent()?;
            items.push(Value::parse(ctx).failing(ctx)?);
            ctx.ascent();

            match ctx.peek() {
                Some(',') => {}
                Some(']') => break,
                _ => return ctx.failing("missing ',' delimiter"),
            }

            ctx.offset += 1;
        }

        ctx.offset += 1;
        Ok(Self { items })
    }
}

fn parse(src: &str) -> Result<Value, Error> {
    let mut ctx = Context::new(src);
    let val = Value::parse(&mut ctx)?;

    if !ctx.src().is_empty() {
        return ctx.error("trailing characters");
    }

    Ok(val)
}

fn main() {
    println!("{:#?}", parse(r#"{ "key": [
        1,
        2,
        3
    ] }"#));
}
