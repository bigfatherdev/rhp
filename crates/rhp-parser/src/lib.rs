//! Parser escrito a mao + AST para o RHP (Fase 3).
//!
//! Gramatica implementada (Marco 2 + compostos da Fase 3):
//!
//! ```text
//! Program    := Stmt*
//! Block      := "{" Stmt* "}"
//! Stmt       := LetStmt | ConstStmt | DestructureStmt | AssignStmt | EchoStmt
//!             | IfStmt | WhileStmt | ForStmt | ForCStmt | SwitchStmt | TryStmt
//!             | ThrowStmt | BreakStmt | ContinueStmt | FnDef | ReturnStmt
//!             | Block | ImportStmt | ImportNames | Export | ExprStmt
//! LetStmt    := "let" Ident ("=" Expr)? ";"
//! ConstStmt  := "const" Ident "=" Expr ";"
//! AssignStmt := Target "=" Expr ";"
//! Target     := Ident | Ident "[" Expr "]"
//! EchoStmt   := "echo" Expr ";"
//! IfStmt     := "if" Expr Block ("else" Block)?
//! WhileStmt  := "while" Expr Block
//! ForStmt    := "for" Ident "in" Expr Block
//! ForCStmt   := "for" Stmt? ";" Expr? ";" (Assign | Expr)? Block
//!             // secoes vazias ok (`for (;;)` = infinito)
//! SwitchStmt := "switch" Expr "{" (CaseClause | DefaultClause)* "}"
//! CaseClause := "case" Expr Block  // `default` unico, posicao livre
//! DefaultClause := "default" Block
//! TryStmt    := "try" Block ("catch" Ident? Block)? ("finally" Block)?
//!             // ao menos um de `catch`/`finally` e obrigatorio
//! ThrowStmt  := "throw" Expr ";"
//! BreakStmt  := "break" Int? ";"     // N >= 1, padrao 1 (sai de N breakables)
//! ContinueStmt := "continue" Int? ";" // N >= 1, padrao 1 (mira o N-esimo laco)
//! FnDef      := "fn" Ident "(" (Ident ("," Ident)*)? ")" Block
//! ReturnStmt := "return" Expr? ";"
//! ImportStmt := "import" String ";"
//!             | "import" "{" Ident ("as" Ident)? ("," ...)* "}" "from" String ";"
//! ExportStmt := "export" (FnDef | LetStmt | ConstStmt)
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
//! Postfix    := Primary (("[" Expr "]") | ("(" Args ")"))*
//! Primary    := Int | Float | String | "true" | "false" | "null"
//!             | Array | Map | Ident | Closure | Match | "(" Expr ")"
//! Closure    := "fn" "(" (Ident ("," Ident)*)? ")" Block
//! Match      := "match" Expr "{" (MatchArm ",")* (MatchArm ","?)? "}"
//! MatchArm   := (Expr | "default") "=>" Expr
//! Array      := "[" (Expr ("," Expr)*)? "]"
//! Map        := "{" ((String | Ident) ":" Expr ("," ...)*)? "}"
//! Call       := Ident "(" (Expr ("," Expr)*)? ")"   // callee direto
//! CallValue  := Postfix "(" (Expr ("," Expr)*)? ")" // callee arbitrario
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
    /// `let [a, b] = expr;` / `let {x} = expr;` / `let {k: v} = expr;`
    /// (e as formas `const`, que congelam todos; RFC 0007). RHS avaliado
    /// uma vez; faltas viram erro do `Index` vigente (sem `null`).
    Destructure {
        target: DestructureTarget,
        init: Expr,
        is_const: bool,
    },
    /// `const NOME = expr;` — como `let`, mas imutavel (atribuicao posterior e erro de compilacao).
    Const {
        name: String,
        init: Expr,
    },
    /// `static nome = expr;` dentro de funcao (estado por definicao, RFC 0010).
    Static {
        name: String,
        init: Expr,
    },
    Break,
    Continue,
    /// `break N;` / `continue N;` (N >= 1, literal; ausente = 1; RFC 0008).
    /// `break N` sai dos N breakables mais internos (laco ou `switch`);
    /// `continue N` mira o N-esimo laco externo. Alem do aninhamento = erro.
    BreakN(u32),
    ContinueN(u32),
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
    /// `for init; cond; passo { corpo }` (partes opcionais; RFC 0008).
    /// `init`/`passo`: qualquer statement (`None` = ausente);
    /// `cond`: `None` = `true`. Compilacao propria: `continue` salta ao passo.
    ForC {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Vec<Stmt>,
    },
    /// `switch EXPR { (case EXPR { ... } | default { ... })* }` (RFC 0004).
    /// Comparacao frouxa `==`, fallthrough, `default` unico em qualquer
    /// posicao (`cond: None`). `break` sai do `switch`; `continue` mira o
    /// laco externo (checado na compilacao).
    Switch {
        discriminant: Expr,
        cases: Vec<SwitchCase>,
    },
    /// `try BLOCO (catch VAR? BLOCO)? (finally BLOCO)?` (RFC 0001).
    /// Ao menos um de `catch`/`finally` e obrigatorio (checado no parse).
    /// `catch` presente com corpo vazio (`catch e { }`) consome o erro.
    Try {
        body: Vec<Stmt>,
        catch_var: Option<String>,
        catch_body: Option<Vec<Stmt>>,
        finally_body: Option<Vec<Stmt>>,
    },
    /// `throw EXPR;` — lanca o valor como erro (RFC 0001).
    Throw {
        expr: Expr,
    },
    FnDef {
        name: String,
        params: Vec<Param>,
        /// `...rest` (no maximo um, por ultimo; RFC 0006).
        variadic: Option<String>,
        body: Vec<Stmt>,
    },
    Return {
        expr: Option<Expr>,
    },
    /// `import "caminho.rhp";` — resolvido pelo carregador de modulos
    /// (a compilacao direta ignora; use o loader da CLI).
    Import(String),
    /// `import { a, b as c } from "caminho.rhp";` — importa só nomes
    /// exportados (privacidade real, ADR 0009). `from`/`as` sao
    /// contextuais (identificadores posicionais, como o `in` do `for`).
    ImportNames {
        path: String,
        names: Vec<ImportName>,
    },
    /// `export { a, b as c } from "caminho.rhp";` — re-export seletivo
    /// (RFC 0011; nomes passam aos exports deste modulo).
    ReExport {
        path: String,
        names: Vec<ImportName>,
    },
    /// `export fn/let/const` (ADR 0009, opt-in; sem `export` = privado).
    /// O linker extrai o nome e remove o marcador; compilar um `Export`
    /// direto da erro (defesa em profundidade).
    Export {
        def: Box<Stmt>,
    },
    Block(Vec<Stmt>),
    ExprStmt {
        expr: Expr,
    },
}

