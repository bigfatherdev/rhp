//! Parser escrito a mao + AST para o RHP (Fase 3).
//!
//! Gramatica implementada (Marco 2 + compostos da Fase 3):
//!
//! ```text
//! Program    := Stmt*
//! Block      := "{" Stmt* "}"
//! Stmt       := LetStmt | ConstStmt | AssignStmt | EchoStmt | IfStmt
//!             | WhileStmt | ForStmt | BreakStmt | ContinueStmt | FnDef
//!             | ReturnStmt | Block | ImportStmt | ExprStmt
//! LetStmt    := "let" Ident ("=" Expr)? ";"
//! ConstStmt  := "const" Ident "=" Expr ";"
//! AssignStmt := Target "=" Expr ";"
//! Target     := Ident | Ident "[" Expr "]"
//! EchoStmt   := "echo" Expr ";"
//! IfStmt     := "if" Expr Block ("else" Block)?
//! WhileStmt  := "while" Expr Block
//! ForStmt    := "for" Ident "in" Expr Block
//! BreakStmt  := "break" ";"
//! ContinueStmt := "continue" ";"
//! FnDef      := "fn" Ident "(" (Ident ("," Ident)*)? ")" Block
//! ReturnStmt := "return" Expr? ";"
//! ImportStmt := "import" String ";"
//! ExprStmt   := Expr ";"
//! Expr       := Ternary
//! Ternary    := NullCoalesce ("?" Ternary ":" Ternary)?
//! NullCoalesce := Equality ("??" NullCoalesce)?
//! Equality   := BitOr (("==" | "!=") BitOr)*
//! BitOr      := BitXor (("|") BitXor)*
//! BitXor     := BitAnd ("^" BitAnd)*
//! BitAnd     := Comparison (("&") Comparison)*
//! Comparison := Shift (("<" | "<=" | ">" | ">=") Shift)*
//! Shift      := Addition (("<<" | ">>") Addition)*
//! Addition   := Multiplication (("+" | "-") Multiplication)*
//! Multiplication := Unary (("*" | "/" | "%") Unary)*
//! Unary      := ("!" | "-" | "~" | "+") Unary | Postfix
//! Postfix    := Primary ("[" Expr "]")*
//! Primary    := Int | Float | String | "true" | "false" | "null"
//!             | Array | Map | Ident | Call | "(" Expr ")"
//! Array      := "[" (Expr ("," Expr)*)? "]"
//! Map        := "{" ((String | Ident) ":" Expr ("," ...)*)? "}"
//! Call       := Ident "(" (Expr ("," Expr)*)? ")"
//! ```

use rhp_lexer::{Token, TokenKind};
use std::fmt;

/// Programa RHP: sequencia de instrucoes.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

/// Instrucao do RHP.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Echo {
        expr: Expr,
    },
    Let {
        name: String,
        init: Option<Expr>,
    },
    /// `const NOME = expr;` — como `let`, mas imutavel (atribuicao posterior e erro de compilacao).
    Const {
        name: String,
        init: Expr,
    },
    Break,
    Continue,
    Assign {
        target: AssignTarget,
        expr: Expr,
    },
    If {
        cond: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    /// `for VAR in ITER BLOCO` — itera elementos de array ou chaves
    /// ordenadas de mapa. `ITER` e avaliado uma vez (snapshot por valor);
    /// `VAR` e declarada a cada iteracao no escopo do corpo.
    /// `in` e contextual (identificador posicional, nao palavra-chave).
    For {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
    },
    FnDef {
        name: String,
        params: Vec<String>,
        body: Vec<Stmt>,
    },
    Return {
        expr: Option<Expr>,
    },
    /// `import "caminho.rhp";` — resolvido pelo carregador de modulos
    /// (a compilacao direta ignora; use o loader da CLI).
    Import(String),
    Block(Vec<Stmt>),
    ExprStmt {
        expr: Expr,
    },
}

/// Alvo de atribuicao: variavel ou elemento `nome[indice]` (um nivel).
#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Var(String),
    Index { name: String, index: Expr },
}

/// Expressao do RHP.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    StringLiteral(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
    Var(String),
    Array(Vec<Expr>),
    Map(Vec<(String, Expr)>),
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Ternary {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
}

/// Operador binario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    NullCoalesce,
}

/// Operador unario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}

/// Erro sintatico com posicao.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl ParseError {
    fn new(message: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            message: message.into(),
            line,
            col,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

struct Cursor<'a> {
    tokens: &'a [Token],
    pos: usize,
    depth: u32,
}

