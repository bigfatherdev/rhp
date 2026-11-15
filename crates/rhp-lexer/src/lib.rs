//! Lexer escrito a mao para o RHP (Fase A: + `export`).
//!
//! Reconhece o subconjunto implementado: palavras-chave (`echo`, `let`,
//! `const`, `if`, `else`, `while`, `for`, `break`, `continue`, `try`,
//! `catch`, `finally`, `throw`, `export`, `fn`, `return`, `import`,
//! `true`, `false`, `null`), identificadores, literais (string, inteiro,
//! float), operadores e pontuacao. Comentarios de linha `//` sao ignorados.
//! Erros carregam linha/coluna.
//!
//! Notas: `in` (do `for x in ...`), `from`/`as` (do `import { } from`)
//! NAO sao palavras-chave: lidos como identificadores e reconhecidos
//! posicionalmente pelo parser.

use std::fmt;

/// Classificacao de um token.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Palavras-chave.
    Echo,
    Let,
    Const,
    If,
    Else,
    While,
    For,
    Break,
    Continue,
    Try,
    Catch,
    Finally,
    Throw,
    Export,
    Fn,
    Return,
    Import,
    True,
    False,
    Null,
    // Literais e identificador.
    Ident(String),
    IntLiteral(i64),
    FloatLiteral(f64),
    StringLiteral(String),
    // Operadores.
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    BitNot,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Question,
    QuestionQuestion,
    Assign,
    EqEq,
    BangEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    // Pontuacao.
    Semicolon,
    Comma,
    Colon,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
}

/// Um token com posicao de inicio (linha/coluna, base 1).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

impl Token {
    #[must_use]
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Self { kind, line, col }
    }
}

/// Erro lexico com posicao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl LexError {
    fn new(message: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            message: message.into(),
            line,
            col,
        }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for LexError {}

