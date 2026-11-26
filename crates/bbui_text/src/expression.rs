//! Numeric expressions for editable values. Evaluation never mutates a control.

use bbui_core::NumericValue;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    InvalidCharacter(usize),
    InvalidNumber(usize),
    UnexpectedToken,
    UnknownName(String),
    InvalidArguments(String),
    DivisionByZero,
    Domain,
    Overflow,
    TooComplex,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCharacter(at) => write!(formatter, "invalid character at byte {at}"),
            Self::InvalidNumber(at) => write!(formatter, "invalid number at byte {at}"),
            Self::UnexpectedToken => formatter.write_str("unexpected expression token"),
            Self::UnknownName(name) => write!(formatter, "unknown name: {name}"),
            Self::InvalidArguments(name) => write!(formatter, "invalid arguments for {name}"),
            Self::DivisionByZero => formatter.write_str("division by zero"),
            Self::Domain => formatter.write_str("expression is outside the function domain"),
            Self::Overflow => formatter.write_str("numeric overflow"),
            Self::TooComplex => formatter.write_str("expression is too complex"),
        }
    }
}

impl std::error::Error for Error {}

/// Evaluate integer arithmetic without passing any value through floating
/// point. The `x` variable is the control's exact current value.
fn evaluate_exact_basic(source: &str, x: i128) -> Result<i128, Error> {
    struct Parser<'a> {
        bytes: &'a [u8],
        at: usize,
        x: i128,
        depth: u8,
    }
    impl Parser<'_> {
        fn skip_space(&mut self) {
            while self.bytes.get(self.at).is_some_and(u8::is_ascii_whitespace) {
                self.at += 1;
            }
        }
        fn eat(&mut self, token: u8) -> bool {
            self.skip_space();
            if self.bytes.get(self.at) == Some(&token) {
                self.at += 1;
                true
            } else {
                false
            }
        }
        fn sum(&mut self) -> Result<i128, Error> {
            let mut value = self.product()?;
            loop {
                if self.eat(b'+') {
                    value = value.checked_add(self.product()?).ok_or(Error::Overflow)?;
                } else if self.eat(b'-') {
                    value = value.checked_sub(self.product()?).ok_or(Error::Overflow)?;
                } else {
                    return Ok(value);
                }
            }
        }
        fn product(&mut self) -> Result<i128, Error> {
            let mut value = self.atom()?;
            loop {
                if self.eat(b'*') {
                    value = value.checked_mul(self.atom()?).ok_or(Error::Overflow)?;
                } else if self.eat(b'/') {
                    let divisor = self.atom()?;
                    value = rounded_div(value, divisor)?;
                } else {
                    return Ok(value);
                }
            }
        }
        fn atom(&mut self) -> Result<i128, Error> {
            if self.eat(b'+') {
                self.depth = self
                    .depth
                    .checked_add(1)
                    .filter(|depth| *depth <= 32)
                    .ok_or(Error::TooComplex)?;
                let value = self.atom();
                self.depth -= 1;
                return value;
            }
            if self.eat(b'-') {
                self.depth = self
                    .depth
                    .checked_add(1)
                    .filter(|depth| *depth <= 32)
                    .ok_or(Error::TooComplex)?;
                let value = self
                    .atom()
                    .and_then(|value| value.checked_neg().ok_or(Error::Overflow));
                self.depth -= 1;
                return value;
            }
            if self.eat(b'(') {
                self.depth = self
                    .depth
                    .checked_add(1)
                    .filter(|depth| *depth <= 32)
                    .ok_or(Error::TooComplex)?;
                let value = self.sum()?;
                self.depth -= 1;
                if !self.eat(b')') {
                    return Err(Error::UnexpectedToken);
                }
                return Ok(value);
            }
            self.skip_space();
            let start = self.at;
            if self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
                return std::str::from_utf8(&self.bytes[start..self.at])
                    .map_err(|_| Error::InvalidNumber(start))?
                    .parse::<i128>()
                    .map_err(|_| Error::Overflow);
            }
            if self.bytes.get(self.at).is_some_and(u8::is_ascii_alphabetic) {
                while self
                    .bytes
                    .get(self.at)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    self.at += 1;
                }
                let name = std::str::from_utf8(&self.bytes[start..self.at])
                    .map_err(|_| Error::InvalidCharacter(start))?;
                return if name == "x" {
                    Ok(self.x)
                } else {
                    Err(Error::UnknownName(name.into()))
                };
            }
            Err(Error::UnexpectedToken)
        }
    }
    if source.len() > 4_096 {
        return Err(Error::TooComplex);
    }
    let mut parser = Parser {
        bytes: source.as_bytes(),
        at: 0,
        x,
        depth: 0,
    };
    let value = parser.sum()?;
    parser.skip_space();
    if parser.at != parser.bytes.len() {
        return Err(Error::InvalidCharacter(parser.at));
    }
    Ok(value)
}

