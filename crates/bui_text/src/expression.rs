//! Numeric expressions for editable values. Evaluation never mutates a control.

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

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
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

fn lex(source: &str) -> Result<Vec<Token>, Error> {
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
                let n = source[start..i]
                    .parse()
                    .map_err(|_| Error::InvalidNumber(start))?;
                out.push(Token::Number(n));
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

pub fn evaluate(source: &str, vars: &HashMap<String, f64>) -> Result<f64, Error> {
    if source.len() > 4_096 {
        return Err(Error::TooComplex);
    }
    let tokens = lex(source)?;
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
    expression.evaluate(vars)
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
    fn evaluate(&self, vars: &HashMap<String, f64>) -> Result<f64, Error> {
        match self {
            Self::Number(value) => finite(*value),
            Self::Name(name) => match name.as_str() {
                "pi" => Ok(std::f64::consts::PI),
                "e" => Ok(std::f64::consts::E),
                _ => vars
                    .get(name)
                    .copied()
                    .ok_or_else(|| Error::UnknownName(name.clone()))
                    .and_then(finite),
            },
            Self::Unary { negative, value } => {
                let value = value.evaluate(vars)?;
                finite(if *negative { -value } else { value })
            }
            Self::Binary {
                operation,
                left,
                right,
            } => {
                let left = left.evaluate(vars)?;
                let right = right.evaluate(vars)?;
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
                let left = left.evaluate(vars)?;
                let right = right.evaluate(vars)?;
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
                if condition.evaluate(vars)? != 0.0 {
                    value.evaluate(vars)
                } else {
                    alternative.evaluate(vars)
                }
            }
            Self::Call { name, arguments } => {
                let values = arguments
                    .iter()
                    .map(|argument| argument.evaluate(vars))
                    .collect::<Result<Vec<_>, _>>()?;
                function(name, &values)
            }
        }
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