/// Nome selecionado em `import { } from`: original + alias opcional.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportName {
    pub name: String,
    pub alias: Option<String>,
}

/// Parametro de funcao/closure: nome + padrao opcional (constante, RFC 0006).
/// O `...rest` (variadico, por ultimo) vai em `variadic`, nao aqui.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub default: Option<Expr>,
}

/// Ramo de `switch`: `cond: None` e o `default` (unico, posicao livre).
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub cond: Option<Expr>,
    pub body: Vec<Stmt>,
}

/// Alvo de atribuicao: variavel ou elemento `nome[indice]` (um nivel).
#[derive(Debug, Clone, PartialEq)]
pub enum AssignTarget {
    Var(String),
    Index { name: String, index: Expr },
}

/// Argumento de chamada (RFC 0009): posicional, nomeado (`x: v`) ou
/// spread (`...arr`, splice em ordem). `Expr::Call`/`CallValue` guardam
/// `Vec<CallArg>`; o caso so-posicional compila no `Call` classico.
#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Pos(Expr),
    Named(String, Expr),
    Spread(Expr),
}

impl CallArg {
    /// Expressao do argumento (qualquer forma).
    pub fn value(&self) -> &Expr {
        match self {
            CallArg::Pos(expr) | CallArg::Named(_, expr) | CallArg::Spread(expr) => expr,
        }
    }
}

/// Padrao de destructuring (raso; RFC 0007): por posicao `[a, b]` ou por
/// chave `{x}` (atalho p/ `("x","x")`) / `{k: v}` (`(chave, nome)`).
/// Aninhados sao erro de parse.
#[derive(Debug, Clone, PartialEq)]
pub enum DestructureTarget {
    Positional(Vec<String>),
    ByKey(Vec<(String, String)>),
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
        args: Vec<CallArg>,
    },
    /// Chamada de callee arbitrario: `f()(1)`, `(fn(x) { x })(2)`,
    /// `arr[0](x)`. `f(x)` direto continua `Call` (precedencia de builtins
    /// preservada byte a byte). `Box` no callee para nao inflar `sizeof(Expr)`
    /// (o guarda de profundidade assume frames enxutos; ver MAX_PARSE_DEPTH).
    CallValue {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    /// `fn (params) { corpo }` como valor (closure, RFC 0003).
    /// Sem nome: recursao usa o `let` externo (ligacao tardia).
    /// `Box` proposital: mantem `Expr` no tamanho anterior a RFC 0003.
    Closure(Box<ClosureData>),
    /// `match sujeito { c1 => v1, default => v2 }` (expressao, RFC 0008).
    /// Igualdade estrita; sem fallthrough; primeiro match vence; `default`
    /// unico (qualquer posicao); sem match e sem `default` = erro de runtime.
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
    },
}

/// Braco de `match`: `cond: None` e o `default` (RFC 0008).
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub cond: Option<Expr>,
    pub value: Expr,
}

/// Corpo de closure, sempre heap-alocado (ver `Expr::Closure`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureData {
    pub params: Vec<Param>,
    /// `...rest` (no maximo um, por ultimo; RFC 0006).
    pub variadic: Option<String>,
    pub body: Vec<Stmt>,
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

    /// Lookahead de N tokens (para desambiguar `for x in` vs `for init;`).
    fn peek_at(&self, offset: usize) -> Option<&'a Token> {
        self.tokens.get(self.pos + offset)
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
        | TokenKind::Try
        | TokenKind::Catch
        | TokenKind::Finally
        | TokenKind::Throw
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
        TokenKind::Static => parse_static(cursor),
        TokenKind::Break => {
            cursor.next();
            Ok(parse_break_level(cursor, true)?)
        }
        TokenKind::Continue => {
            cursor.next();
            Ok(parse_break_level(cursor, false)?)
        }
        TokenKind::If => parse_if(cursor),
        TokenKind::While => parse_while(cursor),
        TokenKind::For => parse_for(cursor),
        TokenKind::Switch => parse_switch(cursor),
        TokenKind::Try => parse_try(cursor),
        TokenKind::Throw => parse_throw(cursor),
        TokenKind::Fn => parse_fn_def(cursor),
        TokenKind::Return => parse_return(cursor),
        TokenKind::Import => parse_import(cursor),
        TokenKind::Export => parse_export(cursor),
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

/// Nivel de `break [N];` / `continue [N];` (RFC 0008): literal `>= 1`;
/// ausente = 1 (`Break`/`Continue` simples). N alem do aninhamento e erro
/// de compilacao (o parser so valida a forma).
fn parse_break_level(cursor: &mut Cursor<'_>, is_break: bool) -> Result<Stmt, ParseError> {
    let level = match cursor.peek().map(|t| &t.kind) {
        Some(TokenKind::IntLiteral(n)) => {
            let tok = cursor.next().expect("peek garantiu inteiro");
            let n = *n;
            if n < 1 || n > u32::MAX as i64 {
                return Err(ParseError::new(
                    "nivel de `break`/`continue` deve ser inteiro >= 1".to_string(),
                    tok.line,
                    tok.col,
                ));
            }
            n as u32
        }
        _ => 1,
    };
    expect_semicolon(cursor)?;
    Ok(if is_break {
        if level == 1 {
            Stmt::Break
        } else {
            Stmt::BreakN(level)
        }
    } else if level == 1 {
        Stmt::Continue
    } else {
        Stmt::ContinueN(level)
    })
}