fn rounded_div(value: i128, divisor: i128) -> Result<i128, Error> {
    if divisor == 0 {
        return Err(Error::DivisionByZero);
    }
    let quotient = value.checked_div(divisor).ok_or(Error::Overflow)?;
    let remainder = value.checked_rem(divisor).ok_or(Error::Overflow)?;
    if remainder == 0 {
        return Ok(quotient);
    }
    let denominator = divisor.unsigned_abs();
    let numerator = remainder.unsigned_abs();
    if numerator < denominator.div_ceil(2) {
        return Ok(quotient);
    }
    quotient
        .checked_add(if (value < 0) == (divisor < 0) { 1 } else { -1 })
        .ok_or(Error::Overflow)
}

pub fn evaluate_exact_integer(source: &str, x: i128) -> Result<i128, Error> {
    evaluate_exact_basic(source, x)
}

/// Evaluate the shared expression language while retaining exact integer
/// values. Named integer variables retain their full width; fractional
/// expressions remain available for safe-width fields.
pub fn evaluate_exact_integer_with_vars(
    source: &str,
    x: i128,
    vars: &HashMap<String, NumericValue>,
) -> Result<i128, Error> {
    match evaluate_exact_basic(source, x) {
        Ok(value) => Ok(value),
        // The basic parser can evaluate a branch before discovering a trailing
        // conditional. Let the full grammar decide which branch is live.
        Err(_) => parse::<true>(source)?.evaluate_exact(x, vars),
    }
}

pub fn evaluate_exact_i64(source: &str, x: i64) -> Result<i64, Error> {
    i64::try_from(evaluate_exact_integer(source, i128::from(x))?).map_err(|_| Error::Overflow)
}

pub fn evaluate_exact_u64(source: &str, x: u64) -> Result<u64, Error> {
    u64::try_from(evaluate_exact_integer(source, i128::from(x))?).map_err(|_| Error::Overflow)
}

pub fn evaluate_exact_i64_with_vars(
    source: &str,
    x: i64,
    vars: &HashMap<String, NumericValue>,
) -> Result<i64, Error> {
    i64::try_from(evaluate_exact_integer_with_vars(
        source,
        i128::from(x),
        vars,
    )?)
    .map_err(|_| Error::Overflow)
}

pub fn evaluate_exact_u64_with_vars(
    source: &str,
    x: u64,
    vars: &HashMap<String, NumericValue>,
) -> Result<u64, Error> {
    u64::try_from(evaluate_exact_integer_with_vars(
        source,
        i128::from(x),
        vars,
    )?)
    .map_err(|_| Error::Overflow)
}

#[cfg(test)]
mod exact_integer_tests {
    use super::*;

    #[test]
    fn full_width_values_and_checked_arithmetic() {
        assert_eq!(evaluate_exact_i64("-9223372036854775808", 0), Ok(i64::MIN));
        assert_eq!(evaluate_exact_i64("x + 1", i64::MAX - 1), Ok(i64::MAX));
        assert_eq!(evaluate_exact_u64("18446744073709551615", 0), Ok(u64::MAX));
        assert_eq!(evaluate_exact_u64("x + 1", u64::MAX), Err(Error::Overflow));
        assert_eq!(evaluate_exact_i64("(2 + 3) * 4", 0), Ok(20));
        assert_eq!(evaluate_exact_i64("1 / 0", 0), Err(Error::DivisionByZero));
        assert_eq!(
            evaluate_exact_i64(&"-".repeat(64), 0),
            Err(Error::TooComplex)
        );
        assert_eq!(evaluate_exact_u64("-1", 0), Err(Error::Overflow));
        assert!(evaluate_exact_i64("1.5", 0).is_err());
    }