/// Profundidade sintatica maxima (protecao contra estouro de pilha com
/// entradas patologicas; ver AUD-001). Cadeias planas (`1+1+...`, instrucoes
/// em sequencia) nao acumulam profundidade. Calibrado para caber com folga
/// na pilha de 2 MiB das threads de teste (~9 frames Rust por nivel).
pub const MAX_PARSE_DEPTH: u32 = 128;

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            depth: 0,
        }
    }

    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a Token> {
        let tok = self.tokens.get(self.pos)?;
        self.pos += 1;
        Some(tok)
    }

    fn end_pos(&self) -> (usize, usize) {
        self.peek().map_or_else(
            || {
                // Posicao de EOF: apos o ultimo token, ou 1:1 se vazio.
                self.tokens.last().map_or((1, 1), |t| {
                    let width = token_width(t);
                    (t.line, t.col + width)
                })
            },
            |t| (t.line, t.col),
        )
    }

    /// Executa `f` um nivel sintatico abaixo, recusando alem do limite.
    fn nest<T>(
        &mut self,
        what: &str,
        f: impl FnOnce(&mut Self) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        if self.depth >= MAX_PARSE_DEPTH {
            let (line, col) = self.end_pos();
            return Err(ParseError::new(
                format!("profundidade maxima excedida em {what} (limite {MAX_PARSE_DEPTH})"),
                line,
                col,
            ));
        }
        self.depth += 1;
        let result = f(&mut *self);
        self.depth -= 1;
        result
    }
}

fn token_width(token: &Token) -> usize {
    match &token.kind {
        TokenKind::Echo
        | TokenKind::Let
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::While
        | TokenKind::For
        | TokenKind::Fn
        | TokenKind::Import
        | TokenKind::Null
        | TokenKind::True
        | TokenKind::False => 0, // largura exata irrelevante para EOF apos palavra-chave
        TokenKind::Return => 6,
        TokenKind::Ident(s) => s.len(),
        TokenKind::StringLiteral(s) => s.len() + 2,
        TokenKind::IntLiteral(v) => v.to_string().len(),
        TokenKind::FloatLiteral(v) => v.to_string().len(),
        TokenKind::EqEq
        | TokenKind::BangEq
        | TokenKind::LtEq
        | TokenKind::GtEq
        | TokenKind::Shl
        | TokenKind::Shr
        | TokenKind::QuestionQuestion => 2,
        _ => 1,
    }
}

/// Analisa uma lista de tokens e produz a AST.
///
/// # Erros
///
/// Retorna [`ParseError`] com linha/coluna ao encontrar token inesperado,
/// expressao ausente, delimitador ausente ou bloco nao fechado.
pub fn parse(tokens: &[Token]) -> Result<Program, ParseError> {
    let mut cursor = Cursor::new(tokens);
    let mut statements = Vec::new();

    while cursor.peek().is_some() {
        statements.push(parse_stmt(&mut cursor)?);
    }

    Ok(Program { statements })
}

fn parse_stmt(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.nest("instrucao", parse_stmt_inner)
}

fn parse_stmt_inner(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    let tok = cursor.peek().cloned().ok_or_else(|| {
        let (line, col) = cursor.end_pos();
        ParseError::new("esperado instrucao, encontrado fim de arquivo", line, col)
    })?;
    match &tok.kind {
        TokenKind::Echo => parse_echo(cursor),
        TokenKind::Let => parse_let(cursor),
        TokenKind::Const => parse_const(cursor),
        TokenKind::Break => {
            cursor.next();
            expect_semicolon(cursor)?;
            Ok(Stmt::Break)
        }
        TokenKind::Continue => {
            cursor.next();
            expect_semicolon(cursor)?;
            Ok(Stmt::Continue)
        }
        TokenKind::If => parse_if(cursor),
        TokenKind::While => parse_while(cursor),
        TokenKind::For => parse_for(cursor),
        TokenKind::Fn => parse_fn_def(cursor),
        TokenKind::Return => parse_return(cursor),
        TokenKind::Import => parse_import(cursor),
        TokenKind::LBrace => Ok(Stmt::Block(parse_block(cursor)?)),
        _ => parse_assign_or_expr_stmt(cursor),
    }
}

fn parse_echo(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let expr = parse_expr(cursor)?;
    expect_semicolon(cursor)?;
    Ok(Stmt::Echo { expr })
}