fn parse_const(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    // `const [a, b] = ...` / `const {x} = ...` (congela todos; RFC 0007).
    if matches!(
        cursor.peek().map(|t| &t.kind),
        Some(TokenKind::LBracket | TokenKind::LBrace)
    ) {
        let target = parse_destructure_target(cursor)?;
        expect_token(cursor, &TokenKind::Assign, "esperado `=` apos padrao")?;
        let init = parse_expr(cursor)?;
        expect_semicolon(cursor)?;
        return Ok(Stmt::Destructure {
            target,
            init,
            is_const: true,
        });
    }
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

/// `static nome = expr;` (RFC 0010): estado por definicao de funcao.
/// Init obrigatorio; `static` fora de funcao e erro de compilacao.
fn parse_static(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = expect_ident(cursor)?;
    expect_token(
        cursor,
        &TokenKind::Assign,
        "esperado `=` apos nome da estatica (init obrigatorio)",
    )?;
    let init = parse_expr(cursor)?;
    expect_semicolon(cursor)?;
    Ok(Stmt::Static { name, init })
}

fn parse_let(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    // `let [a, b] = ...` / `let {x} = ...` / `let {k: v} = ...` (RFC 0007).
    if matches!(
        cursor.peek().map(|t| &t.kind),
        Some(TokenKind::LBracket | TokenKind::LBrace)
    ) {
        let target = parse_destructure_target(cursor)?;
        expect_token(cursor, &TokenKind::Assign, "esperado `=` apos padrao")?;
        let init = parse_expr(cursor)?;
        expect_semicolon(cursor)?;
        return Ok(Stmt::Destructure {
            target,
            init,
            is_const: false,
        });
    }
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

/// Padrao de destructuring apos `let`/`const` (cursor no `[` ou `{`).
fn parse_destructure_target(cursor: &mut Cursor<'_>) -> Result<DestructureTarget, ParseError> {
    match cursor.next().map(|t| t.kind.clone()) {
        Some(TokenKind::LBracket) => {
            let mut names = Vec::new();
            if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RBracket)) {
                loop {
                    names.push(expect_ident(cursor)?);
                    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                        cursor.next();
                        continue;
                    } else {
                        break;
                    }
                }
            }
            expect_token(cursor, &TokenKind::RBracket, "esperado `]` apos padrao")?;
            if names.is_empty() {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "padrao `[]` vazio (declare ao menos um nome)".to_string(),
                    line,
                    col,
                ));
            }
            Ok(DestructureTarget::Positional(names))
        }
        Some(TokenKind::LBrace) => {
            let mut pairs = Vec::new();
            if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RBrace)) {
                loop {
                    let key = expect_ident(cursor)?;
                    // `{k: v}` renomeia; `{x}` e atalho para `("x", "x")`.
                    let name = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Colon)) {
                        cursor.next();
                        expect_ident(cursor)?
                    } else {
                        key.clone()
                    };
                    pairs.push((key, name));
                    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                        cursor.next();
                        continue;
                    } else {
                        break;
                    }
                }
            }
            expect_token(cursor, &TokenKind::RBrace, "esperado `}` apos padrao")?;
            if pairs.is_empty() {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "padrao `{}` vazio (declare ao menos um nome)".to_string(),
                    line,
                    col,
                ));
            }
            Ok(DestructureTarget::ByKey(pairs))
        }
        _ => {
            let (line, col) = cursor.end_pos();
            Err(ParseError::new(
                "esperado `[` ou `{` apos `let`/`const` para destructuring".to_string(),
                line,
                col,
            ))
        }
    }
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
    cursor.next(); // consome `for`
                   // `for x in ...` (for-in) vs `for init; cond; passo` (for-C, RFC 0008).
                   // Desambiguacao sem backtracking: for-in exige `Ident` + `in`;
                   // for-C comeca com outra coisa — ou `Ident` seguido de `;`/`=`/`(`,
                   // que abrem init valido (`x;`, `i = 0`, `f()`). `Ident` seguido do
                   // resto (`for x [...]`) cai no caminho for-in, que da o erro util
                   // "esperado `in`" (typo comum) em vez de erro obscuro.
    let is_for_in = match (
        cursor.peek_at(0).map(|t| &t.kind),
        cursor.peek_at(1).map(|t| &t.kind),
    ) {
        (Some(TokenKind::Ident(_)), Some(TokenKind::Ident(name))) if name == "in" => true,
        (
            Some(TokenKind::Ident(_)),
            Some(TokenKind::Semicolon | TokenKind::Assign | TokenKind::LParen),
        ) => false,
        (Some(TokenKind::Ident(_)), _) => true,
        _ => false,
    };
    if !is_for_in {
        return parse_for_c(cursor);
    }
    let var = expect_ident(cursor)?;
    // `in` e contextual: aceita apenas o identificador `in` nesta posicao,
    // sem reservar a palavra no resto da linguagem. A dica cobre o for-C
    // (`for init; cond; passo`), cuja init valida comeca com `;`, `=` ou
    // `(` apos o identificador (ou nao e identificador).
    match cursor.next() {
        Some(t) => match &t.kind {
            TokenKind::Ident(name) if name == "in" => {}
            other => {
                return Err(ParseError::new(
                    format!(
                        "esperado `in` apos variavel do `for` (ou `;`/`=`/`(` para o `for init; cond; passo`), encontrado {}",
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

/// `for init; cond; passo { corpo }` (for-C, RFC 0008; cursor apos `for`).
/// `init`: `;` (ausente) ou `let`/`const`/atribuicao/expressao (com `;`);
/// `cond`: `;` (vale `true`) ou expr + `;`;
/// `passo`: `{` (ausente) ou atribuicao/expressao SEM `;` final
/// (`let`/`const` no passo sao erro: redeclarariam a cada iteracao).
fn parse_for_c(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    let init = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Semicolon)) {
        cursor.next();
        None
    } else {
        match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Let) => Some(Box::new(parse_let(cursor)?)),
            Some(TokenKind::Const) => Some(Box::new(parse_const(cursor)?)),
            _ => Some(Box::new(parse_assign_or_expr_stmt(cursor)?)),
        }
    };
    let cond = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Semicolon)) {
        cursor.next();
        None
    } else {
        let expr = parse_expr(cursor)?;
        expect_semicolon(cursor)?;
        Some(expr)
    };
    let step = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LBrace)) {
        None
    } else {
        // Atribuicao (`x = e`, `x[i] = e`) ou expressao; o `{` fecha.
        // `let`/`const` aqui sao erro (redeclarariam a cada iteracao).
        if matches!(
            cursor.peek().map(|t| &t.kind),
            Some(TokenKind::Let | TokenKind::Const)
        ) {
            let tok = cursor.next().expect("peek garantiu `let`/`const`");
            return Err(ParseError::new(
                "passo do `for` nao declara variavel (use atribuicao ou expressao)".to_string(),
                tok.line,
                tok.col,
            ));
        }
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
                            "alvo de atribuicao invalido (use `nome` ou `nome[indice]`)"
                                .to_string(),
                            line,
                            col,
                        ));
                    }
                },
                _ => {
                    let (line, col) = cursor.end_pos();
                    return Err(ParseError::new(
                        "alvo de atribuicao invalido (use `nome` ou `nome[indice]`)".to_string(),
                        line,
                        col,
                    ));
                }
            };
            cursor.next(); // consome `=`
            let value = parse_expr(cursor)?;
            Some(Box::new(Stmt::Assign {
                target,
                expr: value,
            }))
        } else {
            Some(Box::new(Stmt::ExprStmt { expr }))
        }
    };
    let body = parse_block(cursor)?;
    Ok(Stmt::ForC {
        init,
        cond,
        step,
        body,
    })
}