    #[test]
    fn exact_values_support_named_variables_and_shared_expression_forms() {
        let vars = HashMap::from([
            ("c".into(), NumericValue::Integer(1)),
            ("y".into(), NumericValue::Unsigned(7)),
        ]);
        assert_eq!(
            evaluate_exact_u64_with_vars("x if c else y", u64::MAX, &vars),
            Ok(u64::MAX)
        );
        assert_eq!(
            evaluate_exact_u64_with_vars("max(x, 18446744073709551614)", u64::MAX, &vars),
            Ok(u64::MAX)
        );
        assert_eq!(
            evaluate_exact_u64_with_vars("x - y*2", u64::MAX, &vars),
            Ok(u64::MAX - 14)
        );
        assert_eq!(
            evaluate_exact_u64_with_vars("x + 1.0", u64::MAX - 1, &vars),
            Ok(u64::MAX)
        );
        assert_eq!(evaluate_exact_i64_with_vars("2**3 + 7%4", 0, &vars), Ok(11));
        assert_eq!(
            evaluate_exact_i64_with_vars("1 if 1 else 1/0", 0, &vars),
            Ok(1)
        );
        assert_eq!(
            evaluate_exact_i64_with_vars("1/0 if 0 else 2", 0, &vars),
            Ok(2)
        );
        assert_eq!(
            evaluate_exact_i64_with_vars(
                "170141183460469231731687303715884105727+1 if 0 else 2",
                0,
                &vars
            ),
            Ok(2)
        );
        assert_eq!(
            evaluate_exact_i64_with_vars("1/0 if 1 else 2", 0, &vars),
            Err(Error::DivisionByZero)
        );
        assert_eq!(evaluate_exact_i64("3/2", 0), Ok(2));
        let wide = HashMap::from([("limit".into(), NumericValue::Unsigned(u64::MAX - 1))]);
        assert_eq!(
            evaluate_exact_u64_with_vars("x == limit", u64::MAX - 1, &wide),
            Ok(1)
        );
        assert_eq!(
            evaluate_exact_u64_with_vars("x - limit", u64::MAX, &wide),
            Ok(1)
        );
        assert_eq!(
            evaluate_exact_i64_with_vars("sqrt(2)*2", 0, &wide),
            Err(Error::Domain)
        );
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    ExactNumber(i128),
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Power,
    LParen,
    RParen,
    Comma,
    If,
    Else,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Eq,
    NotEq,
    End,
}

fn lex<const EXACT: bool>(source: &str) -> Result<Vec<Token>, Error> {
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let token = match bytes[i] {
            b'0'..=b'9' | b'.' => {
                if bytes[i] == b'.' && !bytes.get(i + 1).is_some_and(u8::is_ascii_digit) {
                    return Err(Error::InvalidNumber(i));
                }
                while bytes
                    .get(i)
                    .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
                {
                    i += 1;
                }
                if bytes.get(i).is_some_and(|b| *b == b'e' || *b == b'E') {
                    i += 1;
                    if bytes.get(i).is_some_and(|b| *b == b'+' || *b == b'-') {
                        i += 1;
                    }
                    let digits = i;
                    while bytes.get(i).is_some_and(u8::is_ascii_digit) {
                        i += 1;
                    }
                    if digits == i {
                        return Err(Error::InvalidNumber(start));
                    }
                }
                let raw = &source[start..i];
                let float: f64 = raw.parse().map_err(|_| Error::InvalidNumber(start))?;
                if EXACT {
                    let exact = raw.parse::<i128>().ok().or_else(|| {
                        (float.is_finite()
                            && float.fract() == 0.0
                            && float.abs() <= 9_007_199_254_740_992.0)
                            .then_some(float as i128)
                    });
                    out.push(exact.map_or(Token::Number(float), Token::ExactNumber));
                } else {
                    out.push(Token::Number(float));
                }
                continue;
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                i += 1;
                while bytes
                    .get(i)
                    .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
                {
                    i += 1;
                }
                let word = &source[start..i];
                out.push(match word {
                    "if" => Token::If,
                    "else" => Token::Else,
                    _ => Token::Name(word.into()),
                });
                continue;
            }
            b'+' => Token::Plus,
            b'-' => Token::Minus,
            b'/' => Token::Slash,
            b'%' => Token::Percent,
            b'(' => Token::LParen,
            b')' => Token::RParen,
            b',' => Token::Comma,
            b'*' if bytes.get(i + 1) == Some(&b'*') => {
                i += 1;
                Token::Power
            }
            b'*' => Token::Star,
            b'<' if bytes.get(i + 1) == Some(&b'=') => {
                i += 1;
                Token::LessEq
            }
            b'<' => Token::Less,
            b'>' if bytes.get(i + 1) == Some(&b'=') => {
                i += 1;
                Token::GreaterEq
            }
            b'>' => Token::Greater,
            b'=' if bytes.get(i + 1) == Some(&b'=') => {
                i += 1;
                Token::Eq
            }
            b'!' if bytes.get(i + 1) == Some(&b'=') => {
                i += 1;
                Token::NotEq
            }
            _ => return Err(Error::InvalidCharacter(i)),
        };
        i += 1;
        out.push(token);
    }
    out.push(Token::End);
    Ok(out)
}