/// Transforma o fonte em tokens.
///
/// # Erros
///
/// Retorna [`LexError`] com linha/coluna ao encontrar caractere inesperado,
/// string nao terminada, escape invalido ou literal numerico invalido.
pub fn lex(input: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    let mut line: usize = 1;
    let mut col: usize = 1;

    while let Some((_, ch)) = chars.peek().copied() {
        match ch {
            ' ' | '\t' | '\r' => {
                chars.next();
                col += 1;
            }
            '\n' => {
                chars.next();
                line += 1;
                col = 1;
            }
            ';' => {
                tokens.push(Token::new(TokenKind::Semicolon, line, col));
                chars.next();
                col += 1;
            }
            ',' => {
                tokens.push(Token::new(TokenKind::Comma, line, col));
                chars.next();
                col += 1;
            }
            ':' => {
                tokens.push(Token::new(TokenKind::Colon, line, col));
                chars.next();
                col += 1;
            }
            '(' => {
                tokens.push(Token::new(TokenKind::LParen, line, col));
                chars.next();
                col += 1;
            }
            ')' => {
                tokens.push(Token::new(TokenKind::RParen, line, col));
                chars.next();
                col += 1;
            }
            '{' => {
                tokens.push(Token::new(TokenKind::LBrace, line, col));
                chars.next();
                col += 1;
            }
            '}' => {
                tokens.push(Token::new(TokenKind::RBrace, line, col));
                chars.next();
                col += 1;
            }
            '[' => {
                tokens.push(Token::new(TokenKind::LBracket, line, col));
                chars.next();
                col += 1;
            }
            ']' => {
                tokens.push(Token::new(TokenKind::RBracket, line, col));
                chars.next();
                col += 1;
            }
            '+' => {
                tokens.push(Token::new(TokenKind::Plus, line, col));
                chars.next();
                col += 1;
            }
            '-' => {
                tokens.push(Token::new(TokenKind::Minus, line, col));
                chars.next();
                col += 1;
            }
            '*' => {
                tokens.push(Token::new(TokenKind::Star, line, col));
                chars.next();
                col += 1;
            }
            '/' => {
                chars.next();
                col += 1;
                // Comentario de linha `//`: ignora ate o fim da linha.
                if chars.peek().copied().is_some_and(|(_, c)| c == '/') {
                    for (_, c) in chars.by_ref() {
                        col += 1;
                        if c == '\n' {
                            line += 1;
                            col = 1;
                            break;
                        }
                    }
                } else {
                    tokens.push(Token::new(TokenKind::Slash, line, col - 1));
                }
            }
            '%' => {
                tokens.push(Token::new(TokenKind::Percent, line, col));
                chars.next();
                col += 1;
            }
            '&' => {
                tokens.push(Token::new(TokenKind::BitAnd, line, col));
                chars.next();
                col += 1;
            }
            '|' => {
                tokens.push(Token::new(TokenKind::BitOr, line, col));
                chars.next();
                col += 1;
            }
            '^' => {
                tokens.push(Token::new(TokenKind::BitXor, line, col));
                chars.next();
                col += 1;
            }
            '~' => {
                tokens.push(Token::new(TokenKind::BitNot, line, col));
                chars.next();
                col += 1;
            }
            '?' => {
                let start_col = col;
                chars.next();
                col += 1;
                if chars.peek().copied().is_some_and(|(_, c)| c == '?') {
                    chars.next();
                    col += 1;
                    tokens.push(Token::new(TokenKind::QuestionQuestion, line, start_col));
                } else {
                    tokens.push(Token::new(TokenKind::Question, line, start_col));
                }
            }
            '=' => {
                let start_col = col;
                chars.next();
                col += 1;
                if chars.peek().copied().is_some_and(|(_, c)| c == '=') {
                    chars.next();
                    col += 1;
                    tokens.push(Token::new(TokenKind::EqEq, line, start_col));
                } else {
                    tokens.push(Token::new(TokenKind::Assign, line, start_col));
                }
            }
            '!' => {
                let start_col = col;
                chars.next();
                col += 1;
                if chars.peek().copied().is_some_and(|(_, c)| c == '=') {
                    chars.next();
                    col += 1;
                    tokens.push(Token::new(TokenKind::BangEq, line, start_col));
                } else {
                    tokens.push(Token::new(TokenKind::Bang, line, start_col));
                }
            }
            '<' => {
                let start_col = col;
                chars.next();
                col += 1;
                match chars.peek().copied() {
                    Some((_, '=')) => {
                        chars.next();
                        col += 1;
                        tokens.push(Token::new(TokenKind::LtEq, line, start_col));
                    }
                    Some((_, '<')) => {
                        chars.next();
                        col += 1;
                        tokens.push(Token::new(TokenKind::Shl, line, start_col));
                    }
                    _ => tokens.push(Token::new(TokenKind::Lt, line, start_col)),
                }
            }
            '>' => {
                let start_col = col;
                chars.next();
                col += 1;
                match chars.peek().copied() {
                    Some((_, '=')) => {
                        chars.next();
                        col += 1;
                        tokens.push(Token::new(TokenKind::GtEq, line, start_col));
                    }
                    Some((_, '>')) => {
                        chars.next();
                        col += 1;
                        tokens.push(Token::new(TokenKind::Shr, line, start_col));
                    }
                    _ => tokens.push(Token::new(TokenKind::Gt, line, start_col)),
                }
            }
            '"' => {
                tokens.push(lex_string(&mut chars, &mut line, &mut col)?);
            }
            c if c.is_ascii_digit() => {
                tokens.push(lex_number(&mut chars, &mut line, &mut col)?);
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                tokens.push(lex_ident_or_keyword(&mut chars, &mut line, &mut col));
            }
            other => {
                return Err(LexError::new(
                    format!("caractere inesperado: {other}"),
                    line,
                    col,
                ));
            }
        }
    }

    Ok(tokens)
}