/// `switch EXPR { case EXPR { ... } default { ... } }` (RFC 0004).
///
/// `default` e unico (segundo = erro) e livre de posicao; cases e default
/// exigem braces (consistente com `if`/`while`). `case`/`default` fora do
/// `switch` nao chegam aqui (o dispatch so entra via `TokenKind::Switch`).
fn parse_switch(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next(); // consome `switch`
    let discriminant = parse_expr(cursor)?;
    expect_token(
        cursor,
        &TokenKind::LBrace,
        "esperado `{` apos expressao do `switch`",
    )?;
    let mut cases = Vec::new();
    let mut has_default = false;
    loop {
        match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::RBrace) => {
                cursor.next();
                break;
            }
            Some(TokenKind::Case) => {
                cursor.next();
                let cond = parse_expr(cursor)?;
                let body = parse_block(cursor)?;
                cases.push(SwitchCase {
                    cond: Some(cond),
                    body,
                });
            }
            Some(TokenKind::Default) => {
                let tok = cursor.next().expect("peek garantiu `default`");
                if has_default {
                    return Err(ParseError::new(
                        "`switch` aceita um unico `default`".to_string(),
                        tok.line,
                        tok.col,
                    ));
                }
                has_default = true;
                let body = parse_block(cursor)?;
                cases.push(SwitchCase { cond: None, body });
            }
            Some(other) => {
                let tok = cursor.next().expect("peek garantiu token");
                return Err(ParseError::new(
                    format!(
                        "esperado `case`, `default` ou `}}` no `switch`, encontrado {}",
                        describe(other)
                    ),
                    tok.line,
                    tok.col,
                ));
            }
            None => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado `case`, `default` ou `}` no `switch`, encontrado fim de arquivo",
                    line,
                    col,
                ));
            }
        }
    }
    Ok(Stmt::Switch {
        discriminant,
        cases,
    })
}

/// `match sujeito { c1 => v1, c2 => v2, default => v3 }` (expressao, RFC 0008).
/// Igualdade estrita; primeiro match vence; `default` unico (qualquer posicao).
/// Virgula final opcional; `match` vazio e erro.
fn parse_match(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    // `tok` ja consumiu `match` (dispatch em `parse_primary`).
    let subject = parse_expr(cursor)?;
    expect_token(
        cursor,
        &TokenKind::LBrace,
        "esperado `{` apos sujeito do `match`",
    )?;
    let mut arms = Vec::new();
    let mut has_default = false;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RBrace)) {
        let (line, col) = cursor.end_pos();
        return Err(ParseError::new(
            "`match` vazio (declare ao menos um braco)".to_string(),
            line,
            col,
        ));
    }
    loop {
        let cond = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Default) => {
                let tok = cursor.next().expect("peek garantiu `default`");
                if has_default {
                    return Err(ParseError::new(
                        "`match` aceita um unico `default`".to_string(),
                        tok.line,
                        tok.col,
                    ));
                }
                has_default = true;
                None
            }
            Some(_) => Some(parse_expr(cursor)?),
            None => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado braco (`expr => ...`) ou `}` no `match`, encontrado fim de arquivo"
                        .to_string(),
                    line,
                    col,
                ));
            }
        };
        expect_token(
            cursor,
            &TokenKind::FatArrow,
            "esperado `=>` no braco do `match`",
        )?;
        let value = parse_expr(cursor)?;
        arms.push(MatchArm { cond, value });
        match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Comma) => {
                cursor.next();
                if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RBrace)) {
                    cursor.next();
                    break;
                }
            }
            Some(TokenKind::RBrace) => {
                cursor.next();
                break;
            }
            Some(other) => {
                let tok = cursor.next().expect("peek garantiu token");
                return Err(ParseError::new(
                    format!(
                        "esperado `,` ou `}}` no `match`, encontrado {}",
                        describe(other)
                    ),
                    tok.line,
                    tok.col,
                ));
            }
            None => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado `,` ou `}` no `match`, encontrado fim de arquivo".to_string(),
                    line,
                    col,
                ));
            }
        }
    }
    Ok(Expr::Match {
        subject: Box::new(subject),
        arms,
    })
}

fn parse_throw(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let expr = parse_expr(cursor)?;
    expect_semicolon(cursor)?;
    Ok(Stmt::Throw { expr })
}

fn parse_try(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let body = parse_block(cursor)?;
    let mut catch_var = None;
    let mut catch_body: Option<Vec<Stmt>> = None;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Catch)) {
        cursor.next();
        // `catch` com ou sem variavel: `catch e { }` ou `catch { }`.
        if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LBrace)) {
            catch_var = Some(expect_ident(cursor)?);
        }
        catch_body = Some(parse_block(cursor)?);
    }
    let mut finally_body = None;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Finally)) {
        cursor.next();
        finally_body = Some(parse_block(cursor)?);
    }
    if catch_body.is_none() && finally_body.is_none() {
        let (line, col) = cursor.end_pos();
        return Err(ParseError::new(
            "`try` exige ao menos `catch` ou `finally`",
            line,
            col,
        ));
    }
    // `catch` vazio (`catch e { }`) e valido: consome o erro e segue.
    Ok(Stmt::Try {
        body,
        catch_var,
        catch_body,
        finally_body,
    })
}

/// Corpo de `fn` anonima apos a lista de params (corpo isolado para manter
/// o frame de `parse_primary` enxuto no caminho quente; ver MAX_PARSE_DEPTH).
/// Arrow `fn (params) => expr` vira closure com `return` unico; senao bloco.
#[inline(never)]
fn parse_fn_body(
    cursor: &mut Cursor<'_>,
    params: Vec<Param>,
    variadic: Option<String>,
) -> Result<Expr, ParseError> {
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::FatArrow)) {
        cursor.next();
        let value = parse_expr(cursor)?;
        return Ok(Expr::Closure(Box::new(ClosureData {
            params,
            variadic,
            body: vec![Stmt::Return { expr: Some(value) }],
        })));
    }
    let body = parse_block(cursor)?;
    Ok(Expr::Closure(Box::new(ClosureData {
        params,
        variadic,
        body,
    })))
}

fn parse_fn_def(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = expect_ident(cursor)?;
    let (params, variadic) = parse_params_list(cursor, "apos nome da funcao")?;
    let body = parse_block(cursor)?;
    Ok(Stmt::FnDef {
        name,
        params,
        variadic,
        body,
    })
}

