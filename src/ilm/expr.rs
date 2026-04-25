use std::collections::BTreeMap;
use std::iter::Peekable;

#[derive(Debug, Clone, Copy)]
pub enum Token<'a> {
    Number(f64),
    Unit(&'a str), // "%", "px", "pt", "em"
    Var(&'a str),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn next_token(&mut self) -> Option<Token<'a>> {
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len() && bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
        if self.pos >= bytes.len() {
            return None;
        }

        let c = bytes[self.pos];
        match c {
            b'+' => {
                self.pos += 1;
                Some(Token::Plus)
            }
            b'-' => {
                self.pos += 1;
                Some(Token::Minus)
            }
            b'*' => {
                self.pos += 1;
                Some(Token::Star)
            }
            b'/' => {
                self.pos += 1;
                Some(Token::Slash)
            }
            b'(' => {
                self.pos += 1;
                Some(Token::LParen)
            }
            b')' => {
                self.pos += 1;
                Some(Token::RParen)
            }
            b'$' => {
                self.pos += 1;
                let start = self.pos;
                while self.pos < bytes.len()
                    && (bytes[self.pos].is_ascii_alphanumeric() || bytes[self.pos] == b'_')
                {
                    self.pos += 1;
                }
                Some(Token::Var(&self.input[start..self.pos]))
            }
            b'%' => {
                self.pos += 1;
                Some(Token::Unit("%"))
            }
            _ if c.is_ascii_digit() || c == b'.' => {
                let start = self.pos;
                while self.pos < bytes.len()
                    && (bytes[self.pos].is_ascii_digit() || bytes[self.pos] == b'.')
                {
                    self.pos += 1;
                }
                let s = &self.input[start..self.pos];
                let num = s.parse::<f64>().unwrap_or(0.0);
                Some(Token::Number(num))
            }
            b'p' | b'e' => {
                // px, pt, em
                let start = self.pos;
                while self.pos < bytes.len() && bytes[self.pos].is_ascii_alphabetic() {
                    self.pos += 1;
                }
                Some(Token::Unit(&self.input[start..self.pos]))
            }
            _ => {
                self.pos += 1;
                self.next_token() // Skip unknown
            }
        }
    }
}

pub struct EvalContext<'a> {
    pub vars: &'a BTreeMap<String, f64>,
    pub reference_length: f64, // for %
    pub font_size_pt: f64,     // for em
}

pub fn evaluate(expr: &str, ctx: &EvalContext) -> f64 {
    let mut lexer = Lexer::new(expr);
    let mut tokens = Vec::new();
    let mut has_units = false;
    while let Some(t) = lexer.next_token() {
        if matches!(t, Token::Unit(_)) {
            has_units = true;
        }
        tokens.push(t);
    }

    let mut it = tokens.into_iter().peekable();
    let val = parse_expr(&mut it, ctx);
    
    if !has_units {
        val * ctx.reference_length
    } else {
        val
    }
}

fn parse_expr<'a, I: Iterator<Item = Token<'a>>>(it: &mut Peekable<I>, ctx: &EvalContext) -> f64 {
    let mut val = parse_term(it, ctx);
    while let Some(&token) = it.peek() {
        match token {
            Token::Plus => {
                it.next();
                val += parse_term(it, ctx);
            }
            Token::Minus => {
                it.next();
                val -= parse_term(it, ctx);
            }
            _ => break,
        }
    }
    val
}

fn parse_term<'a, I: Iterator<Item = Token<'a>>>(it: &mut Peekable<I>, ctx: &EvalContext) -> f64 {
    let mut val = parse_factor(it, ctx);
    while let Some(&token) = it.peek() {
        match token {
            Token::Star => {
                it.next();
                val *= parse_factor(it, ctx);
            }
            Token::Slash => {
                it.next();
                let divisor = parse_factor(it, ctx);
                if divisor != 0.0 {
                    val /= divisor;
                }
            }
            _ => break,
        }
    }
    val
}

fn parse_factor<'a, I: Iterator<Item = Token<'a>>>(it: &mut Peekable<I>, ctx: &EvalContext) -> f64 {
    if let Some(token) = it.next() {
        match token {
            Token::Number(n) => {
                // Check if next token is a unit
                let mut multiplier = 1.0;
                let mut apply_unit = |unit: &str| match unit {
                    "%" => multiplier = ctx.reference_length / 100.0,
                    "px" => multiplier = 9525.0,
                    "pt" => multiplier = 12700.0,
                    "em" => multiplier = ctx.font_size_pt * 12700.0,
                    _ => {}
                };

                if let Some(&Token::Unit(u)) = it.peek() {
                    apply_unit(u);
                    it.next();
                } else {
                    multiplier = 1.0;
                }
                n * multiplier
            }
            Token::Var(name) => *ctx.vars.get(name).unwrap_or(&0.0),
            Token::LParen => {
                let val = parse_expr(it, ctx);
                if let Some(Token::RParen) = it.peek() {
                    it.next();
                }
                val
            }
            Token::Minus => -parse_factor(it, ctx),
            _ => 0.0,
        }
    } else {
        0.0
    }
}
