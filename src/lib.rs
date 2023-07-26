///! Crate for parsing json.

use std::fmt;
mod tests;

const MAX_DEPTH: usize = 256;

/// Error with line number and context. 
pub struct Error {
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

/// Trait related to any data structure that json supports.
trait Parsing<'src>: Sized {
    fn parse(ctx: &mut Context<'src>) -> Result<Self, Error>;
}

/// Trait for indicating that any error has to be propagated up.
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

/// Information required at runtime when parsing json>
#[derive(Debug)]
struct Context<'src> {
    /// Reference to input string
    src: &'src str,

    /// Offset into input string
    offset: usize,

    /// Recursion depth
    depth: usize,

    /// Indicator used by the [`Failing`] trait
    is_failing: bool,
}

impl<'src> Context<'src> {
    /// Create's a new [`Context`] required for parsing json
    fn new(src: &'src str) -> Self {
        Self {
            src,
            offset: 0,
            depth: 0,
            is_failing: false,
        }
    }

    /// Where we are in the string.
    fn src(&self) -> &'src str {
        &self.src[self.offset..]
    }

    /// Increases the known recursion depth and checks for if we overflow [`MAX_DEPTH`].
    fn descent(&mut self) -> Result<(), Error> {
        self.depth += 1;

        if self.depth == MAX_DEPTH {
            self.failing("reached recursion depth")
        } else {
            Ok(())
        }
    }

    /// Decreases the known recursion depth.
    fn ascent(&mut self) {
        self.depth -= 1;
    }

    /// Formats an [`Error`] by getting the current offset.
    fn error<T>(&self, msg: &str) -> Result<T, Error> {
        Err(Error {
            offset: self.offset,
            msg: msg.to_string(),
        })
    }

    /// Formats an [`Error`] by getting the current offset whilst 
    /// notifying that the error has to also be propagated up. 
    fn failing<T>(&mut self, msg: &str) -> Result<T, Error> {
        self.is_failing = true;
        self.error(msg)
    }

    /// Return the next character in the stream.
    fn peek(&mut self) -> Option<char> {
        self.src().chars().next()
    }

    /// Conditionally increments the stream if the first character matches `chr`.
    fn consume(&mut self, chr: char) -> Result<(), Error> {
        match self.peek() {
            Some(got) if got == chr => {
                self.offset += 1;
                Ok(())
            }
            Some(got) => self.error(&format!("expected '{chr}' got '{got}'")),
            None => self.error(&format!("expected '{chr}' got EOF")),
        }
    }

    /// Conditionally increments the stream if the stream starts with `s`.
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

    /// Increments the stream whilst any whitespace is encountered. 
    fn consume_whitespace(&mut self) {
        // all forms of accepted whitespace
        while let Some('\u{0020}' | '\u{000a}' | '\u{000d}' | '\u{0009}') = self.peek() {
            self.offset += 1;
        } 
    }

    /// Read a string that starts and ends with '"'.
    /// Increments stream past the string.
    fn consume_str(&mut self) -> Result<&'src str, Error> {
        self.consume('"')?;
        let start = self.offset;

        loop {
            if let Some('"') = self.peek() {
                break;
            }

            match self.peek() {
                // TODO: check character range
                Some(..) => self.offset += 1,
                None => return self.error("reached EOF on string"),
            }
        }

        self.offset += 1;
        Ok(&self.src[start..self.offset])
    }

    /// Reads a whole number, both negative or positive.
    /// Increments stream past the integer.
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

/// Either a root value, an [`Array`]'s item or an [`Object`]'s value.
#[derive(Debug)]
pub enum Value<'src> {
    /// ```text
    /// '{' <ws> '}' | '{' {<ws> <string> <ws> ':' <ws> <value> <ws>} '}'
    /// ```
    Object(Object<'src>),

    /// ```text
    /// '[' <ws> ']' | '{' {<ws> <value> <ws>} '}'
    /// ```
    Array(Array<'src>),

    /// ```text
    /// '"' {'0020' . '10ffff' - '"' - '\' | <escape>} '"'
    ///
    /// <escape> = '"'
    ///          | '\'
    ///          | '/'
    ///          | 'b'
    ///          | 'f'
    ///          | 'n'
    ///          | 'r'
    ///          | 't'
    ///          | 'u' <hex> <hex> <hex> <hex>
    ///
    /// <hex> = digit
    ///       | 'a'..'f'
    ///       | 'A'..'F'
    /// ```
    String(&'src str),

    /// ```text
    /// <integer> | <fraction> | <exponent>
    ///
    /// <integer> = ['-'] {<digit>}+
    /// <fraction> = <integer> '.' {<digit>}+
    /// <exponent> = <fraction> ('E' | 'e') ['+' | '-'] {<digit>}+
    /// ```
    Number(Number),

    /// ```text
    /// "true"
    /// ```
    True,

    /// ```text
    /// "false"
    /// ```
    False,

    /// ```text
    /// "null"
    /// ```
    Null,
}

impl<'src> Value<'src> {
    /// Reads [`Value`] excluding any whitespace.
    /// Increments stream past [`Value`].
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
    /// Reads [`Value`] and whitespace.
    /// Increments stream past [`Value`].
    fn parse(ctx: &mut Context<'src>) -> Result<Self, Error> {
        ctx.consume_whitespace();
        let this = Self::parse_inner(ctx);
        ctx.consume_whitespace();
        this
    }
}

/// whole integer, floating-point integer or floating-point integer in scientific notation.
#[derive(Debug)]
pub enum Number {
    Int(isize),
    Frac { int: isize, frac: isize },
    Exp { int: isize, exp: isize },
    FracExp { int: isize, frac: isize, exp: isize },
}

impl Parsing<'_> for Number {
    /// Reads either an whole integer, floating-point integer or floating-point
    /// integer in scientific notation.
    /// Increments stream past [`Number`].
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

/// List of (key, value) pairs.
#[derive(Debug)]
pub struct Object<'src> {
    pub items: Vec<(&'src str, Value<'src>)>,
}

impl<'src> Parsing<'src> for Object<'src> {
    /// Reads a list of "key": [`Value`] pairs, starting with '{', ending with '}' and delimited
    /// by ','. Increments stream past key's and [`Value`]'s.
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

/// List of values.
#[derive(Debug)]
pub struct Array<'src> {
    pub items: Vec<Value<'src>>,
}

impl<'src> Parsing<'src> for Array<'src> {
    /// Read list of values, starting with '[', ending with ']' and delimited by ','.
    /// Increments stream past [`Value`]'s.
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

/// Tries to parse json [`Value`].
pub fn parse(src: &str) -> Result<Value, Error> {
    let mut ctx = Context::new(src);
    let val = Value::parse(&mut ctx)?;

    if !ctx.src().is_empty() {
        return ctx.error("trailing characters");
    }

    Ok(val)
}