/// Lista de parametros `(a, b = expr, ...rest)`: `(` ja deve ser o proximo.
/// Retorna `(params, variadic)`. Regras (RFC 0006): `...rest` so por ultimo;
/// obrigatorio apos opcional e erro; duplicatas sao checadas na compilacao.
fn parse_params_list(
    cursor: &mut Cursor<'_>,
    context: &str,
) -> Result<(Vec<Param>, Option<String>), ParseError> {
    expect_token(
        cursor,
        &TokenKind::LParen,
        &format!("esperado `(` {context}"),
    )?;
    let mut params = Vec::new();
    let mut variadic = None;
    let mut seen_optional = false;
    if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
        loop {
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Ellipsis)) {
                cursor.next();
                let name_tok = cursor.next().ok_or_else(|| {
                    let (line, col) = cursor.end_pos();
                    ParseError::new("esperado nome apos `...` (resto de parametros)", line, col)
                })?;
                match &name_tok.kind {
                    TokenKind::Ident(name) => {
                        variadic = Some(name.clone());
                    }
                    other => {
                        return Err(ParseError::new(
                            format!("esperado nome apos `...`, encontrado {}", describe(other)),
                            name_tok.line,
                            name_tok.col,
                        ));
                    }
                }
                break;
            }
            let name = expect_ident(cursor)?;
            let default = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
                cursor.next();
                seen_optional = true;
                Some(parse_expr(cursor)?)
            } else {
                if seen_optional {
                    let (line, col) = cursor.end_pos();
                    return Err(ParseError::new(
                        "parametro obrigatorio apos opcional (reordene: opcionais por ultimo)"
                            .to_string(),
                        line,
                        col,
                    ));
                }
                None
            };
            params.push(Param { name, default });
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                cursor.next();
                // `, ...rest` ou `, )`: continua o laco (o `...`/`)` decide).
                continue;
            } else {
                break;
            }
        }
    }
    expect_token(cursor, &TokenKind::RParen, "esperado `)` apos parametros")?;
    Ok((params, variadic))
}

/// Argumentos de chamada `(a, b, ...)`: `(` ja deve ser o proximo token.
/// Argumentos de chamada `(a, x: b, ...c)`: `(` ja deve ser o proximo token.
/// Retorna `Vec<CallArg>` (RFC 0009). Regras: `x: v` so com `Ident` simples
/// (`:` apos outra expressao = erro; ternarios consomem o proprio `:`);
/// posicional apos nomeado e duplicata de nome = erro (visivel no fonte).
fn parse_call_args(cursor: &mut Cursor<'_>) -> Result<Vec<CallArg>, ParseError> {
    cursor.next();
    let mut args = Vec::new();
    let mut seen_named = false;
    let mut seen_names = std::collections::HashSet::new();
    if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RParen)) {
        loop {
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Ellipsis)) {
                cursor.next();
                let value = parse_expr(cursor)?;
                args.push(CallArg::Spread(value));
            } else {
                let expr = parse_expr(cursor)?;
                match expr {
                    Expr::Var(name)
                        if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Colon)) =>
                    {
                        cursor.next();
                        if !seen_names.insert(name.clone()) {
                            let (line, col) = cursor.end_pos();
                            return Err(ParseError::new(
                                format!("argumento nomeado duplicado: `{name}`"),
                                line,
                                col,
                            ));
                        }
                        seen_named = true;
                        let value = parse_expr(cursor)?;
                        args.push(CallArg::Named(name, value));
                    }
                    _ => {
                        if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Colon)) {
                            let tok = cursor.next().expect("peek garantiu `:`");
                            return Err(ParseError::new(
                                "nome de argumento esperado antes de `:` (use `nome: valor`)"
                                    .to_string(),
                                tok.line,
                                tok.col,
                            ));
                        }
                        if seen_named {
                            let (line, col) = cursor.end_pos();
                            return Err(ParseError::new(
                                "argumento posicional apos nomeado (reordene: posicionais primeiro)".to_string(),
                                line,
                                col,
                            ));
                        }
                        args.push(CallArg::Pos(expr));
                    }
                }
            }
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                cursor.next();
            } else {
                break;
            }
        }
    }
    expect_token(cursor, &TokenKind::RParen, "esperado `)` apos argumentos")?;
    Ok(args)
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
    // Seletivo: `import { a, b as c } from "mod.rhp";`.
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LBrace)) {
        return parse_import_names(cursor);
    }
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

fn parse_import_names(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next(); // `{`
    let mut names = Vec::new();
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RBrace)) {
        let (line, col) = cursor.end_pos();
        return Err(ParseError::new(
            "`import { }` vazio: selecione ao menos um nome",
            line,
            col,
        ));
    }
    loop {
        let name = expect_ident(cursor)?;
        // Alias opcional: `b as c` (`as` e contextual, como `in`/`from`).
        let alias = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Ident(word)) if word == "as" => {
                cursor.next();
                Some(expect_ident(cursor)?)
            }
            _ => None,
        };
        names.push(ImportName { name, alias });
        match cursor.next() {
            Some(t) => match &t.kind {
                TokenKind::Comma => {}
                TokenKind::RBrace => break,
                other => {
                    return Err(ParseError::new(
                        format!(
                            "esperado `,` ou `}}` no `import`, encontrado {}",
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
                    "esperado `,` ou `}` no `import`, encontrado fim de arquivo",
                    line,
                    col,
                ));
            }
        }
    }
    // `from` contextual (identificador posicional, nao reservado).
    match cursor.next() {
        Some(t) => match &t.kind {
            TokenKind::Ident(word) if word == "from" => {}
            other => {
                return Err(ParseError::new(
                    format!(
                        "esperado `from` apos lista do `import`, encontrado {}",
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
                "esperado `from` apos lista do `import`, encontrado fim de arquivo",
                line,
                col,
            ));
        }
    }
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
    Ok(Stmt::ImportNames { path, names })
}

/// Re-export seletivo (RFC 0011): cursor no `{` apos `export`.
/// Mesma forma do import seletivo; `from` obrigatorio (sem `from` e erro
/// claro, nao `export { }` local — RHP nao tem essa forma).
fn parse_reexport(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next(); // consome `{`
    let mut names = Vec::new();
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::RBrace)) {
        let (line, col) = cursor.end_pos();
        return Err(ParseError::new(
            "`export { }` vazio: selecione ao menos um nome (ou use `export fn/let/const`)"
                .to_string(),
            line,
            col,
        ));
    }
    loop {
        let name = expect_ident(cursor)?;
        let alias = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Ident(word)) if word == "as" => {
                cursor.next();
                Some(expect_ident(cursor)?)
            }
            _ => None,
        };
        names.push(ImportName { name, alias });
        match cursor.next() {
            Some(t) => match &t.kind {
                TokenKind::Comma => {}
                TokenKind::RBrace => break,
                other => {
                    return Err(ParseError::new(
                        format!(
                            "esperado `,` ou `}}` no re-export, encontrado {}",
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
                    "esperado `,` ou `}` no re-export, encontrado fim de arquivo".to_string(),
                    line,
                    col,
                ));
            }
        }
    }
    match cursor.peek().map(|t| &t.kind) {
        Some(TokenKind::Ident(word)) if word == "from" => {
            cursor.next();
        }
        _ => {
            let (line, col) = cursor.end_pos();
            return Err(ParseError::new(
                "re-export exige `from \"modulo\"` (ex.: `export { a } from \"lib.rhp\"`)"
                    .to_string(),
                line,
                col,
            ));
        }
    }
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
                "esperado caminho de modulo, encontrado fim de arquivo".to_string(),
                line,
                col,
            ));
        }
    };
    expect_semicolon(cursor)?;
    Ok(Stmt::ReExport { path, names })
}