fn parse_const(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = expect_ident(cursor)?;
    expect_token(
        cursor,
        &TokenKind::Assign,
        "esperado `=` apos nome da constante",
    )?;
    let init = parse_expr(cursor)?;
    expect_semicolon(cursor)?;
    Ok(Stmt::Const { name, init })
}

fn parse_let(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = expect_ident(cursor)?;
    let init = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
        cursor.next();
        Some(parse_expr(cursor)?)
    } else {
        None
    };
    expect_semicolon(cursor)?;
    Ok(Stmt::Let { name, init })
}

/// `alvo = expr;` ou instrucao por expressao.
///
/// O alvao valido e `nome` ou `nome[indice]` (um nivel); outras formas com
/// `=` apos expressao geram "alvo de atribuicao invalido".
fn parse_assign_or_expr_stmt(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    let expr = parse_expr(cursor)?;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
        let target = match expr {
            Expr::Var(name) => AssignTarget::Var(name),
            Expr::Index { base, index } => match *base {
                Expr::Var(name) => AssignTarget::Index {
                    name,
                    index: *index,
                },
                _ => {
                    let (line, col) = cursor.end_pos();
                    return Err(ParseError::new(
                        "alvo de atribuicao invalido (use `nome` ou `nome[indice]`)",
                        line,
                        col,
                    ));
                }
            },
            _ => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "alvo de atribuicao invalido (use `nome` ou `nome[indice]`)",
                    line,
                    col,
                ));
            }
        };
        cursor.next(); // consome `=`
        let value = parse_expr(cursor)?;
        expect_semicolon(cursor)?;
        Ok(Stmt::Assign {
            target,
            expr: value,
        })
    } else {
        expect_semicolon(cursor)?;
        Ok(Stmt::ExprStmt { expr })
    }
}

fn parse_if(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let cond = parse_expr(cursor)?;
    let then_branch = parse_block(cursor)?;
    let else_branch = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Else)) {
        cursor.next();
        Some(parse_block(cursor)?)
    } else {
        None
    };
    Ok(Stmt::If {
        cond,
        then_branch,
        else_branch,
    })
}

fn parse_while(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let cond = parse_expr(cursor)?;
    let body = parse_block(cursor)?;
    Ok(Stmt::While { cond, body })
}

fn parse_for(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let var = expect_ident(cursor)?;
    // `in` e contextual: aceita apenas o identificador `in` nesta posicao,
    // sem reservar a palavra no resto da linguagem.
    match cursor.next() {
        Some(t) => match &t.kind {
            TokenKind::Ident(name) if name == "in" => {}
            other => {
                return Err(ParseError::new(
                    format!(
                        "esperado `in` apos variavel do `for`, encontrado {}",
                        describe(other)
                    ),
                    t.line,
                    t.col,
                ));
            }
        },
        None => {
            let (line, col) = cursor.end_pos();
            return Err(ParseError::new(
                "esperado `in` apos variavel do `for`, encontrado fim de arquivo",
                line,
                col,
            ));
        }
    }
    let iter = parse_expr(cursor)?;
    let body = parse_block(cursor)?;
    Ok(Stmt::For { var, iter, body })
}

fn parse_fn_def(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = expect_ident(cursor)?;
    expect_token(
        cursor,
        &TokenKind::LParen,
        "esperado `(` apos nome da funcao",
    )?;
    let mut params = Vec::new();
    if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
        loop {
            params.push(expect_ident(cursor)?);
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                cursor.next();
            } else {
                break;
            }
        }
    }
    expect_token(cursor, &TokenKind::RParen, "esperado `)` apos parametros")?;
    let body = parse_block(cursor)?;
    Ok(Stmt::FnDef { name, params, body })
}

fn parse_return(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let expr = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Semicolon)) {
        None
    } else {
        Some(parse_expr(cursor)?)
    };
    expect_semicolon(cursor)?;
    Ok(Stmt::Return { expr })
}

fn parse_import(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let path = match cursor.next() {
        Some(t) => match &t.kind {
            TokenKind::StringLiteral(s) => s.clone(),
            other => {
                return Err(ParseError::new(
                    format!(
                        "esperado caminho de modulo (string), encontrado {}",
                        describe(other)
                    ),
                    t.line,
                    t.col,
                ));
            }
        },
        None => {
            let (line, col) = cursor.end_pos();
            return Err(ParseError::new(
                "esperado caminho de modulo, encontrado fim de arquivo",
                line,
                col,
            ));
        }
    };
    expect_semicolon(cursor)?;
    Ok(Stmt::Import(path))
}

