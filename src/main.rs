use std::env;
use std::io::{self, IsTerminal, Read, Write};
use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PRECISION: usize = 12;
const MAX_PRECISION: usize = 15;

const EXIT_OK: u8 = 0;
const EXIT_ERROR: u8 = 2;

fn main() -> ExitCode {
    ExitCode::from(run())
}

fn run() -> u8 {
    let opts = match parse_args(env::args().skip(1).collect()) {
        Ok(Some(opts)) => opts,
        Ok(None) => return EXIT_OK,
        Err(msg) => return usage_error(&msg),
    };

    let source = match resolve_expression(&opts) {
        Ok(source) => source,
        Err(msg) => return usage_error(&msg),
    };

    match evaluate(&source) {
        Ok(value) => {
            let display = format_number(value, opts.precision);
            let line = if opts.json {
                format!(
                    "{{\"expression\":{},\"value\":{},\"display\":{}}}",
                    json_string(&source),
                    json_number(value),
                    json_string(&display)
                )
            } else {
                display
            };
            match emit(&line) {
                Ok(()) => EXIT_OK,
                Err(msg) => usage_error(&msg),
            }
        }
        Err(err) => {
            let msg = err.message(&source);
            if opts.json {
                let line = format!(
                    "{{\"expression\":{},\"error\":{}}}",
                    json_string(&source),
                    json_string(&msg)
                );
                if let Err(write_err) = emit(&line) {
                    return usage_error(&write_err);
                }
            } else {
                eprintln!("calc: {source}: {msg}");
            }
            EXIT_ERROR
        }
    }
}

fn usage_error(msg: &str) -> u8 {
    eprintln!("calc: {msg}");
    EXIT_ERROR
}

struct Options {
    json: bool,
    precision: usize,
    expression: Option<String>,
}

fn parse_args(args: Vec<String>) -> Result<Option<Options>, String> {
    let mut json = false;
    let mut precision = DEFAULT_PRECISION;
    let mut parts: Vec<String> = Vec::new();
    let mut args = args.into_iter();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("calc {VERSION}");
                return Ok(None);
            }
            "-j" | "--json" => json = true,
            "-p" | "--precision" => match args.next() {
                Some(value) => precision = parse_precision(&value)?,
                None => return Err("option `--precision` requires a value".into()),
            },
            "--" => {
                parts.extend(args);
                break;
            }
            _ => {
                if let Some(value) = arg.strip_prefix("--precision=") {
                    precision = parse_precision(value)?;
                } else if let Some(value) = arg.strip_prefix("-p") {
                    if value.bytes().all(|b| b.is_ascii_digit()) && !value.is_empty() {
                        precision = parse_precision(value)?;
                    } else {
                        parts.push(arg);
                    }
                } else if arg.starts_with("--") {
                    return Err(format!("unknown option `{arg}`"));
                } else {
                    parts.push(arg);
                }
            }
        }
    }

    Ok(Some(Options {
        json,
        precision,
        expression: if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        },
    }))
}

fn parse_precision(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(n) if n <= MAX_PRECISION => Ok(n),
        _ => Err(format!(
            "invalid precision `{value}`: expected an integer from 0 to {MAX_PRECISION}"
        )),
    }
}

fn resolve_expression(opts: &Options) -> Result<String, String> {
    if let Some(expression) = &opts.expression {
        if expression.trim() == "-" {
            return read_stdin();
        }
        if !expression.trim().is_empty() {
            return Ok(expression.trim().to_string());
        }
    }

    if io::stdin().is_terminal() {
        return Err("no expression given (try `calc --help`)".into());
    }

    read_stdin()
}

fn read_stdin() -> Result<String, String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| format!("cannot read stdin: {e}"))?;

    let input = input.trim();
    if input.is_empty() {
        return Err("empty expression".into());
    }
    Ok(input.to_string())
}

fn emit(line: &str) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    match writeln!(stdout, "{line}") {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(format!("cannot write output: {e}")),
    }
}