fn parse_export(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    let (line, col) = cursor
        .peek()
        .map_or_else(|| cursor.end_pos(), |t| (t.line, t.col));
    cursor.next();
    // Re-export seletivo: `export { a, b as c } from "mod.rhp";` (RFC 0011).
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LBrace)) {
        return parse_reexport(cursor);
    }
    // So `fn`/`let`/`const` no nivel em que o `export` aparece (o loader
    // rejeita fora do nivel superior, como faz com `import`).
    let def = match cursor.peek().map(|t| &t.kind) {
        Some(TokenKind::Fn) => parse_fn_def(cursor)?,
        Some(TokenKind::Let) => parse_let(cursor)?,
        Some(TokenKind::Const) => parse_const(cursor)?,
        _ => {
            return Err(ParseError::new(
                "`export` so admite `fn`, `let` ou `const`",
                line,
                col,
            ));
        }
    };
    Ok(Stmt::Export { def: Box::new(def) })
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
        // Elvis `a ?: b` (RFC 0008): acucar de parse para `a ?? b`.
        if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Colon)) {
            return parse_elvis_rest(cursor, cond);
        }
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

/// Resto do Elvis apos `?` + `:` (corpo isolado para manter o frame de
/// `parse_ternary` enxuto no caminho quente; ver MAX_PARSE_DEPTH).
#[inline(never)]
fn parse_elvis_rest(cursor: &mut Cursor<'_>, cond: Expr) -> Result<Expr, ParseError> {
    cursor.next(); // consome `:`
    let else_branch = parse_ternary(cursor)?;
    Ok(Expr::Binary {
        op: BinOp::NullCoalesce,
        left: Box::new(cond),
        right: Box::new(else_branch),
    })
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
    loop {
        match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::LBracket) => {
                expr = parse_index_suffix(cursor, expr)?;
            }
            Some(TokenKind::LParen) => {
                expr = parse_call_suffix(cursor, expr)?;
            }
            _ => break,
        }
    }
    Ok(expr)
}

/// Corpo do sufixo `[indice]` isolado para manter o frame de
/// `parse_postfix` pequeno no caminho quente (ver MAX_PARSE_DEPTH).
#[inline(never)]
fn parse_index_suffix(cursor: &mut Cursor<'_>, base: Expr) -> Result<Expr, ParseError> {
    cursor.next();
    let index = parse_expr(cursor)?;
    expect_token(cursor, &TokenKind::RBracket, "esperado `]`")?;
    Ok(Expr::Index {
        base: Box::new(base),
        index: Box::new(index),
    })
}