fn parse_block(cursor: &mut Cursor<'_>) -> Result<Vec<Stmt>, ParseError> {
    expect_token(cursor, &TokenKind::LBrace, "esperado `{`")?;
    let mut statements = Vec::new();
    loop {
        match cursor.peek().map(|t| &t.kind) {
            None => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado `}`, encontrado fim de arquivo",
                    line,
                    col,
                ));
            }
            Some(TokenKind::RBrace) => {
                cursor.next();
                return Ok(statements);
            }
            Some(_) => statements.push(parse_stmt(cursor)?),
        }
    }
}

fn parse_expr(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    cursor.nest("expressao", parse_ternary)
}

fn parse_ternary(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let cond = parse_null_coalesce(cursor)?;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Question)) {
        cursor.next();
        let then_branch = parse_ternary(cursor)?;
        expect_token(cursor, &TokenKind::Colon, "esperado `:` no ternario")?;
        let else_branch = parse_ternary(cursor)?;
        return Ok(Expr::Ternary {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        });
    }
    Ok(cond)
}

fn parse_null_coalesce(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let left = parse_equality(cursor)?;
    if matches!(
        cursor.peek().map(|t| &t.kind),
        Some(TokenKind::QuestionQuestion)
    ) {
        cursor.next();
        // Associativo a direita: `a ?? b ?? c` = `a ?? (b ?? c)`.
        let right = parse_null_coalesce(cursor)?;
        return Ok(Expr::Binary {
            op: BinOp::NullCoalesce,
            left: Box::new(left),
            right: Box::new(right),
        });
    }
    Ok(left)
}

fn parse_equality(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_bitor(cursor)?;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::EqEq) => BinOp::Eq,
            Some(TokenKind::BangEq) => BinOp::Ne,
            _ => break,
        };
        cursor.next();
        let right = parse_bitor(cursor)?;
        expr = Expr::Binary {
            op,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_bitor(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_bitxor(cursor)?;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::BitOr)) {
        cursor.next();
        let right = parse_bitxor(cursor)?;
        expr = Expr::Binary {
            op: BinOp::BitOr,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_bitxor(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_bitand(cursor)?;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::BitXor)) {
        cursor.next();
        let right = parse_bitand(cursor)?;
        expr = Expr::Binary {
            op: BinOp::BitXor,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_bitand(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_comparison(cursor)?;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::BitAnd)) {
        cursor.next();
        let right = parse_comparison(cursor)?;
        expr = Expr::Binary {
            op: BinOp::BitAnd,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_comparison(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_shift(cursor)?;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Lt) => BinOp::Lt,
            Some(TokenKind::LtEq) => BinOp::Le,
            Some(TokenKind::Gt) => BinOp::Gt,
            Some(TokenKind::GtEq) => BinOp::Ge,
            _ => break,
        };
        cursor.next();
        let right = parse_shift(cursor)?;
        expr = Expr::Binary {
            op,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_shift(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_addition(cursor)?;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Shl) => BinOp::Shl,
            Some(TokenKind::Shr) => BinOp::Shr,
            _ => break,
        };
        cursor.next();
        let right = parse_addition(cursor)?;
        expr = Expr::Binary {
            op,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_addition(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_multiplication(cursor)?;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Plus) => BinOp::Add,
            Some(TokenKind::Minus) => BinOp::Sub,
            _ => break,
        };
        cursor.next();
        let right = parse_multiplication(cursor)?;
        expr = Expr::Binary {
            op,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_multiplication(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_unary(cursor)?;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Star) => BinOp::Mul,
            Some(TokenKind::Slash) => BinOp::Div,
            Some(TokenKind::Percent) => BinOp::Mod,
            _ => break,
        };
        cursor.next();
        let right = parse_unary(cursor)?;
        expr = Expr::Binary {
            op,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_unary(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    match cursor.peek().map(|t| &t.kind) {
        Some(TokenKind::Bang) => {
            cursor.next();
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(parse_unary(cursor)?),
            })
        }
        Some(TokenKind::Minus) => {
            cursor.next();
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(parse_unary(cursor)?),
            })
        }
        Some(TokenKind::BitNot) => {
            cursor.next();
            Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(parse_unary(cursor)?),
            })
        }
        // `+x` e identidade (compativel; sem no dedicado na AST).
        Some(TokenKind::Plus) => {
            cursor.next();
            parse_unary(cursor)
        }
        _ => parse_postfix(cursor),
    }
}