fn lex_string(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    line: &mut usize,
    col: &mut usize,
) -> Result<Token, LexError> {
    let start_line = *line;
    let start_col = *col;
    // Consome a aspa de abertura.
    chars.next();
    *col += 1;
    let mut value = String::new();
    let mut closed = false;
    // `while let` e proposital aqui: o corpo tambem consome via `chars.next()`.
    #[allow(clippy::while_let_on_iterator)]
    while let Some((_, c)) = chars.next() {
        match c {
            '"' => {
                *col += 1;
                closed = true;
                break;
            }
            '\\' => {
                // Escape: precisa de um caractere seguinte.
                *col += 1;
                match chars.next() {
                    Some((_, 'n')) => {
                        value.push('\n');
                        *col += 1;
                    }
                    Some((_, 'r')) => {
                        value.push('\r');
                        *col += 1;
                    }
                    Some((_, 't')) => {
                        value.push('\t');
                        *col += 1;
                    }
                    Some((_, '\\')) => {
                        value.push('\\');
                        *col += 1;
                    }
                    Some((_, '"')) => {
                        value.push('"');
                        *col += 1;
                    }
                    Some((_, 'u')) => {
                        *col += 1;
                        value.push(lex_unicode_escape(chars, line, col, start_line, start_col)?);
                    }
                    Some((_, other)) => {
                        return Err(LexError::new(
                            format!("escape invalido: \\{other}"),
                            *line,
                            *col,
                        ));
                    }
                    None => {
                        return Err(LexError::new("string nao terminada", start_line, start_col));
                    }
                }
            }
            '\n' => {
                return Err(LexError::new(
                    "quebra de linha sem escape dentro de string",
                    *line,
                    *col,
                ));
            }
            other => {
                value.push(other);
                *col += 1;
            }
        }
    }
    if !closed {
        return Err(LexError::new("string nao terminada", start_line, start_col));
    }
    Ok(Token::new(
        TokenKind::StringLiteral(value),
        start_line,
        start_col,
    ))
}

/// Le `\u{HEX}` apos o `u` ja consumido (1–6 digitos, ponto valido).
fn lex_unicode_escape(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    line: &mut usize,
    col: &mut usize,
    start_line: usize,
    start_col: usize,
) -> Result<char, LexError> {
    const HINT: &str = "escape unicode invalido (use \\u{HEX}, 1-6 digitos)";
    match chars.next() {
        Some((_, '{')) => {
            *col += 1;
        }
        _ => {
            return Err(LexError::new(
                "escape invalido: \\u (use \\u{HEX})",
                *line,
                *col,
            ));
        }
    }
    let mut hex = String::new();
    let mut closed = false;
    for (_, c) in chars.by_ref() {
        *col += 1;
        if c == '}' {
            closed = true;
            break;
        }
        if c.is_ascii_hexdigit() && hex.len() < 6 {
            hex.push(c);
        } else {
            return Err(LexError::new(HINT, *line, *col));
        }
    }
    if !closed || hex.is_empty() {
        return Err(LexError::new(HINT, start_line, start_col));
    }
    let code =
        u32::from_str_radix(&hex, 16).map_err(|_| LexError::new(HINT, start_line, start_col))?;
    char::from_u32(code).ok_or_else(|| {
        LexError::new(
            "escape unicode invalido (ponto de codigo fora do intervalo)",
            start_line,
            start_col,
        )
    })
}

fn lex_number(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    line: &mut usize,
    col: &mut usize,
) -> Result<Token, LexError> {
    let start_line = *line;
    let start_col = *col;
    let mut text = String::new();
    while let Some((_, c)) = chars.peek().copied() {
        if c.is_ascii_digit() {
            text.push(c);
            chars.next();
            *col += 1;
        } else {
            break;
        }
    }
    // Parte fracionaria opcional: `.` seguido de ao menos um digito.
    let mut is_float = false;
    // Espia `.` + digito sem consumir ainda.
    let mut probe = chars.clone();
    if matches!(probe.next(), Some((_, '.')))
        && probe
            .peek()
            .copied()
            .is_some_and(|(_, c)| c.is_ascii_digit())
    {
        is_float = true;
        chars.next(); // consome `.`
        *col += 1;
        text.push('.');
        while let Some((_, c)) = chars.peek().copied() {
            if c.is_ascii_digit() {
                text.push(c);
                chars.next();
                *col += 1;
            } else {
                break;
            }
        }
    }
    if is_float {
        text.parse::<f64>().map_or_else(
            |_| {
                Err(LexError::new(
                    format!("literal float invalido: {text}"),
                    start_line,
                    start_col,
                ))
            },
            |v| {
                Ok(Token::new(
                    TokenKind::FloatLiteral(v),
                    start_line,
                    start_col,
                ))
            },
        )
    } else {
        text.parse::<i64>().map_or_else(
            |_| {
                Err(LexError::new(
                    format!("literal inteiro fora do intervalo: {text}"),
                    start_line,
                    start_col,
                ))
            },
            |v| Ok(Token::new(TokenKind::IntLiteral(v), start_line, start_col)),
        )
    }
}