/// Corpo do sufixo `(args)` isolado pelo mesmo motivo acima.
#[inline(never)]
fn parse_call_suffix(cursor: &mut Cursor<'_>, callee: Expr) -> Result<Expr, ParseError> {
    let args = parse_call_args(cursor)?;
    Ok(match callee {
        // `f(x)` direto preserva o caminho classico (builtins).
        Expr::Var(name) => Expr::Call { name, args },
        callee => Expr::CallValue {
            callee: Box::new(callee),
            args,
        },
    })
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
                let args = parse_call_args(cursor)?;
                Ok(Expr::Call { name, args })
            } else {
                Ok(Expr::Var(name))
            }
        }
        TokenKind::Fn => {
            // `tok` ja consumiu `fn`; `(` vem a seguir ou e erro claro.
            let (params, variadic) =
                parse_params_list(cursor, "apos `fn` (funcao anonima usa `fn (args) { }`)")?;
            parse_fn_body(cursor, params, variadic)
        }
        TokenKind::Match => parse_match(cursor),
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
        TokenKind::Match => "`match`".to_string(),
        TokenKind::FatArrow => "`=>`".to_string(),
        TokenKind::Ellipsis => "`...`".to_string(),
        TokenKind::Switch => "`switch`".to_string(),
        TokenKind::Case => "`case`".to_string(),
        TokenKind::Default => "`default`".to_string(),
        TokenKind::Try => "`try`".to_string(),
        TokenKind::Catch => "`catch`".to_string(),
        TokenKind::Finally => "`finally`".to_string(),
        TokenKind::Throw => "`throw`".to_string(),
        TokenKind::Export => "`export`".to_string(),
        TokenKind::Fn => "`fn`".to_string(),
        TokenKind::Const => "`const`".to_string(),
        TokenKind::Static => "`static`".to_string(),
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
    fn parses_switch_with_default_in_middle() {
        let program = lex_and_parse(
            "switch x { case 1 { echo \"a\"; } default { echo \"d\"; } case 2 { echo \"b\"; } }",
        )
        .expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::Switch {
                discriminant: Expr::Var("x".to_string()),
                cases: vec![
                    SwitchCase {
                        cond: Some(Expr::Int(1)),
                        body: vec![Stmt::Echo {
                            expr: Expr::StringLiteral("a".to_string()),
                        }],
                    },
                    SwitchCase {
                        cond: None,
                        body: vec![Stmt::Echo {
                            expr: Expr::StringLiteral("d".to_string()),
                        }],
                    },
                    SwitchCase {
                        cond: Some(Expr::Int(2)),
                        body: vec![Stmt::Echo {
                            expr: Expr::StringLiteral("b".to_string()),
                        }],
                    },
                ],
            }]
        );
        // Vazio e sem-default passam.
        lex_and_parse("switch x { }").expect("switch vazio deve passar");
        lex_and_parse("switch 1 { case 1 { echo 1; } }").expect("sem default deve passar");
    }

    #[test]
    fn rejects_switch_misuse() {
        // `default` duplo.
        let err = lex_and_parse("switch x { default { } default { } }")
            .expect_err("default duplo deve falhar");
        assert!(err.to_string().contains("unico `default`"), "obteve: {err}");
        // `case` fora de `switch` nao e statement valido (cai em expressao).
        let err = lex_and_parse("case 1 { echo 1; }").expect_err("case solto deve falhar");
        assert!(err.to_string().contains("`case`"), "obteve: {err}");
        // `default` fora de `switch` tambem nao.
        lex_and_parse("default { echo 1; }").expect_err("default solto deve falhar");
    }

    #[test]
    fn parses_params_with_defaults_and_variadic() {
        let program = lex_and_parse("fn f(a, b = 2, c = 1 + 2, ...rest) { return a; }")
            .expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::FnDef {
                name: "f".to_string(),
                params: vec![
                    Param {
                        name: "a".to_string(),
                        default: None,
                    },
                    Param {
                        name: "b".to_string(),
                        default: Some(Expr::Int(2)),
                    },
                    Param {
                        name: "c".to_string(),
                        default: Some(Expr::Binary {
                            op: BinOp::Add,
                            left: Box::new(Expr::Int(1)),
                            right: Box::new(Expr::Int(2)),
                        }),
                    },
                ],
                variadic: Some("rest".to_string()),
                body: vec![Stmt::Return {
                    expr: Some(Expr::Var("a".to_string())),
                }],
            }]
        );
        // Closure aceita a mesma forma.
        let program =
            lex_and_parse("let f = fn (a = \"x\") { return a; };").expect("parse deve passar");
        match &program.statements[0] {
            Stmt::Let {
                init: Some(Expr::Closure(data)),
                ..
            } => {
                assert_eq!(data.params.len(), 1);
                assert!(data.params[0].default.is_some());
                assert_eq!(data.variadic, None);
            }
            other => panic!("esperado closure, obteve {other:?}"),
        }
    }

    #[test]
    fn rejects_bad_param_forms() {
        // Obrigatorio apos opcional.
        let err = lex_and_parse("fn f(a = 1, b) { }").expect_err("deve falhar");
        assert!(err.to_string().contains("apos opcional"), "obteve: {err}");
        // `...` no meio.
        let err = lex_and_parse("fn f(...r, b) { }").expect_err("deve falhar");
        assert!(err.to_string().contains("`)`"), "obteve: {err}");
        // `...` sem nome.
        lex_and_parse("fn f(...) { }").expect_err("`...` sem nome deve falhar");
    }

    #[test]
    fn parses_named_and_spread_args() {
        let program = lex_and_parse("echo f(1, x: 2, ...r);").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::Echo {
                expr: Expr::Call {
                    name: "f".to_string(),
                    args: vec![
                        CallArg::Pos(Expr::Int(1)),
                        CallArg::Named("x".to_string(), Expr::Int(2)),
                        CallArg::Spread(Expr::Var("r".to_string())),
                    ],
                }
            }]
        );
        // Ternario com `:` nao vira nomeado; `:` apos nao-Var e erro.
        lex_and_parse("echo f(a ? b : c);").expect("ternario deve passar");
        let err = lex_and_parse("echo f(1 : 2);").expect_err("deve falhar");
        assert!(
            err.to_string().contains("nome de argumento"),
            "obteve: {err}"
        );
        // Posicional apos nomeado e duplicata: erros de parse.
        let err = lex_and_parse("echo f(x: 1, 2);").expect_err("deve falhar");
        assert!(
            err.to_string().contains("posicional apos nomeado"),
            "obteve: {err}"
        );
        let err = lex_and_parse("echo f(x: 1, x: 2);").expect_err("deve falhar");
        assert!(err.to_string().contains("duplicado"), "obteve: {err}");
    }

    #[test]
    fn parses_static_statement() {
        let program =
            lex_and_parse("fn f() { static n = 0; return n; }").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::FnDef {
                name: "f".to_string(),
                params: vec![],
                variadic: None,
                body: vec![
                    Stmt::Static {
                        name: "n".to_string(),
                        init: Expr::Int(0),
                    },
                    Stmt::Return {
                        expr: Some(Expr::Var("n".to_string())),
                    },
                ],
            }]
        );
        // Init obrigatorio.
        lex_and_parse("fn f() { static n; }").expect_err("static sem init deve falhar");
    }

    #[test]
    fn parses_destructuring_forms() {
        let program = lex_and_parse("let [a, b] = arr;").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::Destructure {
                target: DestructureTarget::Positional(vec!["a".to_string(), "b".to_string()]),
                init: Expr::Var("arr".to_string()),
                is_const: false,
            }]
        );
        let program = lex_and_parse("const {k: v, x} = m;").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::Destructure {
                target: DestructureTarget::ByKey(vec![
                    ("k".to_string(), "v".to_string()),
                    ("x".to_string(), "x".to_string()),
                ]),
                init: Expr::Var("m".to_string()),
                is_const: true,
            }]
        );
    }

    #[test]
    fn rejects_bad_destructure_forms() {
        // Padrao vazio.
        lex_and_parse("let [] = a;").expect_err("`[]` vazio deve falhar");
        lex_and_parse("let {} = m;").expect_err("`{}` vazio deve falhar");
        // Aninhado: `expect_ident` rejeita.
        lex_and_parse("let [[a]] = x;").expect_err("aninhado deve falhar");
        lex_and_parse("let {k: [v]} = m;").expect_err("aninhado deve falhar");
        // Placeholder `[, b]` nao suportado.
        lex_and_parse("let [, b] = x;").expect_err("placeholder deve falhar");
        // `const` sem init continua erro (regra vigente).
        lex_and_parse("const [a] ;").expect_err("const sem init deve falhar");
    }

    #[test]
    fn parses_match_expression() {
        let program = lex_and_parse("let x = match v { 1 => \"um\", _ => \"outro\" };")
            .expect("parse deve passar");
        // `_` nao e default: e variavel; valor e `match` com 2 bracos.
        match &program.statements[0] {
            Stmt::Let {
                init: Some(Expr::Match { arms, .. }),
                ..
            } => {
                assert_eq!(arms.len(), 2);
                assert!(arms[0].cond.is_some());
            }
            other => panic!("esperado match, obteve {other:?}"),
        }
        // `default`, virgula final, vazio e default duplo.
        lex_and_parse("echo match 1 { 1 => \"a\", default => \"b\", };").expect("deve passar");
        lex_and_parse("echo match x { };").expect_err("match vazio deve falhar");
        lex_and_parse("echo match x { default => 1, default => 2 };")
            .expect_err("default duplo deve falhar");
    }

    #[test]
    fn parses_elvis_and_arrow() {
        // Elvis desugara para `??`.
        let program = lex_and_parse("echo a ?: b;").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::NullCoalesce,
                    left: Box::new(Expr::Var("a".to_string())),
                    right: Box::new(Expr::Var("b".to_string())),
                }
            }]
        );
        // Arrow vira closure com `return` unico.
        let program = lex_and_parse("let f = fn (x) => x + 1;").expect("parse deve passar");
        match &program.statements[0] {
            Stmt::Let {
                init: Some(Expr::Closure(data)),
                ..
            } => {
                assert_eq!(
                    data.body,
                    vec![Stmt::Return {
                        expr: Some(Expr::Binary {
                            op: BinOp::Add,
                            left: Box::new(Expr::Var("x".to_string())),
                            right: Box::new(Expr::Int(1)),
                        })
                    }]
                );
            }
            other => panic!("esperada closure, obteve {other:?}"),
        }
    }

    #[test]
    fn parses_for_c_and_break_levels() {
        let program =
            lex_and_parse("for i = 0; i < 3; i = i + 1 { echo i; }").expect("parse deve passar");
        assert!(matches!(program.statements[0], Stmt::ForC { .. }));
        let program = lex_and_parse("for ;; { break; }").expect("deve passar");
        assert!(matches!(program.statements[0], Stmt::ForC { .. }));
        // `for x in` continua for-in (lookahead preservado).
        let program = lex_and_parse("for x in [1] { echo x; }").expect("deve passar");
        assert!(matches!(program.statements[0], Stmt::For { .. }));
        // Niveis.
        let program = lex_and_parse("while true { break 2; }").expect("deve passar");
        assert!(matches!(program.statements[0], Stmt::While { .. }));
        let program = lex_and_parse("break 2;").expect("deve passar");
        assert_eq!(program.statements, vec![Stmt::BreakN(2)]);
        let program = lex_and_parse("continue;").expect("deve passar");
        assert_eq!(program.statements, vec![Stmt::Continue]);
        lex_and_parse("break 0;").expect_err("nivel 0 deve falhar");
        lex_and_parse("break x;").expect_err("nivel nao-literal deve falhar");
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
    fn parses_anonymous_fn_and_general_calls() {
        // `let f = fn (x) { return x; };`
        let program = lex_and_parse("let f = fn (x) { return x; };").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Let {
                init: Some(Expr::Closure(_)),
                ..
            }
        ));
        // Chamada direta de literal: IIFE.
        let program = lex_and_parse("echo (fn (x) { return x; })(41);").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Echo {
                expr: Expr::CallValue { .. }
            }
        ));
        // `f(x)` direto continua `Call` (precedencia de builtins).
        let program = lex_and_parse("echo soma(1);").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Echo {
                expr: Expr::Call { .. }
            }
        ));
        // Chamada encadeada e por indice: `CallValue`.
        let program = lex_and_parse("echo f(1)(2);").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Echo {
                expr: Expr::CallValue { .. }
            }
        ));
    }

    #[test]
    fn rejects_named_fn_in_expression() {
        // `fn nome` so existe como instrucao; em expressao exige `(`.
        let err = lex_and_parse("let f = fn g(x) { return x; };").expect_err("deve falhar");
        assert!(
            err.message.contains("esperado `(`"),
            "obteve: {}",
            err.message
        );
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
    fn parses_try_catch_finally_and_throw() {
        let program =
            lex_and_parse("try { echo 1; } catch e { echo e; } finally { echo 2; } throw \"x\";")
                .expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![
                Stmt::Try {
                    body: vec![Stmt::Echo { expr: Expr::Int(1) }],
                    catch_var: Some("e".to_string()),
                    catch_body: Some(vec![Stmt::Echo {
                        expr: Expr::Var("e".to_string()),
                    }]),
                    finally_body: Some(vec![Stmt::Echo { expr: Expr::Int(2) }]),
                },
                Stmt::Throw {
                    expr: Expr::StringLiteral("x".to_string()),
                },
            ]
        );
        // `catch` sem variavel e `try` so com `finally` sao validos.
        let program =
            lex_and_parse("try { echo 1; } catch { echo 2; }").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Try {
                catch_var: None,
                finally_body: None,
                ..
            }
        ));
        let program =
            lex_and_parse("try { echo 1; } finally { echo 2; }").expect("parse deve passar");
        assert!(matches!(
            program.statements[0],
            Stmt::Try {
                catch_var: None,
                finally_body: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn rejects_try_without_catch_nor_finally() {
        let err = lex_and_parse("try { echo 1; }").expect_err("deve falhar");
        assert!(err.message.contains("ao menos"), "obteve: {}", err.message);
    }

    #[test]
    fn parses_export_prefix_and_selective_import() {
        let program = lex_and_parse("export fn dobro(x) { return x; } export let base = 40;")
            .expect("parse deve passar");
        assert!(matches!(program.statements[0], Stmt::Export { .. }));
        let program = lex_and_parse("import { dobro, base as b } from \"mat.rhp\";")
            .expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::ImportNames {
                path: "mat.rhp".to_string(),
                names: vec![
                    ImportName {
                        name: "dobro".to_string(),
                        alias: None,
                    },
                    ImportName {
                        name: "base".to_string(),
                        alias: Some("b".to_string()),
                    },
                ],
            }]
        );
        // `from`/`as` seguem identificadores comuns fora do `import`.
        let program = lex_and_parse("let from = 1; let a_as_b = 2;").expect("parse deve passar");
        assert_eq!(program.statements.len(), 2);
    }

    #[test]
    fn rejects_bad_export_and_import_forms() {
        let err = lex_and_parse("export echo 1;").expect_err("deve falhar");
        assert!(
            err.message.contains("`fn`, `let` ou `const`"),
            "obteve: {}",
            err.message
        );
        let err = lex_and_parse("import { a } \"m.rhp\";").expect_err("deve falhar");
        assert!(err.message.contains("`from`"), "obteve: {}", err.message);
        let err = lex_and_parse("import { } from \"m.rhp\";").expect_err("deve falhar");
        assert!(err.message.contains("vazio"), "obteve: {}", err.message);
    }

    #[test]
    fn parses_reexport_statement() {
        let program =
            lex_and_parse("export { a, b as c } from \"m.rhp\";").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::ReExport {
                path: "m.rhp".to_string(),
                names: vec![
                    ImportName {
                        name: "a".to_string(),
                        alias: None
                    },
                    ImportName {
                        name: "b".to_string(),
                        alias: Some("c".to_string())
                    },
                ],
            }]
        );
        // Erros: vazio, sem `from`, sem caminho.
        lex_and_parse("export { } from \"m.rhp\";").expect_err("vazio deve falhar");
        let err = lex_and_parse("export { a };").expect_err("sem from deve falhar");
        assert!(
            err.message.contains("exige `from"),
            "obteve: {}",
            err.message
        );
        lex_and_parse("export { a } from 1;").expect_err("caminho nao-string deve falhar");
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

    /// Gerador pseudoaleatorio deterministico (LCG; sem dependencias).
    struct FuzzRng(u64);

    impl FuzzRng {
        fn next(&mut self, bound: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % bound
        }
    }

    #[test]
    fn fuzz_random_tokens_never_panic() {
        // Etapa 3: sequencias aleatorias de fragmentos (seed fixa).
        // Lexer e parser nunca entram em panico: so Ok ou Err.
        const FRAGS: [&str; 51] = [
            "let", "const", "fn", "if", "else", "while", "for", "in", "return", "echo", "true",
            "false", "null", "match", "switch", "case", "default", "break", "continue", "try",
            "catch", "finally", "throw", "import", "export", "from", "as", "static", "x", "f",
            "123", "4.5", "\"s\"", "(", ")", "{", "}", "[", "]", ",", ";", ":", ".", "=", "==",
            "+", "-", "*", "<", ">", "!",
        ];
        let mut rng = FuzzRng(0x9A55_1EED_0A25_E5E0);
        for _ in 0..5000 {
            let mut src = String::new();
            let n = 1 + rng.next(20_usize);
            for i in 0..n {
                if i > 0 {
                    src.push(' ');
                }
                src.push_str(FRAGS[rng.next(FRAGS.len())]);
            }
            if let Ok(tokens) = rhp_lexer::lex(&src) {
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