/// Whether an expression contains a numeric literal that cannot be safely
/// passed through the legacy floating-point evaluator.
pub fn has_wide_numeric_literal(source: &str) -> bool {
    lex::<true>(source).is_ok_and(|tokens| {
        tokens.into_iter().any(|token| match token {
            Token::ExactNumber(value) => value.unsigned_abs() > 9_007_199_254_740_992,
            Token::Number(value) => value.abs() > 9_007_199_254_740_992.0,
            _ => false,
        })
    })
}

pub fn evaluate(source: &str, vars: &HashMap<String, f64>) -> Result<f64, Error> {
    parse::<false>(source)?.evaluate(
        &|name| {
            vars.get(name)
                .copied()
                .ok_or_else(|| Error::UnknownName(name.into()))
        },
        false,
    )
}

/// Evaluate a float expression against typed field variables without copying
/// the map. The field's current value always overrides a named `x` variable.
pub fn evaluate_typed(
    source: &str,
    x: f64,
    vars: &HashMap<String, NumericValue>,
) -> Result<f64, Error> {
    parse::<false>(source)?.evaluate(
        &|name| {
            if name == "x" {
                Ok(x)
            } else {
                vars.get(name)
                    .map(|value| value.as_f64())
                    .ok_or_else(|| Error::UnknownName(name.into()))
            }
        },
        false,
    )
}

/// Evaluate only safe-width values on the selected expression path.
pub fn evaluate_typed_safe(
    source: &str,
    x: f64,
    vars: &HashMap<String, NumericValue>,
) -> Result<f64, Error> {
    let safe = |value: f64| {
        if value.is_finite() && value.abs() <= 9_007_199_254_740_992.0 {
            Ok(value)
        } else {
            Err(Error::Domain)
        }
    };
    parse::<true>(source)?.evaluate(
        &|name| {
            if name == "x" {
                safe(x)
            } else {
                vars.get(name)
                    .map(|value| match value {
                        NumericValue::Integer(value)
                            if value.unsigned_abs() <= 9_007_199_254_740_992 =>
                        {
                            safe(*value as f64)
                        }
                        NumericValue::Unsigned(value) if *value <= 9_007_199_254_740_992 => {
                            safe(*value as f64)
                        }
                        NumericValue::Float(value) => safe(*value),
                        _ => Err(Error::Domain),
                    })
                    .ok_or_else(|| Error::UnknownName(name.into()))?
            }
        },
        true,
    )
}