fn parse_postfix(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_primary(cursor)?;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LBracket)) {
        cursor.next();
        let index = parse_expr(cursor)?;
        expect_token(cursor, &TokenKind::RBracket, "esperado `]`")?;
        expr = Expr::Index {
            base: Box::new(expr),
            index: Box::new(index),
        };
    }
    Ok(expr)
}

fn parse_primary(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let tok = cursor.next().cloned().ok_or_else(|| {
        let (line, col) = cursor.end_pos();
        ParseError::new("esperado expressao, encontrado fim de arquivo", line, col)
    })?;
    match &tok.kind {
        TokenKind::StringLiteral(s) => Ok(Expr::StringLiteral(s.clone())),
        TokenKind::IntLiteral(v) => Ok(Expr::Int(*v)),
        TokenKind::FloatLiteral(v) => Ok(Expr::Float(*v)),
        TokenKind::True => Ok(Expr::Bool(true)),
        TokenKind::False => Ok(Expr::Bool(false)),
        TokenKind::Null => Ok(Expr::Null),
        TokenKind::LBracket => parse_array(cursor),
        TokenKind::LBrace => parse_map(cursor),
        TokenKind::Ident(name) => {
            let name = name.clone();
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
                cursor.next();
                let mut args = Vec::new();
                if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
                    loop {
                        args.push(parse_expr(cursor)?);
                        if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                            cursor.next();
                        } else {
                            break;
                        }
                    }
                }
                expect_token(cursor, &TokenKind::RParen, "esperado `)` apos argumentos")?;
                Ok(Expr::Call { name, args })
            } else {
                Ok(Expr::Var(name))
            }
        }
        TokenKind::LParen => {
            let expr = parse_expr(cursor)?;
            expect_token(cursor, &TokenKind::RParen, "esperado `)`")?;
            Ok(expr)
        }
        other => Err(ParseError::new(
            format!("esperado expressao, encontrado {}", describe(other)),
            tok.line,
            tok.col,
        )),
    }
}

fn parse_array(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut items = Vec::new();
    if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
        loop {
            items.push(parse_expr(cursor)?);
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                cursor.next();
            } else {
                break;
            }
        }
    }
    expect_token(cursor, &TokenKind::RBracket, "esperado `]`")?;
    Ok(Expr::Array(items))
}

fn parse_map(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut entries = Vec::new();
    if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RBrace)) {
        loop {
            let key = match cursor.next() {
                Some(t) => match &t.kind {
                    TokenKind::StringLiteral(s) => s.clone(),
                    TokenKind::Ident(name) => name.clone(),
                    other => {
                        return Err(ParseError::new(
                            format!(
                                "esperado chave de mapa (string ou identificador), encontrado {}",
                                describe(other)
                            ),
                            t.line,
                            t.col,
                        ));
                    }
                },
                None => {
                    let (line, col) = cursor.end_pos();
                    return Err(ParseError::new(
                        "esperado chave de mapa, encontrado fim de arquivo",
                        line,
                        col,
                    ));
                }
            };
            expect_token(cursor, &TokenKind::Colon, "esperado `:` apos chave do mapa")?;
            let value = parse_expr(cursor)?;
            entries.push((key, value));
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                cursor.next();
            } else {
                break;
            }
        }
    }
    expect_token(cursor, &TokenKind::RBrace, "esperado `}`")?;
    Ok(Expr::Map(entries))
}

fn expect_ident(cursor: &mut Cursor<'_>) -> Result<String, ParseError> {
    match cursor.next() {
        Some(t) => match &t.kind {
            TokenKind::Ident(name) => Ok(name.clone()),
            other => Err(ParseError::new(
                format!("esperado identificador, encontrado {}", describe(other)),
                t.line,
                t.col,
            )),
        },
        None => {
            let (line, col) = cursor.end_pos();
            Err(ParseError::new(
                "esperado identificador, encontrado fim de arquivo",
                line,
                col,
            ))
        }
    }
}

fn expect_token(
    cursor: &mut Cursor<'_>,
    expected: &TokenKind,
    message: &str,
) -> Result<(), ParseError> {
    match cursor.next() {
        Some(t) if &t.kind == expected => Ok(()),
        Some(t) => Err(ParseError::new(
            format!("{message}, encontrado {}", describe(&t.kind)),
            t.line,
            t.col,
        )),
        None => {
            let (line, col) = cursor.end_pos();
            Err(ParseError::new(
                format!("{message}, encontrado fim de arquivo"),
                line,
                col,
            ))
        }
    }
}