fn lex_ident_or_keyword(
    chars: &mut std::iter::Peekable<std::str::CharIndices<'_>>,
    line: &mut usize,
    col: &mut usize,
) -> Token {
    let start_line = *line;
    let start_col = *col;
    let mut ident = String::new();
    while let Some((_, c)) = chars.peek().copied() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ident.push(c);
            chars.next();
            *col += 1;
        } else {
            break;
        }
    }
    let kind = match ident.as_str() {
        "echo" => TokenKind::Echo,
        "let" => TokenKind::Let,
        "const" => TokenKind::Const,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "while" => TokenKind::While,
        "for" => TokenKind::For,
        "try" => TokenKind::Try,
        "catch" => TokenKind::Catch,
        "finally" => TokenKind::Finally,
        "throw" => TokenKind::Throw,
        "export" => TokenKind::Export,
        "fn" => TokenKind::Fn,
        "return" => TokenKind::Return,
        "import" => TokenKind::Import,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "null" => TokenKind::Null,
        _ => TokenKind::Ident(ident),
    };
    Token::new(kind, start_line, start_col)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src)
            .expect("lex deve passar")
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn lexes_echo_hello() {
        let tokens = lex("echo \"Hello from RHP!\";").expect("lex deve passar");
        assert_eq!(
            tokens,
            vec![
                Token::new(TokenKind::Echo, 1, 1),
                Token::new(
                    TokenKind::StringLiteral("Hello from RHP!".to_string()),
                    1,
                    6
                ),
                Token::new(TokenKind::Semicolon, 1, 23),
            ]
        );
    }

    #[test]
    fn lexes_empty_program() {
        let tokens = lex("").expect("programa vazio deve lexar");
        assert!(tokens.is_empty());
    }

    #[test]
    fn reports_unterminated_string() {
        let err = lex("echo \"abc").expect_err("deve falhar");
        assert_eq!(err.message, "string nao terminada");
        assert_eq!((err.line, err.col), (1, 6));
    }

    #[test]
    fn reports_invalid_escape() {
        let err = lex("echo \"a\\qb\";").expect_err("escape invalido deve falhar");
        assert!(err.message.contains("escape invalido"));
    }

    #[test]
    fn resolves_unicode_escapes() {
        let tokens = lex("echo \"A\\u{41}\\u{1F600}\";").expect("lex deve passar");
        assert_eq!(
            tokens[1],
            Token::new(TokenKind::StringLiteral("AA\u{1F600}".to_string()), 1, 6)
        );
    }

    #[test]
    fn rejects_bad_unicode_escapes() {
        for src in [
            "echo \"\\u\";",
            "echo \"\\u{}\";",
            "echo \"\\u{110000}\";",
            "echo \"\\u{D800}\";",
            "echo \"\\u{41\";",
            "echo \"\\u{ZZ}\";",
        ] {
            assert!(lex(src).is_err(), "deve falhar: {src}");
        }
    }

    #[test]
    fn resolves_escapes() {
        let tokens = lex("echo \"a\\n\\\"b\\\\\";").expect("lex deve passar");
        assert_eq!(
            tokens[1],
            Token::new(TokenKind::StringLiteral("a\n\"b\\".to_string()), 1, 6)
        );
    }

    #[test]
    fn reports_unexpected_character() {
        let err = lex("echo @;").expect_err("deve falhar");
        assert_eq!((err.line, err.col), (1, 6));
        assert!(err.message.contains("caractere inesperado"));
    }

    #[test]
    fn lexes_identifiers_and_keywords() {
        assert_eq!(
            kinds("let if else while for fn return import echo"),
            vec![
                TokenKind::Let,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::While,
                TokenKind::For,
                TokenKind::Fn,
                TokenKind::Return,
                TokenKind::Import,
                TokenKind::Echo,
            ]
        );
        // `try`/`catch`/`finally`/`throw`/`export` (Fase A).
        assert_eq!(
            kinds("try catch finally throw export"),
            vec![
                TokenKind::Try,
                TokenKind::Catch,
                TokenKind::Finally,
                TokenKind::Throw,
                TokenKind::Export,
            ]
        );
        // `in` e contextual (do `for x in ...`): continua identificador.
        assert_eq!(kinds("in"), vec![TokenKind::Ident("in".to_string())]);
        assert_eq!(
            kinds("x _y z9"),
            vec![
                TokenKind::Ident("x".to_string()),
                TokenKind::Ident("_y".to_string()),
                TokenKind::Ident("z9".to_string()),
            ]
        );
        assert_eq!(
            kinds("true false null"),
            vec![TokenKind::True, TokenKind::False, TokenKind::Null]
        );
    }

    #[test]
    fn lexes_numbers() {
        assert_eq!(kinds("42"), vec![TokenKind::IntLiteral(42)]);
        assert_eq!(kinds("0"), vec![TokenKind::IntLiteral(0)]);
        assert_eq!(kinds("1.5"), vec![TokenKind::FloatLiteral(1.5)]);
        assert_eq!(kinds("0.25"), vec![TokenKind::FloatLiteral(0.25)]);
    }

    #[test]
    fn reports_integer_overflow() {
        let err = lex("99999999999999999999999").expect_err("deve falhar");
        assert!(err.message.contains("fora do intervalo"));
    }

    #[test]
    fn lexes_operators() {
        assert_eq!(
            kinds("+ - * / % ! = == != < <= > >="),
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Percent,
                TokenKind::Bang,
                TokenKind::Assign,
                TokenKind::EqEq,
                TokenKind::BangEq,
                TokenKind::Lt,
                TokenKind::LtEq,
                TokenKind::Gt,
                TokenKind::GtEq,
            ]
        );
        assert_eq!(
            kinds("; , : ( ) { } [ ]"),
            vec![
                TokenKind::Semicolon,
                TokenKind::Comma,
                TokenKind::Colon,
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBrace,
                TokenKind::RBrace,
                TokenKind::LBracket,
                TokenKind::RBracket,
            ]
        );
    }

    #[test]
    fn lexes_bitwise_and_ternary_operators() {
        assert_eq!(
            kinds("& | ^ ~ << >> ? ??"),
            vec![
                TokenKind::BitAnd,
                TokenKind::BitOr,
                TokenKind::BitXor,
                TokenKind::BitNot,
                TokenKind::Shl,
                TokenKind::Shr,
                TokenKind::Question,
                TokenKind::QuestionQuestion,
            ]
        );
        assert_eq!(
            kinds("break continue const"),
            vec![TokenKind::Break, TokenKind::Continue, TokenKind::Const]
        );
    }

    #[test]
    fn ignores_line_comments() {
        assert_eq!(
            kinds("// comentario\necho 1; // fim"),
            vec![
                TokenKind::Echo,
                TokenKind::IntLiteral(1),
                TokenKind::Semicolon,
            ]
        );
    }

    #[test]
    fn tracks_line_and_col() {
        let tokens = lex("echo \"a\";\necho \"b\";").expect("lex deve passar");
        assert_eq!((tokens[3].line, tokens[3].col), (2, 1));
        assert_eq!((tokens[4].line, tokens[4].col), (2, 6));
    }

    #[test]
    fn malformed_inputs_never_panic() {
        // Corpus de entradas malformadas: lexer nunca entra em panico,
        // so retorna Ok(tokens) ou Err(posicionado).
        let inputs = [
            "",
            "\"",
            "\"abc",
            "echo \"a\\",
            "echo \"a\\x\"",
            "echo \"a\\u\"",
            "echo \"a\\u{\"",
            "echo \"a\\u{ZZ}\"",
            "echo 99999999999999999999999999",
            "echo @#$",
            "'",
            "\"a\nb\"",
            "====",
            "!!!!",
            "[[[[",
            "let ",
            "echo 1+",
            "//",
            "/",
            "/=",
            "\"\\u{110000}\"",
        ];
        for src in inputs {
            let _ = lex(src);
        }
    }
}