fn parse<const EXACT: bool>(source: &str) -> Result<Expression, Error> {
    if source.len() > 4_096 {
        return Err(Error::TooComplex);
    }
    let tokens = lex::<EXACT>(source)?;
    if tokens.len() > 512 {
        return Err(Error::TooComplex);
    }
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
    };
    let expression = parser.conditional()?;
    if parser.peek() != &Token::End {
        return Err(Error::UnexpectedToken);
    }
    Ok(expression)
}

fn finite(value: f64) -> Result<f64, Error> {
    if value.is_nan() {
        Err(Error::Domain)
    } else if value.is_infinite() {
        Err(Error::Overflow)
    } else {
        Ok(value)
    }
}

#[derive(Debug, Clone)]
enum Expression {
    Number(f64),
    ExactNumber(i128),
    Name(String),
    Unary {
        negative: bool,
        value: Box<Expression>,
    },
    Binary {
        operation: BinaryOperation,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Compare {
        operation: CompareOperation,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Conditional {
        value: Box<Expression>,
        condition: Box<Expression>,
        alternative: Box<Expression>,
    },
    Call {
        name: String,
        arguments: Vec<Expression>,
    },
}

#[derive(Debug, Clone, Copy)]
enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Power,
}

#[derive(Debug, Clone, Copy)]
enum CompareOperation {
    Less,
    LessEq,
    Greater,
    GreaterEq,
    Equal,
    NotEqual,
}

impl Expression {
    fn evaluate<F: Fn(&str) -> Result<f64, Error>>(
        &self,
        lookup: &F,
        safe: bool,
    ) -> Result<f64, Error> {
        match self {
            Self::Number(value) => {
                if safe && value.abs() > 9_007_199_254_740_992.0 {
                    return Err(Error::Domain);
                }
                finite(*value)
            }
            Self::ExactNumber(value) => {
                if safe && value.unsigned_abs() > 9_007_199_254_740_992 {
                    return Err(Error::Domain);
                }
                finite(*value as f64)
            }
            Self::Name(name) => match name.as_str() {
                "pi" => Ok(std::f64::consts::PI),
                "e" => Ok(std::f64::consts::E),
                _ => lookup(name).and_then(finite),
            },
            Self::Unary { negative, value } => {
                let value = value.evaluate(lookup, safe)?;
                finite(if *negative { -value } else { value })
            }
            Self::Binary {
                operation,
                left,
                right,
            } => {
                let left = left.evaluate(lookup, safe)?;
                let right = right.evaluate(lookup, safe)?;
                match operation {
                    BinaryOperation::Add => finite(left + right),
                    BinaryOperation::Subtract => finite(left - right),
                    BinaryOperation::Multiply => finite(left * right),
                    BinaryOperation::Divide if right == 0.0 => Err(Error::DivisionByZero),
                    BinaryOperation::Divide => finite(left / right),
                    BinaryOperation::Remainder if right == 0.0 => Err(Error::DivisionByZero),
                    BinaryOperation::Remainder => finite(left % right),
                    BinaryOperation::Power => finite(left.powf(right)),
                }
            }
            Self::Compare {
                operation,
                left,
                right,
            } => {
                let left = left.evaluate(lookup, safe)?;
                let right = right.evaluate(lookup, safe)?;
                let value = match operation {
                    CompareOperation::Less => left < right,
                    CompareOperation::LessEq => left <= right,
                    CompareOperation::Greater => left > right,
                    CompareOperation::GreaterEq => left >= right,
                    CompareOperation::Equal => left == right,
                    CompareOperation::NotEqual => left != right,
                };
                Ok(f64::from(value as u8))
            }
            Self::Conditional {
                value,
                condition,
                alternative,
            } => {
                if condition.evaluate(lookup, safe)? != 0.0 {
                    value.evaluate(lookup, safe)
                } else {
                    alternative.evaluate(lookup, safe)
                }
            }
            Self::Call { name, arguments } => {
                let values = arguments
                    .iter()
                    .map(|argument| argument.evaluate(lookup, safe))
                    .collect::<Result<Vec<_>, _>>()?;
                function(name, &values)
            }
        }
    }