fn expect_semicolon(cursor: &mut Cursor<'_>) -> Result<(), ParseError> {
    expect_token(cursor, &TokenKind::Semicolon, "esperado `;`")
}

fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Echo => "`echo`".to_string(),
        TokenKind::Let => "`let`".to_string(),
        TokenKind::If => "`if`".to_string(),
        TokenKind::Else => "`else`".to_string(),
        TokenKind::While => "`while`".to_string(),
        TokenKind::For => "`for`".to_string(),
        TokenKind::Fn => "`fn`".to_string(),
        TokenKind::Const => "`const`".to_string(),
        TokenKind::Break => "`break`".to_string(),
        TokenKind::Continue => "`continue`".to_string(),
        TokenKind::Return => "`return`".to_string(),
        TokenKind::Import => "`import`".to_string(),
        TokenKind::True | TokenKind::False => "booleano".to_string(),
        TokenKind::Null => "`null`".to_string(),
        TokenKind::Ident(_) => "identificador".to_string(),
        TokenKind::IntLiteral(_) | TokenKind::FloatLiteral(_) => "numero".to_string(),
        TokenKind::StringLiteral(_) => "literal de string".to_string(),
        TokenKind::Semicolon => "`;`".to_string(),
        TokenKind::Comma => "`,`".to_string(),
        TokenKind::Colon => "`:`".to_string(),
        TokenKind::LParen => "`(`".to_string(),
        TokenKind::RParen => "`)`".to_string(),
        TokenKind::LBrace => "`{`".to_string(),
        TokenKind::RBrace => "`}`".to_string(),
        TokenKind::LBracket => "`[`".to_string(),
        TokenKind::RBracket => "`]`".to_string(),
        TokenKind::Plus => "`+`".to_string(),
        TokenKind::Minus => "`-`".to_string(),
        TokenKind::Star => "`*`".to_string(),
        TokenKind::Slash => "`/`".to_string(),
        TokenKind::BitAnd => "`&`".to_string(),
        TokenKind::BitOr => "`|`".to_string(),
        TokenKind::BitXor => "`^`".to_string(),
        TokenKind::BitNot => "`~`".to_string(),
        TokenKind::Shl => "`<<`".to_string(),
        TokenKind::Shr => "`>>`".to_string(),
        TokenKind::Question => "`?`".to_string(),
        TokenKind::QuestionQuestion => "`??`".to_string(),
        TokenKind::Percent => "`%`".to_string(),
        TokenKind::Bang => "`!`".to_string(),
        TokenKind::Assign => "`=`".to_string(),
        TokenKind::EqEq => "`==`".to_string(),
        TokenKind::BangEq => "`!=`".to_string(),
        TokenKind::Lt => "`<`".to_string(),
        TokenKind::LtEq => "`<=`".to_string(),
        TokenKind::Gt => "`>`".to_string(),
        TokenKind::GtEq => "`>=`".to_string(),
    }
}