fn print_help() {
    println!(
        "\
calc {VERSION}
A Rust CLI to evaluate arithmetic expressions

Usage:
  calc [OPTIONS] <EXPRESSION>...
  calc [OPTIONS] -
  ... | calc [OPTIONS]

Arguments:
  <EXPRESSION>  Arithmetic expression; multiple arguments are joined with spaces

Options:
  -j, --json            Print a JSON object instead of plain text
  -p, --precision <N>   Decimal places for output, 0-15 [default: {DEFAULT_PRECISION}]
  -h, --help            Print help
  -V, --version         Print version

Operators:
  +  -  *  /  %  ^  ( )
  % is modulo, ^ is exponentiation

Functions:
  sqrt cbrt abs ln log log2 exp sin cos tan asin acos atan
  floor ceil round trunc sign pow atan2 min max

Constants:
  pi e tau

Numbers:
  Decimal, scientific notation, and _ separators: 3.5, 1e-3, 1_000

Exit codes:
  0 success, 2 invalid expression or usage error"
    );
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
}

#[derive(Debug)]
struct Error {
    msg: String,
    pos: usize,
}

impl Error {
    fn new(msg: impl Into<String>, pos: usize) -> Self {
        Self {
            msg: msg.into(),
            pos,
        }
    }

    fn message(&self, source: &str) -> String {
        format!("{} at column {}", self.msg, column(source, self.pos))
    }
}

fn column(source: &str, pos: usize) -> usize {
    source[..pos.min(source.len())].chars().count() + 1
}

fn lex(source: &str) -> Result<Vec<(Token, usize)>, Error> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'0'..=b'9' | b'.' => {
                let start = i;
                let mut number = String::new();

                while i < bytes.len() {
                    match bytes[i] {
                        c if c.is_ascii_digit() || c == b'.' => {
                            number.push(c as char);
                            i += 1;
                        }
                        b'_' => i += 1,
                        _ => break,
                    }
                }

                if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                    number.push('e');
                    i += 1;
                    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                        number.push(bytes[i] as char);
                        i += 1;
                    }
                    let digits = i;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        number.push(bytes[i] as char);
                        i += 1;
                    }
                    if i == digits {
                        return Err(Error::new("invalid exponent", i));
                    }
                }

                let value = number
                    .parse::<f64>()
                    .map_err(|_| Error::new(format!("invalid number `{number}`"), start))?;
                tokens.push((Token::Number(value), start));
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let name = source[start..i].to_ascii_lowercase();
                tokens.push((Token::Ident(name), start));
            }
            b'+' => {
                tokens.push((Token::Plus, i));
                i += 1;
            }
            b'-' => {
                tokens.push((Token::Minus, i));
                i += 1;
            }
            b'*' => {
                tokens.push((Token::Star, i));
                i += 1;
            }
            b'/' => {
                tokens.push((Token::Slash, i));
                i += 1;
            }
            b'%' => {
                tokens.push((Token::Percent, i));
                i += 1;
            }
            b'^' => {
                tokens.push((Token::Caret, i));
                i += 1;
            }
            b'(' => {
                tokens.push((Token::LParen, i));
                i += 1;
            }
            b')' => {
                tokens.push((Token::RParen, i));
                i += 1;
            }
            b',' => {
                tokens.push((Token::Comma, i));
                i += 1;
            }
            _ => {
                let ch = source[i..].chars().next().unwrap_or('?');
                return Err(Error::new(format!("unexpected character `{ch}`"), i));
            }
        }
    }

    Ok(tokens)
}

fn evaluate(source: &str) -> Result<f64, Error> {
    let tokens = lex(source)?;
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
        end: source.len(),
    };

    let value = parser.expression()?;
    if let Some((token, pos)) = parser.peek_token() {
        return Err(Error::new(format!("unexpected {}", describe(token)), pos));
    }
    Ok(value)
}