    fn evaluate_exact(&self, x: i128, vars: &HashMap<String, NumericValue>) -> Result<i128, Error> {
        match self {
            Self::ExactNumber(value) => Ok(*value),
            Self::Number(_) => Err(Error::Domain),
            Self::Name(name) if name == "x" => Ok(x),
            Self::Name(name) if name == "pi" || name == "e" => Err(Error::Domain),
            Self::Name(name) => {
                let value = vars
                    .get(name)
                    .copied()
                    .ok_or_else(|| Error::UnknownName(name.clone()))?;
                match value {
                    NumericValue::Integer(value) => Ok(i128::from(value)),
                    NumericValue::Unsigned(value) => Ok(i128::from(value)),
                    NumericValue::Float(value)
                        if value.is_finite()
                            && value.fract() == 0.0
                            && value.abs() <= 9_007_199_254_740_992.0 =>
                    {
                        Ok(value as i128)
                    }
                    NumericValue::Float(_) => Err(Error::Domain),
                }
            }
            Self::Unary { negative, value } => {
                let value = value.evaluate_exact(x, vars)?;
                if *negative {
                    value.checked_neg().ok_or(Error::Overflow)
                } else {
                    Ok(value)
                }
            }
            Self::Binary {
                operation,
                left,
                right,
            } => {
                let left = left.evaluate_exact(x, vars)?;
                let right = right.evaluate_exact(x, vars)?;
                match operation {
                    BinaryOperation::Add => left.checked_add(right).ok_or(Error::Overflow),
                    BinaryOperation::Subtract => left.checked_sub(right).ok_or(Error::Overflow),
                    BinaryOperation::Multiply => left.checked_mul(right).ok_or(Error::Overflow),
                    BinaryOperation::Divide => rounded_div(left, right),
                    BinaryOperation::Remainder if right == 0 => Err(Error::DivisionByZero),
                    BinaryOperation::Remainder => left.checked_rem(right).ok_or(Error::Overflow),
                    BinaryOperation::Power => {
                        let exponent = u32::try_from(right).map_err(|_| Error::Domain)?;
                        left.checked_pow(exponent).ok_or(Error::Overflow)
                    }
                }
            }
            Self::Compare {
                operation,
                left,
                right,
            } => {
                let left = left.evaluate_exact(x, vars)?;
                let right = right.evaluate_exact(x, vars)?;
                Ok(i128::from(match operation {
                    CompareOperation::Less => left < right,
                    CompareOperation::LessEq => left <= right,
                    CompareOperation::Greater => left > right,
                    CompareOperation::GreaterEq => left >= right,
                    CompareOperation::Equal => left == right,
                    CompareOperation::NotEqual => left != right,
                }))
            }
            Self::Conditional {
                value,
                condition,
                alternative,
            } => {
                if condition.evaluate_exact(x, vars)? != 0 {
                    value.evaluate_exact(x, vars)
                } else {
                    alternative.evaluate_exact(x, vars)
                }
            }
            Self::Call { name, arguments } => {
                let values = arguments
                    .iter()
                    .map(|argument| argument.evaluate_exact(x, vars))
                    .collect::<Result<Vec<_>, _>>()?;
                exact_function(name, &values)
            }
        }
    }
}

fn exact_function(name: &str, args: &[i128]) -> Result<i128, Error> {
    let invalid = || Error::InvalidArguments(name.into());
    match (name, args) {
        ("min", [_first, ..]) => args.iter().copied().min().ok_or_else(invalid),
        ("max", [_first, ..]) => args.iter().copied().max().ok_or_else(invalid),
        ("abs", [value]) => value.checked_abs().ok_or(Error::Overflow),
        ("round" | "floor" | "ceil", [value]) => Ok(*value),
        ("clamp", [value, min, max]) if min <= max => Ok((*value).clamp(*min, *max)),
        ("pow", [base, exponent]) => {
            let exponent = u32::try_from(*exponent).map_err(|_| Error::Domain)?;
            base.checked_pow(exponent).ok_or(Error::Overflow)
        }
        ("sqrt" | "sin" | "cos" | "tan", [_]) => Err(Error::Domain),
        _ => Err(invalid()),
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> &Token {
        &self.tokens[self.at]
    }
    fn take(&mut self) -> Token {
        let token = self.peek().clone();
        self.at += 1;
        token
    }

    fn conditional(&mut self) -> Result<Expression, Error> {
        let value = self.compare()?;
        if self.peek() == &Token::If {
            self.take();
            let condition = self.compare()?;
            if self.take() != Token::Else {
                return Err(Error::UnexpectedToken);
            }
            let alternative = self.conditional()?;
            Ok(Expression::Conditional {
                value: Box::new(value),
                condition: Box::new(condition),
                alternative: Box::new(alternative),
            })
        } else {
            Ok(value)
        }
    }

    fn compare(&mut self) -> Result<Expression, Error> {
        let mut value = self.sum()?;
        loop {
            let operation = self.peek().clone();
            if !matches!(
                operation,
                Token::Less
                    | Token::LessEq
                    | Token::Greater
                    | Token::GreaterEq
                    | Token::Eq
                    | Token::NotEq
            ) {
                break;
            }
            self.take();
            let rhs = self.sum()?;
            let operation = match operation {
                Token::Less => CompareOperation::Less,
                Token::LessEq => CompareOperation::LessEq,
                Token::Greater => CompareOperation::Greater,
                Token::GreaterEq => CompareOperation::GreaterEq,
                Token::Eq => CompareOperation::Equal,
                Token::NotEq => CompareOperation::NotEqual,
                _ => return Err(Error::UnexpectedToken),
            };
            value = Expression::Compare {
                operation,
                left: Box::new(value),
                right: Box::new(rhs),
            };
        }
        Ok(value)
    }

    fn sum(&mut self) -> Result<Expression, Error> {
        let mut value = self.product()?;
        loop {
            let operation = match self.peek() {
                Token::Plus => BinaryOperation::Add,
                Token::Minus => BinaryOperation::Subtract,
                _ => break,
            };
            self.take();
            value = Expression::Binary {
                operation,
                left: Box::new(value),
                right: Box::new(self.product()?),
            };
        }
        Ok(value)
    }

    fn product(&mut self) -> Result<Expression, Error> {
        let mut value = self.unary()?;
        loop {
            let operation = match self.peek() {
                Token::Star => BinaryOperation::Multiply,
                Token::Slash => BinaryOperation::Divide,
                Token::Percent => BinaryOperation::Remainder,
                _ => break,
            };
            self.take();
            value = Expression::Binary {
                operation,
                left: Box::new(value),
                right: Box::new(self.unary()?),
            };
        }
        Ok(value)
    }

    fn unary(&mut self) -> Result<Expression, Error> {
        match self.peek() {
            Token::Plus => {
                self.take();
                self.unary()
            }
            Token::Minus => {
                self.take();
                Ok(Expression::Unary {
                    negative: true,
                    value: Box::new(self.unary()?),
                })
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Expression, Error> {
        let value = self.primary()?;
        if self.peek() == &Token::Power {
            self.take();
            Ok(Expression::Binary {
                operation: BinaryOperation::Power,
                left: Box::new(value),
                right: Box::new(self.unary()?),
            })
        } else {
            Ok(value)
        }
    }

    fn primary(&mut self) -> Result<Expression, Error> {
        match self.take() {
            Token::Number(value) => Ok(Expression::Number(value)),
            Token::ExactNumber(value) => Ok(Expression::ExactNumber(value)),
            Token::LParen => {
                let value = self.conditional()?;
                if self.take() != Token::RParen {
                    return Err(Error::UnexpectedToken);
                }
                Ok(value)
            }
            Token::Name(name) if self.peek() == &Token::LParen => {
                self.take();
                let mut args = Vec::new();
                if self.peek() != &Token::RParen {
                    loop {
                        args.push(self.conditional()?);
                        if self.peek() != &Token::Comma {
                            break;
                        }
                        self.take();
                    }
                }
                if self.take() != Token::RParen {
                    return Err(Error::UnexpectedToken);
                }
                Ok(Expression::Call {
                    name,
                    arguments: args,
                })
            }
            Token::Name(name) => Ok(Expression::Name(name)),
            _ => Err(Error::UnexpectedToken),
        }
    }
}

fn function(name: &str, args: &[f64]) -> Result<f64, Error> {
    let invalid = || Error::InvalidArguments(name.into());
    let result = match (name, args) {
        ("min", [_first, ..]) => args.iter().copied().reduce(f64::min).ok_or_else(invalid)?,
        ("max", [_first, ..]) => args.iter().copied().reduce(f64::max).ok_or_else(invalid)?,
        ("round", [x]) => x.round_ties_even(),
        ("abs", [x]) => x.abs(),
        ("floor", [x]) => x.floor(),
        ("ceil", [x]) => x.ceil(),
        ("sqrt", [x]) => x.sqrt(),
        ("sin", [x]) => x.sin(),
        ("cos", [x]) => x.cos(),
        ("tan", [x]) => x.tan(),
        ("pow", [x, y]) => x.powf(*y),
        ("clamp", [x, lo, hi]) if lo <= hi => x.clamp(*lo, *hi),
        _ => return Err(invalid()),
    };
    finite(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_expressions_and_precedence() {
        let mut vars = HashMap::from([("x".into(), 7.0), ("c".into(), 0.0), ("y".into(), 3.0)]);
        assert_eq!(evaluate("min(4, 3)", &vars), Ok(3.0));
        assert_eq!(evaluate("round(1.7)", &vars), Ok(2.0));
        assert_eq!(evaluate("x if c else y", &vars), Ok(3.0));
        vars.insert("c".into(), 1.0);
        assert_eq!(evaluate("x if c else y", &vars), Ok(7.0));
        assert_eq!(evaluate("2*pi", &vars), Ok(2.0 * std::f64::consts::PI));
        assert_eq!(evaluate("-2**2", &vars), Ok(-4.0));
        assert_eq!(evaluate("2**3**2", &vars), Ok(512.0));
    }

    #[test]
    fn invalid_values_never_return_a_number() {
        let vars = HashMap::new();
        assert_eq!(evaluate("1/0", &vars), Err(Error::DivisionByZero));
        assert_eq!(evaluate("sqrt(-1)", &vars), Err(Error::Domain));
        assert_eq!(evaluate("1e308*10", &vars), Err(Error::Overflow));
        assert!(evaluate("hello", &vars).is_err());
        assert!(evaluate("1e", &vars).is_err());
    }

    #[test]
    fn conditional_branches_are_parsed_but_only_the_selected_branch_is_evaluated() {
        let vars = HashMap::new();
        assert_eq!(evaluate("1 if 1 else 1/0", &vars), Ok(1.0));
        assert_eq!(evaluate("1/0 if 0 else 2", &vars), Ok(2.0));
        assert_eq!(evaluate("sqrt(-1) if 0 else 7", &vars), Ok(7.0));
        assert_eq!(
            evaluate("1 if 0 else 1/0", &vars),
            Err(Error::DivisionByZero)
        );
    }

    #[test]
    fn excessive_expression_length_and_depth_fail_without_recursing() {
        let vars = HashMap::new();
        assert_eq!(evaluate(&"(".repeat(4_097), &vars), Err(Error::TooComplex));
        assert_eq!(evaluate(&"1+".repeat(300), &vars), Err(Error::TooComplex));
    }

    #[test]
    fn generated_integer_expressions_preserve_precedence_and_branch_selection() {
        for x in -32..=32 {
            for y in -32..=32 {
                let vars = HashMap::from([
                    ("x".to_owned(), f64::from(x)),
                    ("y".to_owned(), f64::from(y)),
                ]);
                assert_eq!(evaluate("x + 2*y", &vars), Ok(f64::from(x + 2 * y)));
                assert_eq!(evaluate("(x + y) * 2", &vars), Ok(f64::from((x + y) * 2)));
                assert_eq!(
                    evaluate("x if x > 0 else y", &vars),
                    Ok(f64::from(if x > 0 { x } else { y }))
                );
            }
        }
    }
}