/// Conveniencia: lex + parse em um passo (usado pela CLI e testes).
///
/// # Erros
///
/// Retorna string de erro ja formatada com posicao.
pub fn parse_source(source: &str) -> Result<Program, String> {
    let tokens = rhp_lexer::lex(source).map_err(|e| format!("erro lexico em {e}"))?;
    parse(&tokens).map_err(|e| format!("erro sintatico em {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_and_parse(src: &str) -> Result<Program, ParseError> {
        let tokens = rhp_lexer::lex(src).expect("lex deve passar");
        parse(&tokens)
    }

    #[test]
    fn parses_echo_hello() {
        let program = lex_and_parse("echo \"Hello from RHP!\";").expect("parse deve passar");
        assert_eq!(
            program,
            Program {
                statements: vec![Stmt::Echo {
                    expr: Expr::StringLiteral("Hello from RHP!".to_string()),
                }],
            }
        );
    }

    #[test]
    fn parses_empty_program() {
        let program = lex_and_parse("").expect("programa vazio deve passar");
        assert!(program.statements.is_empty());
    }

    #[test]
    fn parses_multiple_echoes() {
        let program = lex_and_parse("echo \"a\"; echo \"b\";").expect("parse deve passar");
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn parses_precedence_mul_over_add() {
        // `1 + 2 * 3` => Add(1, Mul(2, 3))
        let program = lex_and_parse("echo 1 + 2 * 3;").expect("parse deve passar");
        assert_eq!(
            program.statements[0],
            Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::Add,
                    left: Box::new(Expr::Int(1)),
                    right: Box::new(Expr::Binary {
                        op: BinOp::Mul,
                        left: Box::new(Expr::Int(2)),
                        right: Box::new(Expr::Int(3)),
                    }),
                }
            }
        );
    }

    #[test]
    fn parses_parens_and_unary() {
        let program = lex_and_parse("echo -(1 + 2) * -3;").expect("parse deve passar");
        assert!(matches!(program.statements[0], Stmt::Echo { expr: _ }));
    }

    #[test]
    fn parses_let_assign_and_vars() {
        let program = lex_and_parse("let x = 1; x = x + 2; echo x;").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![
                Stmt::Let {
                    name: "x".to_string(),
                    init: Some(Expr::Int(1)),
                },
                Stmt::Assign {
                    target: AssignTarget::Var("x".to_string()),
                    expr: Expr::Binary {
                        op: BinOp::Add,
                        left: Box::new(Expr::Var("x".to_string())),
                        right: Box::new(Expr::Int(2)),
                    },
                },
                Stmt::Echo {
                    expr: Expr::Var("x".to_string()),
                },
            ]
        );
    }

    #[test]
    fn parses_let_without_init() {
        let program = lex_and_parse("let x;").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::Let {
                name: "x".to_string(),
                init: None
            }]
        );
    }

    #[test]
    fn parses_index_assign() {
        let program = lex_and_parse("a[0] = 1;").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::Assign {
                target: AssignTarget::Index {
                    name: "a".to_string(),
                    index: Expr::Int(0),
                },
                expr: Expr::Int(1),
            }]
        );
    }

    #[test]
    fn parses_array_map_and_index() {
        let program = lex_and_parse("echo [1, 2][0]; echo {a: 1, \"b\": 2}[\"b\"];")
            .expect("parse deve passar");
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn rejects_invalid_assign_target() {
        let err = lex_and_parse("f() = 1;").expect_err("deve falhar");
        assert!(err.message.contains("alvo de atribuicao"));
    }

    #[test]
    fn parses_if_else_and_while() {
        let program = lex_and_parse(
            "if x > 1 { echo \"a\"; } else { echo \"b\"; } while x < 10 { x = x + 1; }",
        )
        .expect("parse deve passar");
        assert_eq!(program.statements.len(), 2);
        assert!(matches!(
            program.statements[0],
            Stmt::If {
                else_branch: Some(_),
                ..
            }
        ));
        assert!(matches!(program.statements[1], Stmt::While { .. }));
    }

    #[test]
    fn parses_for_in_loop() {
        let program = lex_and_parse("for x in [1, 2] { echo x; }").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::For {
                var: "x".to_string(),
                iter: Expr::Array(vec![Expr::Int(1), Expr::Int(2)]),
                body: vec![Stmt::Echo {
                    expr: Expr::Var("x".to_string()),
                }],
            }]
        );
    }

    #[test]
    fn keeps_in_as_plain_identifier_elsewhere() {
        // `in` so tem sentido apos `for VAR`; no resto, e identificador comum.
        let program = lex_and_parse("let in = 1; echo in;").expect("parse deve passar");
        assert!(matches!(program.statements[0], Stmt::Let { .. }));
    }

    #[test]
    fn rejects_for_without_in() {
        let err = lex_and_parse("for x [1, 2] { echo x; }").expect_err("deve falhar");
        assert!(
            err.message.contains("esperado `in`"),
            "obteve: {}",
            err.message
        );
    }

    #[test]
    fn rejects_for_without_block() {
        let err = lex_and_parse("for x in [1];").expect_err("deve falhar");
        assert!(
            err.message.contains("esperado `{`"),
            "obteve: {}",
            err.message
        );
    }

    #[test]
    fn parses_fn_def_and_call() {
        let program = lex_and_parse("fn soma(a, b) { return a + b; } echo soma(1, 2);")
            .expect("parse deve passar");
        assert_eq!(program.statements.len(), 2);
        assert!(matches!(program.statements[0], Stmt::FnDef { .. }));
        assert!(matches!(program.statements[1], Stmt::Echo { .. }));
    }

    #[test]
    fn reports_missing_semicolon() {
        let tokens = rhp_lexer::lex("echo \"x\"").expect("lex deve passar");
        let err = parse(&tokens).expect_err("`;` ausente deve falhar");
        assert!(err.message.contains("esperado `;`"));
    }

    #[test]
    fn reports_missing_expression() {
        let tokens = rhp_lexer::lex("echo ;").expect("lex deve passar");
        let err = parse(&tokens).expect_err("expressao ausente deve falhar");
        assert!(err.message.contains("esperado expressao"));
    }

    #[test]
    fn reports_unexpected_token_at_start() {
        let tokens = rhp_lexer::lex(";").expect("lex deve passar");
        let err = parse(&tokens).expect_err("deve falhar");
        assert!(err.message.contains("esperado expressao"));
    }

    #[test]
    fn reports_unclosed_block() {
        let err = lex_and_parse("if true { echo 1;").expect_err("deve falhar");
        assert!(err.message.contains("esperado `}`"));
    }

    #[test]
    fn reports_keyword_as_name() {
        let err = lex_and_parse("let echo = 1;").expect_err("deve falhar");
        assert!(err.message.contains("esperado identificador"));
    }

    #[test]
    fn parses_unary_plus_as_identity() {
        let program = lex_and_parse("echo +5;").expect("parse deve passar");
        assert_eq!(program.statements, vec![Stmt::Echo { expr: Expr::Int(5) }]);
    }

    #[test]
    fn parses_bitwise_with_precedence() {
        // `1 | 2 & 3` => BitOr(1, BitAnd(2, 3)); `1 + 2 << 3` => Shl(Add(1,2),3)
        let program = lex_and_parse("echo 1 | 2 & 3;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::BitOr,
                    ..
                }
            }
        ));
        let program = lex_and_parse("echo 1 + 2 << 3;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Echo {
                expr: Expr::Binary { op: BinOp::Shl, .. }
            }
        ));
    }

    #[test]
    fn parses_ternary_and_null_coalescing() {
        let program = lex_and_parse("echo true ? 1 : 2;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Echo {
                expr: Expr::Ternary { .. }
            }
        ));
        let program = lex_and_parse("echo a ?? b;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::NullCoalesce,
                    ..
                }
            }
        ));
    }

    #[test]
    fn parses_break_continue_and_const() {
        let program = lex_and_parse("while true { break; } while true { continue; } const N = 10;")
            .expect("parse deve passar");
        assert_eq!(program.statements.len(), 3);
        assert!(matches!(program.statements[0], Stmt::While { .. }));
        assert!(matches!(program.statements[2], Stmt::Const { .. }));
    }

    #[test]
    fn parses_import() {
        let program = lex_and_parse("import \"lib.rhp\"; echo 1;").expect("parse deve passar");
        assert_eq!(program.statements[0], Stmt::Import("lib.rhp".to_string()));
    }

    #[test]
    fn rejects_import_without_string() {
        let err = lex_and_parse("import 42;").expect_err("deve falhar");
        assert!(err.message.contains("caminho de modulo"));
    }

    #[test]
    fn parse_source_helper_reports_errors() {
        let err = parse_source("echo \"x\"").expect_err("deve falhar");
        assert!(err.contains("esperado `;`"));
    }

    #[test]
    fn malformed_inputs_never_panic() {
        // Tokenizacao valida + parse: nunca panico, so Ok ou Err.
        let inputs = [
            ";",
            "echo",
            "echo ;",
            "let",
            "let = 1;",
            "let x =",
            "if",
            "if true",
            "if true {",
            "{",
            "}",
            "fn",
            "fn f(",
            "fn f(a,",
            "return return",
            "import",
            "import 42",
            "[1, 2",
            "{a: 1",
            "{1: 2}",
            "f(1,",
            "(1",
            "a[0",
            "== 1;",
            "echo 1 +;",
            "\"unterminated",
            "let x = [;",
            "while while",
        ];
        for src in inputs {
            if let Ok(tokens) = rhp_lexer::lex(src) {
                let _ = parse(&tokens);
            }
        }
    }

    #[test]
    fn rejects_excessive_expression_nesting() {
        let src = format!("echo {}1{};", "(".repeat(400), ")".repeat(400));
        let err = lex_and_parse(&src).expect_err("aninhamento excessivo deve falhar");
        assert!(err.message.contains("profundidade maxima"));
    }

    #[test]
    fn rejects_excessive_block_nesting() {
        let src = format!("{}echo 1;{}", "if true {".repeat(400), "}".repeat(400));
        let err = lex_and_parse(&src).expect_err("aninhamento excessivo deve falhar");
        assert!(err.message.contains("profundidade maxima"));
    }

    #[test]
    fn accepts_nesting_within_limit() {
        let src = format!("echo {}1{};", "(".repeat(100), ")".repeat(100));
        lex_and_parse(&src).expect("100 niveis devem passar");
    }
}