struct Parser<'a> {
    tokens: &'a [(Token, usize)],
    pos: usize,
    end: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|(token, _)| token)
    }

    fn peek_token(&self) -> Option<(&Token, usize)> {
        self.tokens.get(self.pos).map(|(token, pos)| (token, *pos))
    }

    fn position(&self) -> usize {
        self.tokens
            .get(self.pos)
            .map(|(_, pos)| *pos)
            .unwrap_or(self.end)
    }

    fn expression(&mut self) -> Result<f64, Error> {
        let mut value = self.term()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.pos += 1;
                    value += self.term()?;
                }
                Some(Token::Minus) => {
                    self.pos += 1;
                    value -= self.term()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn term(&mut self) -> Result<f64, Error> {
        let mut value = self.unary()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.pos += 1;
                    value *= self.unary()?;
                }
                Some(Token::Slash) => {
                    self.pos += 1;
                    value /= self.unary()?;
                }
                Some(Token::Percent) => {
                    self.pos += 1;
                    value %= self.unary()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn unary(&mut self) -> Result<f64, Error> {
        match self.peek() {
            Some(Token::Minus) => {
                self.pos += 1;
                Ok(-self.unary()?)
            }
            Some(Token::Plus) => {
                self.pos += 1;
                self.unary()
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<f64, Error> {
        let base = self.primary()?;
        if self.peek() == Some(&Token::Caret) {
            self.pos += 1;
            let exponent = self.unary()?;
            Ok(base.powf(exponent))
        } else {
            Ok(base)
        }
    }

    fn primary(&mut self) -> Result<f64, Error> {
        let Some((token, pos)) = self.peek_token().map(|(t, p)| (t.clone(), p)) else {
            return Err(Error::new("unexpected end of expression", self.end));
        };
        self.pos += 1;

        match token {
            Token::Number(value) => Ok(value),
            Token::LParen => {
                let value = self.expression()?;
                match self.peek() {
                    Some(Token::RParen) => {
                        self.pos += 1;
                        Ok(value)
                    }
                    _ => Err(Error::new("expected `)`", self.position())),
                }
            }
            Token::Ident(name) => {
                if self.peek() == Some(&Token::LParen) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Token::RParen) {
                        loop {
                            args.push(self.expression()?);
                            if self.peek() == Some(&Token::Comma) {
                                self.pos += 1;
                            } else {
                                break;
                            }
                        }
                    }
                    match self.peek() {
                        Some(Token::RParen) => self.pos += 1,
                        _ => return Err(Error::new("expected `)`", self.position())),
                    }
                    call(&name, &args, pos)
                } else {
                    constant(&name, pos)
                }
            }
            other => Err(Error::new(format!("unexpected {}", describe(&other)), pos)),
        }
    }
}

fn describe(token: &Token) -> String {
    match token {
        Token::Number(_) => "number".into(),
        Token::Ident(name) => format!("identifier `{name}`"),
        Token::Plus => "`+`".into(),
        Token::Minus => "`-`".into(),
        Token::Star => "`*`".into(),
        Token::Slash => "`/`".into(),
        Token::Percent => "`%`".into(),
        Token::Caret => "`^`".into(),
        Token::LParen => "`(`".into(),
        Token::RParen => "`)`".into(),
        Token::Comma => "`,`".into(),
    }
}

fn constant(name: &str, pos: usize) -> Result<f64, Error> {
    match name {
        "pi" => Ok(std::f64::consts::PI),
        "e" => Ok(std::f64::consts::E),
        "tau" => Ok(std::f64::consts::TAU),
        _ => Err(Error::new(format!("unknown identifier `{name}`"), pos)),
    }
}

fn call(name: &str, args: &[f64], pos: usize) -> Result<f64, Error> {
    let check = |expected: usize| -> Result<(), Error> {
        if args.len() == expected {
            return Ok(());
        }
        let plural = if expected == 1 { "" } else { "s" };
        Err(Error::new(
            format!(
                "`{name}` takes {expected} argument{plural}, got {}",
                args.len()
            ),
            pos,
        ))
    };
    let one = |f: fn(f64) -> f64| -> Result<f64, Error> {
        check(1)?;
        Ok(f(args[0]))
    };
    let two = |f: fn(f64, f64) -> f64| -> Result<f64, Error> {
        check(2)?;
        Ok(f(args[0], args[1]))
    };

    match name {
        "sqrt" => one(f64::sqrt),
        "cbrt" => one(f64::cbrt),
        "abs" => one(f64::abs),
        "ln" => one(f64::ln),
        "log" => one(f64::log10),
        "log2" => one(f64::log2),
        "exp" => one(f64::exp),
        "sin" => one(f64::sin),
        "cos" => one(f64::cos),
        "tan" => one(f64::tan),
        "asin" => one(f64::asin),
        "acos" => one(f64::acos),
        "atan" => one(f64::atan),
        "floor" => one(f64::floor),
        "ceil" => one(f64::ceil),
        "round" => one(f64::round),
        "trunc" => one(f64::trunc),
        "sign" => one(f64::signum),
        "pow" => two(f64::powf),
        "atan2" => two(f64::atan2),
        "min" => two(f64::min),
        "max" => two(f64::max),
        _ => Err(Error::new(format!("unknown function `{name}`"), pos)),
    }
}

fn format_number(value: f64, precision: usize) -> String {
    if value.is_nan() {
        return "nan".into();
    }
    if value.is_infinite() {
        return if value.is_sign_positive() {
            "inf".into()
        } else {
            "-inf".into()
        };
    }
    if value == 0.0 {
        return "0".into();
    }

    let magnitude = value.abs();
    if !(1e-6..1e16).contains(&magnitude) {
        return trim_exponent(format!("{:.*e}", precision.saturating_sub(1), value));
    }

    let mut text = format!("{value:.precision$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".into();
    }
    text
}

fn trim_exponent(text: String) -> String {
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };
    let mut mantissa = mantissa.to_string();
    if mantissa.contains('.') {
        while mantissa.ends_with('0') {
            mantissa.pop();
        }
        if mantissa.ends_with('.') {
            mantissa.pop();
        }
    }
    format!("{mantissa}e{exponent}")
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn json_number(value: f64) -> String {
    if value.is_finite() {
        format!("{value}")
    } else {
        "null".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(source: &str) -> f64 {
        evaluate(source).unwrap()
    }

    #[test]
    fn precedence_and_parentheses() {
        assert_eq!(eval("2 + 3 * 4"), 14.0);
        assert_eq!(eval("(2 + 3) * 4"), 20.0);
        assert_eq!(eval("10 - 4 - 3"), 3.0);
        assert_eq!(eval("10 % 4"), 2.0);
    }

    #[test]
    fn power_is_right_associative_and_binds_above_unary_minus() {
        assert_eq!(eval("2^3^2"), 512.0);
        assert_eq!(eval("-2^2"), -4.0);
        assert_eq!(eval("2^-3"), 0.125);
    }

    #[test]
    fn numbers_and_separators() {
        assert_eq!(eval("1_000 / 4"), 250.0);
        assert_eq!(eval("1e3 + .5"), 1000.5);
    }

    #[test]
    fn functions_and_constants() {
        assert_eq!(eval("sqrt(9) + min(1, 2)"), 4.0);
        assert_eq!(eval("max(2, 5) - abs(-3)"), 2.0);
        assert!((eval("pi") - std::f64::consts::PI).abs() < 1e-15);
        assert_eq!(eval("pow(2, 10)"), 1024.0);
    }

    #[test]
    fn errors_point_at_the_problem() {
        let err = evaluate("1 +").unwrap_err();
        assert_eq!(
            err.message("1 +"),
            "unexpected end of expression at column 4"
        );

        let err = evaluate("2foo").unwrap_err();
        assert_eq!(
            err.message("2foo"),
            "unexpected identifier `foo` at column 2"
        );

        let err = evaluate("nope(1)").unwrap_err();
        assert_eq!(
            err.message("nope(1)"),
            "unknown function `nope` at column 1"
        );

        let err = evaluate("sqrt(1, 2)").unwrap_err();
        assert_eq!(
            err.message("sqrt(1, 2)"),
            "`sqrt` takes 1 argument, got 2 at column 1"
        );
    }

    #[test]
    fn formatting_removes_float_noise() {
        assert_eq!(format_number(0.1 + 0.2, 12), "0.3");
        assert_eq!(format_number(1.0 / 3.0, 12), "0.333333333333");
        assert_eq!(format_number(2.0 / 3.0, 2), "0.67");
        assert_eq!(format_number(14.0, 12), "14");
        assert_eq!(format_number(-0.0, 12), "0");
        assert_eq!(format_number(1e20, 12), "1e20");
        assert_eq!(format_number(1e-7, 12), "1e-7");
    }

    #[test]
    fn json_escapes_special_characters() {
        assert_eq!(json_string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
        assert_eq!(json_number(f64::INFINITY), "null");
    }
}
