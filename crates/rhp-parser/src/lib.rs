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
//! NullCoalesce := LogicalOr (("??") NullCoalesce)?
//! LogicalOr  := LogicalAnd (("||") LogicalAnd)*
//! LogicalAnd := Equality (("&&") Equality)*
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
use std::cell::{Cell, RefCell};
use std::fmt;

/// Programa RHP: sequencia de instrucoes.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<StmtNode>,
    /// Comentarios `//` apos o ultimo statement (nunca perdidos; Etapa 4).
    pub trailing_comments: Vec<String>,
}

/// Statement com comentarios e espacamento associados (Etapa 4, DevX).
///
/// O `fmt` os reemite na mesma ordem relativa sem precisar de spans na
/// AST: a associacao e por ordem de statement, nao por linha.
#[derive(Debug, Clone, PartialEq)]
pub struct StmtNode {
    /// Comentarios `//` de linha propria que precedem o statement.
    pub leading: Vec<String>,
    /// Comentario `//` na mesma linha do fim do statement, se houver.
    pub trailing: Option<String>,
    /// Linha em branco antes (o `fmt` preserva no maximo 1).
    pub blank_before: bool,
    pub stmt: Stmt,
}

/// Igualdade estrutural ignorando comentarios (Etapa 4): permite aos
/// testes legados comparar `Vec<StmtNode>` com arvores `Stmt` puras via
/// `assert_eq!`. A comparacao desce recursivamente (corpos aninhados usam
/// o mesmo `impl` atraves do `PartialEq` cruzado de `Vec`).
impl PartialEq<Stmt> for StmtNode {
    fn eq(&self, other: &Stmt) -> bool {
        self.stmt == *other
    }
}

/// Statement sem comentarios (sub-parses que nao passam por `parse_stmt`,
/// nos sinteticos da VM, arvores esperadas nos testes). Comentarios ali
/// dentro sao preservados via o proximo statement.
impl From<Stmt> for StmtNode {
    fn from(stmt: Stmt) -> Self {
        StmtNode {
            leading: Vec::new(),
            trailing: None,
            blank_before: false,
            stmt,
        }
    }
}

/// Instrucao do RHP.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Echo {
        expr: Expr,
    },
    Let {
        name: String,
        /// Anotacao de tipo (`x: int`; RFC 0016). Checada em runtime.
        ann: Option<TypeTag>,
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
        /// Anotacao de tipo (`N: string`; RFC 0016). Checada em runtime.
        ann: Option<TypeTag>,
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
        then_branch: Vec<StmtNode>,
        else_branch: Option<Vec<StmtNode>>,
    },
    While {
        cond: Expr,
        body: Vec<StmtNode>,
    },
    /// `for VAR in ITER BLOCO` — itera elementos de array ou chaves
    /// ordenadas de mapa. `ITER` e avaliado uma vez (snapshot por valor);
    /// `VAR` e declarada a cada iteracao no escopo do corpo.
    /// `in` e contextual (identificador posicional, nao palavra-chave).
    For {
        var: String,
        iter: Expr,
        body: Vec<StmtNode>,
    },
    /// `for init; cond; passo { corpo }` (partes opcionais; RFC 0008).
    /// `init`/`passo`: qualquer statement (`None` = ausente);
    /// `cond`: `None` = `true`. Compilacao propria: `continue` salta ao passo.
    ForC {
        init: Option<Box<StmtNode>>,
        cond: Option<Expr>,
        step: Option<Box<StmtNode>>,
        body: Vec<StmtNode>,
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
    /// Fase Next-Gen 4: multiplos `catch Tipo? VAR? BLOCO` (captura tipada).
    Try {
        body: Vec<StmtNode>,
        catches: Vec<CatchClause>,
        finally_body: Option<Vec<StmtNode>>,
    },
    /// `throw EXPR;` — lanca o valor como erro (RFC 0001).
    Throw {
        expr: Expr,
    },
    /// `yield EXPR?;` — suspende o gerador entregando o valor (`Null` sem
    /// EXPR; RFC 0018, Etapa 5). So dentro de funcao (checado na compilacao).
    Yield {
        expr: Option<Expr>,
    },
    FnDef {
        name: String,
        params: Vec<Param>,
        /// `...rest` (no maximo um, por ultimo; RFC 0006).
        variadic: Option<String>,
        /// Anotacao de retorno (`: int`; RFC 0016). Checada em runtime.
        return_ann: Option<TypeTag>,
        body: Vec<StmtNode>,
        /// `async fn` (Fase Next-Gen 3): execucao em tarefa.
        #[allow(dead_code)]
        is_async: bool,
        /// Parametros de tipo (`fn f<T>`; Fase Next-Gen 5, apagamento).
        #[allow(dead_code)]
        generic_params: Vec<GenericParam>,
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
        def: Box<StmtNode>,
    },
    /// `class Nome { fn m() {} ... }` (RFC 0017, Etapa 5). Sem heranca.
    /// Fase Next-Gen 1: `extends`, `implements`, `use` (traits), generics.
    ClassDef {
        name: String,
        /// Classe pai (`extends Base`), unico (sem heranca multipla).
        parent: Option<String>,
        /// Interfaces implementadas (`implements A, B`).
        interfaces: Vec<String>,
        /// Traits compostas (`use T1, T2;` dentro do corpo).
        uses: Vec<String>,
        /// Parametros de tipo (`class Caixa<T>`; apagamento).
        generic_params: Vec<GenericParam>,
        methods: Vec<ClassMethod>,
    },
    /// `interface Nome { fn m(params); ... }` (Fase Next-Gen 1): contrato.
    /// Somente assinaturas (sem corpo).
    InterfaceDef {
        name: String,
        /// Parametros de tipo (`interface Repo<T>`; apagamento).
        generic_params: Vec<GenericParam>,
        methods: Vec<InterfaceMethod>,
    },
    /// `trait Nome { fn m() { ... } }` (Fase Next-Gen 1): reutilizacao.
    TraitDef {
        name: String,
        methods: Vec<ClassMethod>,
    },
    /// `namespace App\Controllers;` (Fase Next-Gen 2): prefixo do arquivo.
    Namespace {
        name: String,
    },
    /// `import App\Controllers\Foo;` ou `import App\Foo as Bar;`
    /// (Fase Next-Gen 2): resolve via autoload (mapeamento namespace->dir).
    ImportQualified {
        path: String,
        alias: Option<String>,
    },
    /// `import native "ext";` (Fase Next-Gen 6): carrega extensao estatica.
    ImportNative {
        name: String,
    },
    Block(Vec<StmtNode>),
    ExprStmt {
        expr: Expr,
    },
}

/// Metodo de classe: como `FnDef`, mas sem nome global (vive na tabela
/// da classe; RFC 0017). `this` e vinculado na chamada (nao e parametro).
#[derive(Debug, Clone, PartialEq)]
pub struct ClassMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub variadic: Option<String>,
    pub return_ann: Option<TypeTag>,
    pub body: Vec<StmtNode>,
    /// `async fn` como metodo (item metodos-async): a chamada gera tarefa
    /// (snapshot do `this`, sem write-back). Traits aceitam igual.
    pub is_async: bool,
}

/// Assinatura de metodo de interface (sem corpo; Fase Next-Gen 1).
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub variadic: Option<String>,
    pub return_ann: Option<TypeTag>,
}

/// Clausula `catch` (Fase Next-Gen 4): `catch Tipo? var? Bloco`.
/// Sem tipo = captura tudo (compativel com RFC 0001).
/// Com tipo = captura instancias da classe/tipo ou subclasses.
#[derive(Debug, Clone, PartialEq)]
pub struct CatchClause {
    pub type_name: Option<String>,
    pub var: Option<String>,
    pub body: Vec<StmtNode>,
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
    /// Anotacao de tipo (`a: int`; RFC 0016, Etapa 5). Checada em runtime.
    pub ann: Option<TypeTag>,
}

/// Tag de tipo para anotacoes (`: int`; RFC 0016, Etapa 5).
///
/// Nomes iguais aos de `type()` (`mapa`, nao `map`). `Copy` de proposito:
/// tags circulam por valor em AST, bytecode e VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    Int,
    Float,
    String,
    Bool,
    Array,
    Map,
    Null,
    Func,
}

impl TypeTag {
    /// Nome no fonte -> tag (`None` = tipo desconhecido).
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "int" => Some(Self::Int),
            "float" => Some(Self::Float),
            "string" => Some(Self::String),
            "bool" => Some(Self::Bool),
            "array" => Some(Self::Array),
            "mapa" => Some(Self::Map),
            "null" => Some(Self::Null),
            "funcao" => Some(Self::Func),
            _ => None,
        }
    }

    /// Nome canonico (mensagens de erro, `fmt`).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::Bool => "bool",
            Self::Array => "array",
            Self::Map => "mapa",
            Self::Null => "null",
            Self::Func => "funcao",
        }
    }
}

/// Ramo de `switch`: `cond: None` e o `default` (unico, posicao livre).
#[derive(Debug, Clone, PartialEq)]
pub struct SwitchCase {
    pub cond: Option<Expr>,
    pub body: Vec<StmtNode>,
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
    Call(Box<CallData>),
    /// Chamada de callee arbitrario: `f()(1)`, `(fn(x) { x })(2)`,
    /// `arr[0](x)`. `f(x)` direto continua `Call` (precedencia de builtins
    /// preservada byte a byte). `Box` no callee e no payload para nao
    /// inflar `sizeof(Expr)` (o guarda de profundidade assume frames
    /// enxutos; ver MAX_PARSE_DEPTH).
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
    /// Acesso a membro: `obj.campo` (RFC 0017). Compila para `Index` com
    /// `Str`; em instancia resolve campo ou metodo vinculado.
    Member {
        obj: Box<Expr>,
        field: String,
    },
    /// Instanciacao: `new Nome(args)` (RFC 0017). `args` aceita as formas
    /// de chamada (posicional, nomeado, spread); a classe vem de `Ident`.
    /// Fase Next-Gen 5 / item bounds: `new Caixa<Sub>` guarda os argumentos
    /// de tipo (checagem de aridade/bounds em compilacao; apagados no runtime).
    New(Box<NewData>),
    /// `super` (Fase Next-Gen 1): classe pai no metodo (`super.falar()`).
    Super,
    /// `await expr` (Fase Next-Gen 3): suspende ate a tarefa/futuro concluir.
    Await {
        expr: Box<Expr>,
    },
}

/// Braco de `match`: `cond: None` e o `default` (RFC 0008).
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub cond: Option<Expr>,
    pub value: Expr,
}

/// Chamada direta `nome<Args...>(...)`: payload encaixotado para manter
/// `sizeof(Expr)` enxuto (ver `MAX_PARSE_DEPTH`). `type_args` e validado
/// sintaticamente no parse e checado (aridade/bounds, RFC 0029) na
/// compilacao; apagado no bytecode.
#[derive(Debug, Clone, PartialEq)]
pub struct CallData {
    pub name: String,
    pub type_args: Vec<String>,
    pub args: Vec<CallArg>,
}

/// Instanciacao `new Nome<Args...>(...)`: idem (checagem em compilacao,
/// apagamento no runtime).
#[derive(Debug, Clone, PartialEq)]
pub struct NewData {
    pub class: Box<Expr>,
    pub type_args: Vec<String>,
    pub args: Vec<CallArg>,
}

/// Parametro de tipo (`T` ou `T: Bound`; item bounds, RFC 0029).
/// `bound` e o nome qualificado da classe/interface exigida (`None` = sem
/// restricao). Validado na definicao (deve existir); verificado no uso.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericParam {
    pub name: String,
    pub bound: Option<String>,
}

/// Corpo de closure, sempre heap-alocado (ver `Expr::Closure`).
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureData {
    pub params: Vec<Param>,
    /// `...rest` (no maximo um, por ultimo; RFC 0006).
    pub variadic: Option<String>,
    /// Anotacao de retorno (`: int`; RFC 0016). Checada em runtime.
    pub return_ann: Option<TypeTag>,
    pub body: Vec<StmtNode>,
    /// `async fn` como valor (Fase Next-Gen 3).
    pub is_async: bool,
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
    /// `&&` logico com curto-circuito (RFC 0031; estritamente `Bool`).
    LogicalAnd,
    /// `||` logico com curto-circuito (RFC 0031; estritamente `Bool`).
    LogicalOr,
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
    pos: Cell<usize>,
    depth: u32,
    /// Comentarios `//` pulados (linha, texto), drenados por statement.
    /// `RefCell`/`Cell` para `peek` continuar `&self` (Etapa 4).
    pending: RefCell<Vec<(usize, String)>>,
    /// Linha do ultimo token consumido (0 = nenhum; Etapa 4).
    last_line: Cell<usize>,
}

/// Profundidade sintatica maxima (protecao contra estouro de pilha com
/// entradas patologicas; ver AUD-001). Cadeias planas (`1+1+...`, instrucoes
/// em sequencia) nao acumulam profundidade. Calibrado para caber com folga
/// na pilha de 2 MiB das threads de teste (~9 frames Rust por nivel).
/// Teto de profundidade sintatica (niveis de `nest` + elos de cadeia).
/// Calibrado por medicao (RHP-AUD-001): em builds debug, ~120 niveis de
/// parenteses ja estouram threads de 2 MiB (cada nivel ~15 quadros de
/// parse); 64 da ~2x de margem. Vale para aninhamento E cadeias planas
/// (`1+1+...`, `a.b.c`, `!!!x`), que aprofundam a AST do mesmo jeito.
pub const MAX_PARSE_DEPTH: u32 = 64;

impl<'a> Cursor<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            pos: Cell::new(0),
            depth: 0,
            pending: RefCell::new(Vec::new()),
            last_line: Cell::new(0),
        }
    }

    /// Pula comentarios `//` coletando-os em `pending` (ordem do fonte).
    fn skip_comments(&self) {
        while let Some(tok) = self.tokens.get(self.pos.get()) {
            match &tok.kind {
                TokenKind::Comment(text) => {
                    self.pending.borrow_mut().push((tok.line, text.clone()));
                    self.pos.set(self.pos.get() + 1);
                }
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<&'a Token> {
        self.skip_comments();
        self.tokens.get(self.pos.get())
    }

    /// Lookahead de N tokens (para desambiguar `for x in` vs `for init;`).
    /// Pula comentarios sem consumi-los (coleta acontece no `peek`/`next`).
    fn peek_at(&self, offset: usize) -> Option<&'a Token> {
        let mut idx = self.pos.get();
        let mut seen = 0;
        loop {
            let tok = self.tokens.get(idx)?;
            if matches!(tok.kind, TokenKind::Comment(_)) {
                idx += 1;
                continue;
            }
            if seen == offset {
                return Some(tok);
            }
            seen += 1;
            idx += 1;
        }
    }

    fn next(&mut self) -> Option<&'a Token> {
        self.skip_comments();
        let tok = self.tokens.get(self.pos.get())?;
        self.pos.set(self.pos.get() + 1);
        self.last_line.set(tok.line);
        Some(tok)
    }

    /// Linha do ultimo token consumido (0 se nenhum).
    fn last_consumed_line(&self) -> usize {
        self.last_line.get()
    }

    /// Drena comentarios pendentes, opcionalmente so os da `line`.
    /// Devolve `(linha, texto)` para o chamador decidir o vinculo.
    fn take_pending(&self, line: Option<usize>) -> Vec<(usize, String)> {
        let mut pending = self.pending.borrow_mut();
        let mut out = Vec::new();
        pending.retain(|(l, text)| {
            if line.is_none_or(|want| *l == want) {
                out.push((*l, text.clone()));
                false
            } else {
                true
            }
        });
        out
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

    /// Conta um elo de cadeia esquerda (`a op b op c...`, `a.b.c`,
    /// `a[0][1]`) contra o teto de profundidade. Cadeias nao passam por
    /// `nest` (sao lacos, nao niveis), mas cada elo aprofunda a AST a
    /// esquerda — sem teto, `1+1+...` arbitrariamente longo estoura a
    /// pilha nativa em `compile`/`Display`/`Drop` (RHP-AUD-001).
    fn check_chain(&self, chain: u32, what: &str) -> Result<(), ParseError> {
        if self.depth + chain > MAX_PARSE_DEPTH {
            let (line, col) = self.end_pos();
            return Err(ParseError::new(
                format!("profundidade maxima excedida em {what} (limite {MAX_PARSE_DEPTH})"),
                line,
                col,
            ));
        }
        Ok(())
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
        | TokenKind::QuestionQuestion
        | TokenKind::LogicalAnd
        | TokenKind::LogicalOr => 2,
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

    Ok(Program {
        statements,
        trailing_comments: cursor
            .take_pending(None)
            .into_iter()
            .map(|(_, text)| text)
            .collect(),
    })
}

fn parse_stmt(cursor: &mut Cursor<'_>) -> Result<StmtNode, ParseError> {
    // Linha em branco antes: linhas vazias entre o ultimo token consumido
    // e o primeiro deste statement, descontando as dos comentarios
    // precedentes (cada `//` ocupa exatamente a sua linha).
    let prev = cursor.last_consumed_line();
    let first = cursor.peek().map(|t| t.line).unwrap_or(prev + 1);
    let pending = cursor.take_pending(None);
    let in_range = pending
        .iter()
        .filter(|(l, _)| *l > prev && *l < first)
        .count();
    let blank_before = first
        .saturating_sub(prev)
        .saturating_sub(1)
        .saturating_sub(in_range)
        > 0;
    let leading: Vec<String> = pending.into_iter().map(|(_, text)| text).collect();
    let stmt = cursor.nest("instrucao", parse_stmt_inner)?;
    // Forca a coleta do que segue (o `peek` pula comentarios para
    // `pending`); o comentario na mesma linha do fim e trailing deste
    // statement, o resto fica pendente para o proximo.
    let _ = cursor.peek();
    let end = cursor.last_consumed_line();
    let trailing = cursor.take_pending(Some(end)).pop().map(|(_, text)| text);
    Ok(StmtNode {
        leading,
        trailing,
        blank_before,
        stmt,
    })
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
        TokenKind::Yield => parse_yield(cursor),
        TokenKind::Fn => parse_fn_def(cursor, false),
        TokenKind::Async => parse_async_or_await_stmt(cursor),
        TokenKind::Class => parse_class_def(cursor),
        TokenKind::Interface => parse_interface_def(cursor),
        TokenKind::Trait => parse_trait_def(cursor),
        TokenKind::Namespace => parse_namespace(cursor),
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
    let ann = parse_type_ann(cursor)?;
    expect_token(
        cursor,
        &TokenKind::Assign,
        "esperado `=` apos nome da constante",
    )?;
    let init = parse_expr(cursor)?;
    expect_semicolon(cursor)?;
    Ok(Stmt::Const { name, ann, init })
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
    let ann = parse_type_ann(cursor)?;
    let init = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Assign)) {
        cursor.next();
        Some(parse_expr(cursor)?)
    } else {
        None
    };
    expect_semicolon(cursor)?;
    Ok(Stmt::Let { name, ann, init })
}

/// Anotacao de tipo apos `:` (`x: int`; RFC 0016). `None` = ausente.
/// Fase Next-Gen 5: nomes genericos (`T`) sao apagados (sem checagem);
/// `array<T>` / `mapa<K,V>` sao apagados para o tipo base.
fn parse_type_ann(cursor: &mut Cursor<'_>) -> Result<Option<TypeTag>, ParseError> {
    if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Colon)) {
        return Ok(None);
    }
    cursor.next();
    parse_type_ann_after_colon(cursor)
}

/// Nucleo de `parse_type_ann` apos consumir `:` (reuso em params genericos).
fn parse_type_ann_after_colon(cursor: &mut Cursor<'_>) -> Result<Option<TypeTag>, ParseError> {
    let tok = cursor.next().ok_or_else(|| {
        let (line, col) = cursor.end_pos();
        ParseError::new("esperado tipo apos `:` (int, string, ...)", line, col)
    })?;
    let base: Option<TypeTag> = match &tok.kind {
        TokenKind::Ident(name) => match TypeTag::parse(name) {
            Some(tag) => Some(tag),
            None => {
                // Generico (`T`, `K`, `V`, ...): apagamento, sem checagem.
                // Aceita identificador simples como parametro de tipo.
                if is_generic_name(name) {
                    // Consome eventual `<...>` apos generico? Nao ha; retorna None.
                    return Ok(None);
                }
                return Err(ParseError::new(
                    format!("tipo desconhecido `{name}` (use int, float, string, bool, array, mapa, null, funcao)"),
                    tok.line,
                    tok.col,
                ));
            }
        },
        // `null` e palavra-chave (nao `Ident`); tipo valido.
        TokenKind::Null => Some(TypeTag::Null),
        other => {
            return Err(ParseError::new(
                format!("esperado tipo apos `:`, encontrado {}", describe(other)),
                tok.line,
                tok.col,
            ));
        }
    };
    // Apagamento de parametrizacao: `array<T>`, `mapa<K,V>`, `Classe<T>`.
    // Se `<` segue, consome `< ident (, ident)* >` e ignora.
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Lt)) {
        cursor.next();
        // Consome ate `>` (inclui `>>` como dois `>`).
        let mut depth = 1u32;
        while depth > 0 {
            match cursor.next() {
                Some(t) => match &t.kind {
                    TokenKind::Lt => depth += 1,
                    TokenKind::Gt => depth -= 1,
                    TokenKind::Shr => {
                        if depth >= 2 {
                            depth -= 2;
                        } else {
                            depth = 0;
                        }
                    }
                    TokenKind::Ident(_) | TokenKind::Comma => {}
                    TokenKind::Null | TokenKind::IntLiteral(_) | TokenKind::StringLiteral(_) => {}
                    _ => {
                        // Permite `>>` dividido? O lexer emite `Shr`; tratado acima.
                        // Qualquer outro token dentro de `<...>` e ignorado se for
                        // parte de nome qualificado; senao erro suave: continua.
                    }
                },
                None => {
                    let (line, col) = cursor.end_pos();
                    return Err(ParseError::new(
                        "esperado `>` apos argumentos de tipo".to_string(),
                        line,
                        col,
                    ));
                }
            }
        }
    }
    Ok(base)
}

/// Nome de parametro de tipo (`T`, `K`, `V`, `TResult`, ...): comeca com
/// maiuscula (convencao) ou e letra unica maiuscula. Usado para apagamento.
fn is_generic_name(name: &str) -> bool {
    matches!(name.chars().next(), Some(c) if c.is_uppercase())
}

/// Parametros de tipo `<T, U: Bound>` apos nome de fn/classe (item bounds).
/// Retorna lista vazia se nao houver `<`. Nao consome `<` de comparacao:
/// so trata como generics se apos `<` vier `Ident` e eventualmente `,`/`>`.
#[inline(never)]
fn parse_generic_params_opt(cursor: &mut Cursor<'_>) -> Result<Vec<GenericParam>, ParseError> {
    if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Lt)) {
        return Ok(Vec::new());
    }
    // Lookahead: `< Ident` (ou `< Ident,`)? Senao, nao e generics (e `<` binario).
    let is_generic = matches!(
        cursor.peek_at(1).map(|t| &t.kind),
        Some(TokenKind::Ident(_))
    );
    if !is_generic {
        return Ok(Vec::new());
    }
    cursor.next(); // `<`
    let mut params = Vec::new();
    loop {
        let name = expect_ident(cursor)?;
        if !is_generic_name(&name) && name.chars().next().is_some_and(|c| c.is_lowercase()) {
            // Permite minusculas tambem (ex.: `<t>`)? Exige maiuscula por convencao,
            // mas aceita qualquer ident para nao quebrar: valida duplicata abaixo.
        }
        // Bound opcional: `T: ClasseOuInterface` (item bounds, RFC 0029).
        // `:` aqui nao e ternario (contexto de lista de tipos).
        let bound = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Colon)) {
            cursor.next(); // `:`
            Some(parse_qualified_name(cursor)?)
        } else {
            None
        };
        if params.iter().any(|p: &GenericParam| p.name == name) {
            let (line, col) = cursor.end_pos();
            return Err(ParseError::new(
                format!("parametro de tipo duplicado `{name}`"),
                line,
                col,
            ));
        }
        params.push(GenericParam { name, bound });
        match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Comma) => {
                cursor.next();
            }
            Some(TokenKind::Gt) => {
                cursor.next();
                break;
            }
            _ => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado `,` ou `>` em parametros de tipo".to_string(),
                    line,
                    col,
                ));
            }
        }
    }
    Ok(params)
}

/// `true` se daqui (`<`) ha `< T (, T)* > (` (tipos + chamada).
/// Varredura sem consumir (via `peek_at`); `>>` (`Shr`) conta como dois `>`.
/// Evita confundir `i < n` (comparacao) com `f<T>(x)` (generics apagados).
fn looks_like_type_args_call(cursor: &Cursor<'_>) -> bool {
    // peek_at(0) = `<` (garantido pelo chamador).
    let mut i = 1usize;
    // Primeiro tipo.
    match cursor.peek_at(i).map(|t| &t.kind) {
        Some(TokenKind::Ident(_)) | Some(TokenKind::Null) => i += 1,
        _ => return false,
    }
    loop {
        match cursor.peek_at(i).map(|t| &t.kind) {
            Some(TokenKind::Comma) => {
                i += 1;
                match cursor.peek_at(i).map(|t| &t.kind) {
                    Some(TokenKind::Ident(_)) | Some(TokenKind::Null) => i += 1,
                    _ => return false,
                }
            }
            Some(TokenKind::Gt) => {
                i += 1;
                break;
            }
            Some(TokenKind::Shr) => {
                // `>>`: fecha um nivel (suficiente para o padrao simples);
                // padroes aninhados complexos sao tratados no consumo real.
                i += 1;
                break;
            }
            // Aninhado `array<T>`: pula `<...>` interno de forma simples.
            Some(TokenKind::Lt) => {
                let mut depth = 1usize;
                i += 1;
                while depth > 0 {
                    match cursor.peek_at(i).map(|t| &t.kind) {
                        Some(TokenKind::Lt) => depth += 1,
                        Some(TokenKind::Gt) => depth -= 1,
                        Some(TokenKind::Shr) => {
                            depth = depth.saturating_sub(2);
                            if depth == 0 {
                                break;
                            }
                        }
                        Some(_) => {}
                        None => return false,
                    }
                    i += 1;
                    if i > 64 {
                        return false;
                    }
                }
            }
            _ => return false,
        }
        if i > 64 {
            return false;
        }
    }
    matches!(cursor.peek_at(i).map(|t| &t.kind), Some(TokenKind::LParen))
}

/// Argumentos de tipo `<int, string>` em uso (`new Caixa<int>`, `f<int>()`).
/// Apagados em runtime; valida apenas a forma (idents/`null` separados por `,`).
#[inline(never)]
fn parse_type_args_opt(cursor: &mut Cursor<'_>) -> Result<Vec<String>, ParseError> {
    if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Lt)) {
        return Ok(Vec::new());
    }
    let is_generic_use = matches!(
        cursor.peek_at(1).map(|t| &t.kind),
        Some(TokenKind::Ident(_)) | Some(TokenKind::Null)
    );
    if !is_generic_use {
        return Ok(Vec::new());
    }
    // Desambiguacao `<` binario vs generics (Fase Next-Gen 5): so trata como
    // argumentos de tipo se houver `< T (, T)* > (` adiante (colado ou nao).
    // Ex.: `i < n` (comparacao) nao tem `>` nem `(`; `f<T>(x)` tem.
    if !looks_like_type_args_call(cursor) {
        return Ok(Vec::new());
    }
    cursor.next(); // `<`
    let mut args = Vec::new();
    loop {
        let tok = cursor.next().ok_or_else(|| {
            let (line, col) = cursor.end_pos();
            ParseError::new("esperado tipo em argumentos de tipo".to_string(), line, col)
        })?;
        let arg = match &tok.kind {
            TokenKind::Ident(s) => s.clone(),
            TokenKind::Null => "null".to_string(),
            _ => {
                return Err(ParseError::new(
                    format!("esperado tipo, encontrado {}", describe(&tok.kind)),
                    tok.line,
                    tok.col,
                ));
            }
        };
        // Apagamento aninhado: `array<T>` -> consome `<...>` interno.
        if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Lt)) {
            cursor.next();
            let mut depth = 1u32;
            while depth > 0 {
                match cursor.next() {
                    Some(t) => match &t.kind {
                        TokenKind::Lt => depth += 1,
                        TokenKind::Gt => depth -= 1,
                        TokenKind::Shr => {
                            depth = depth.saturating_sub(2);
                        }
                        _ => {}
                    },
                    None => {
                        let (line, col) = cursor.end_pos();
                        return Err(ParseError::new(
                            "esperado `>` apos argumentos de tipo".to_string(),
                            line,
                            col,
                        ));
                    }
                }
            }
        }
        args.push(arg);
        match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Comma) => {
                cursor.next();
            }
            Some(TokenKind::Gt) => {
                cursor.next();
                break;
            }
            _ => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado `,` ou `>` em argumentos de tipo".to_string(),
                    line,
                    col,
                ));
            }
        }
    }
    Ok(args)
}

/// Nome qualificado `A\B\C` (Fase Next-Gen 2): `Ident (Backslash Ident)*`.
#[inline(never)]
fn parse_qualified_name(cursor: &mut Cursor<'_>) -> Result<String, ParseError> {
    let mut name = expect_ident(cursor)?;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Backslash)) {
        cursor.next();
        let part = expect_ident(cursor)?;
        name.push('\\');
        name.push_str(&part);
    }
    Ok(name)
}

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
            Some(TokenKind::Let) => Some(Box::new(StmtNode::from(parse_let(cursor)?))),
            Some(TokenKind::Const) => Some(Box::new(StmtNode::from(parse_const(cursor)?))),
            _ => Some(Box::new(StmtNode::from(parse_assign_or_expr_stmt(cursor)?))),
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
            Some(Box::new(StmtNode::from(Stmt::Assign {
                target,
                expr: value,
            })))
        } else {
            Some(Box::new(StmtNode::from(Stmt::ExprStmt { expr })))
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

/// `yield EXPR?;` (RFC 0018, Etapa 5): sem EXPR entrega `Null`.
/// So valido dentro de funcao (checado na compilacao, como `return`).
fn parse_yield(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let expr = if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Semicolon)) {
        None
    } else {
        Some(parse_expr(cursor)?)
    };
    expect_semicolon(cursor)?;
    Ok(Stmt::Yield { expr })
}

fn parse_try(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let body = parse_block(cursor)?;
    let mut catches = Vec::new();
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Catch)) {
        cursor.next();
        // Formas: `catch { }`, `catch e { }`, `catch Tipo e { }`, `catch Tipo { }`.
        // Heuristica: se proximo e `{`, sem tipo nem var; se Ident seguido de `{`,
        // e var (legado) ou tipo sem var; se dois Idents, tipo + var.
        let (type_name, var) = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::LBrace) => (None, None),
            _ => {
                // Tenta ler primeiro identificador (pode ser tipo ou var).
                let first = match cursor.peek() {
                    Some(t) => match &t.kind {
                        TokenKind::Ident(s) => s.clone(),
                        _ => {
                            let (line, col) = cursor.end_pos();
                            return Err(ParseError::new(
                                "esperado tipo, variavel ou `{` apos `catch`".to_string(),
                                line,
                                col,
                            ));
                        }
                    },
                    None => {
                        let (line, col) = cursor.end_pos();
                        return Err(ParseError::new(
                            "esperado bloco apos `catch`, encontrado fim de arquivo".to_string(),
                            line,
                            col,
                        ));
                    }
                };
                // Olha adiante: Ident Ident => tipo + var.
                // Checa se apos o primeiro vem outro Ident (tipo + var) ou `{` direto.
                // peek_at(1) e o token apos o primeiro Ident.
                let second_is_ident = matches!(
                    cursor.peek_at(1).map(|t| &t.kind),
                    Some(TokenKind::Ident(_))
                );
                if second_is_ident {
                    // Consome tipo e var (qualificados com `\`).
                    let tname = parse_qualified_name(cursor)?;
                    let vname = expect_ident(cursor)?;
                    (Some(tname), Some(vname))
                } else {
                    // Um unico Ident: compativel com legado (`catch e`).
                    // Se for nome de classe conhecido? Sem tabela aqui: trata como var
                    // quando o bloco segue, exceto se explicitamente dois idents.
                    // Para `catch Tipo { }` sem var, o usuario escreve `catch Tipo _`?
                    // Nao: aceitamos tipo sem var quando o ident comeca com maiuscula
                    // E o proximo e `{`? Ambiguo com var minuscula. Regra: se o ident
                    // comeca com maiuscula e o bloco segue, trata como TIPO sem var
                    // (convencao RHP/Java); senao, var legada.
                    let _ = first;
                    let nm = parse_qualified_name(cursor)?;
                    let is_upper = nm
                        .rsplit('\\')
                        .next()
                        .and_then(|s| s.chars().next())
                        .is_some_and(|c| c.is_uppercase());
                    if is_upper {
                        (Some(nm), None)
                    } else {
                        (None, Some(nm))
                    }
                }
            }
        };
        let catch_body = parse_block(cursor)?;
        catches.push(CatchClause {
            type_name,
            var,
            body: catch_body,
        });
    }
    let mut finally_body = None;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Finally)) {
        cursor.next();
        finally_body = Some(parse_block(cursor)?);
    }
    if catches.is_empty() && finally_body.is_none() {
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
        catches,
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
    let return_ann = parse_type_ann(cursor)?;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::FatArrow)) {
        cursor.next();
        let value = parse_expr(cursor)?;
        return Ok(Expr::Closure(Box::new(ClosureData {
            params,
            variadic,
            return_ann,
            body: vec![StmtNode::from(Stmt::Return { expr: Some(value) })],
            is_async: false,
        })));
    }
    let body = parse_block(cursor)?;
    Ok(Expr::Closure(Box::new(ClosureData {
        params,
        variadic,
        return_ann,
        body,
        is_async: false,
    })))
}

fn parse_fn_def(cursor: &mut Cursor<'_>, is_async: bool) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = expect_ident(cursor)?;
    let generic_params = parse_generic_params_opt(cursor)?;
    let (params, variadic) = parse_params_list(cursor, "apos nome da funcao")?;
    let return_ann = parse_type_ann(cursor)?;
    let body = parse_block(cursor)?;
    Ok(Stmt::FnDef {
        name,
        params,
        variadic,
        return_ann,
        body,
        is_async,
        generic_params,
    })
}

/// `async fn ...` como statement; `async` fora disso e erro claro.
/// (Fase Next-Gen 3). `await` como statement so via expressao.
fn parse_async_or_await_stmt(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    // `async fn` => definicao assincrona; senao erro (evita `async` solto).
    if matches!(cursor.peek_at(1).map(|t| &t.kind), Some(TokenKind::Fn)) {
        cursor.next(); // `async`
        return parse_fn_def(cursor, true);
    }
    let tok = cursor.peek().cloned().expect("peek garantiu `async`");
    Err(ParseError::new(
        "esperado `fn` apos `async` (use `async fn nome() { }`)".to_string(),
        tok.line,
        tok.col,
    ))
}

/// `namespace A\B;` (Fase Next-Gen 2): prefixo do arquivo, nivel superior.
fn parse_namespace(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = parse_qualified_name(cursor)?;
    expect_semicolon(cursor)?;
    Ok(Stmt::Namespace { name })
}

/// `interface Nome<T> { fn m(params); ... }` (Fase Next-Gen 1).
fn parse_interface_def(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = parse_qualified_name(cursor)?;
    let generic_params = parse_generic_params_opt(cursor)?;
    expect_token(
        cursor,
        &TokenKind::LBrace,
        "esperado `{` apos nome da interface",
    )?;
    let mut methods = Vec::new();
    let mut seen = std::collections::HashSet::new();
    loop {
        match cursor.peek().map(|t| &t.kind) {
            None => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado `}` ou assinatura, encontrado fim de arquivo",
                    line,
                    col,
                ));
            }
            Some(TokenKind::RBrace) => {
                cursor.next();
                break;
            }
            Some(TokenKind::Fn) => {
                cursor.next();
                let mname = expect_ident(cursor)?;
                if !seen.insert(mname.clone()) {
                    let (line, col) = cursor.end_pos();
                    return Err(ParseError::new(
                        format!("metodo duplicado `{mname}` na interface `{name}`"),
                        line,
                        col,
                    ));
                }
                let (params, variadic) = parse_params_list(cursor, "apos nome do metodo")?;
                let return_ann = parse_type_ann(cursor)?;
                expect_semicolon(cursor)?;
                methods.push(InterfaceMethod {
                    name: mname,
                    params,
                    variadic,
                    return_ann,
                });
            }
            Some(other) => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    format!(
                        "no corpo da interface so `fn` (encontrado {})",
                        describe(other)
                    ),
                    line,
                    col,
                ));
            }
        }
    }
    Ok(Stmt::InterfaceDef {
        name,
        generic_params,
        methods,
    })
}

/// Corpo de metodo `fn` ou `async fn` em classe/trait (o `fn` ja consumido;
/// para `async fn`, o chamador consome `async` + `fn` antes).
/// `lugar` descreve o dono para erros (ex.: "na classe `Foo`").
fn parse_method_def(
    cursor: &mut Cursor<'_>,
    seen: &mut std::collections::HashSet<String>,
    lugar: &str,
    is_async: bool,
) -> Result<ClassMethod, ParseError> {
    let mname = expect_ident(cursor)?;
    if !seen.insert(mname.clone()) {
        let (line, col) = cursor.end_pos();
        return Err(ParseError::new(
            format!("metodo duplicado `{mname}` {lugar}"),
            line,
            col,
        ));
    }
    let (params, variadic) = parse_params_list(cursor, "apos nome do metodo")?;
    let return_ann = parse_type_ann(cursor)?;
    let body = parse_block(cursor)?;
    Ok(ClassMethod {
        name: mname,
        params,
        variadic,
        return_ann,
        body,
        is_async,
    })
}

/// `trait Nome { fn m() { } ... }` (Fase Next-Gen 1).
fn parse_trait_def(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = parse_qualified_name(cursor)?;
    expect_token(
        cursor,
        &TokenKind::LBrace,
        "esperado `{` apos nome do trait",
    )?;
    let mut methods = Vec::new();
    let mut seen = std::collections::HashSet::new();
    loop {
        match cursor.peek().map(|t| &t.kind) {
            None => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado `}` ou metodo, encontrado fim de arquivo",
                    line,
                    col,
                ));
            }
            Some(TokenKind::RBrace) => {
                cursor.next();
                break;
            }
            Some(TokenKind::Fn) => {
                cursor.next();
                let lugar = format!("no trait `{name}`");
                methods.push(parse_method_def(cursor, &mut seen, &lugar, false)?);
            }
            Some(TokenKind::Async)
                if matches!(cursor.peek_at(1).map(|t| &t.kind), Some(TokenKind::Fn)) =>
            {
                cursor.next(); // `async`
                cursor.next(); // `fn`
                let lugar = format!("no trait `{name}`");
                methods.push(parse_method_def(cursor, &mut seen, &lugar, true)?);
            }
            Some(other) => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    format!(
                        "no corpo do trait so `fn` ou `async fn` (encontrado {})",
                        describe(other)
                    ),
                    line,
                    col,
                ));
            }
        }
    }
    Ok(Stmt::TraitDef { name, methods })
}

/// `class Nome { fn m(params) { } ... }` (RFC 0017, Etapa 5).
/// Fase Next-Gen 1: `class Nome<T> extends Pai implements I1, I2 { use T1; fn ... }`.
/// So metodos `fn` e `use Trait;` no corpo; metodo duplicado = erro.
fn parse_class_def(cursor: &mut Cursor<'_>) -> Result<Stmt, ParseError> {
    cursor.next();
    let name = parse_qualified_name(cursor)?;
    let generic_params = parse_generic_params_opt(cursor)?;
    // `extends Pai` (opcional, unico).
    let mut parent = None;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Extends)) {
        cursor.next();
        parent = Some(parse_qualified_name(cursor)?);
    }
    // `implements I1, I2` (opcional).
    let mut interfaces = Vec::new();
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Implements)) {
        cursor.next();
        loop {
            interfaces.push(parse_qualified_name(cursor)?);
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                cursor.next();
                continue;
            }
            break;
        }
        if interfaces.is_empty() {
            let (line, col) = cursor.end_pos();
            return Err(ParseError::new(
                "esperado ao menos uma interface apos `implements`".to_string(),
                line,
                col,
            ));
        }
    }
    expect_token(
        cursor,
        &TokenKind::LBrace,
        "esperado `{` apos nome da classe",
    )?;
    let mut methods = Vec::new();
    let mut uses = Vec::new();
    let mut seen = std::collections::HashSet::new();
    loop {
        match cursor.peek().map(|t| &t.kind) {
            None => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    "esperado `}` ou metodo, encontrado fim de arquivo",
                    line,
                    col,
                ));
            }
            Some(TokenKind::RBrace) => {
                cursor.next();
                break;
            }
            Some(TokenKind::Use) => {
                cursor.next();
                // `use Trait;` ou `use T1, T2;`
                loop {
                    uses.push(parse_qualified_name(cursor)?);
                    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Comma)) {
                        cursor.next();
                        continue;
                    }
                    break;
                }
                expect_semicolon(cursor)?;
            }
            Some(TokenKind::Fn) => {
                cursor.next();
                let lugar = format!("na classe `{name}`");
                methods.push(parse_method_def(cursor, &mut seen, &lugar, false)?);
            }
            Some(TokenKind::Async)
                if matches!(cursor.peek_at(1).map(|t| &t.kind), Some(TokenKind::Fn)) =>
            {
                cursor.next(); // `async`
                cursor.next(); // `fn`
                let lugar = format!("na classe `{name}`");
                methods.push(parse_method_def(cursor, &mut seen, &lugar, true)?);
            }
            Some(other) => {
                let (line, col) = cursor.end_pos();
                return Err(ParseError::new(
                    format!(
                        "no corpo da classe so `fn`, `async fn` ou `use` (encontrado {})",
                        describe(other)
                    ),
                    line,
                    col,
                ));
            }
        }
    }
    // Validacao hierarquica basica: sem auto-heranca.
    if let Some(p) = parent.as_ref() {
        if p == &name {
            let (line, col) = cursor.end_pos();
            return Err(ParseError::new(
                format!("classe `{name}` nao pode herdar de si mesma"),
                line,
                col,
            ));
        }
    }
    Ok(Stmt::ClassDef {
        name,
        parent,
        interfaces,
        uses,
        generic_params,
        methods,
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
            let ann = parse_type_ann(cursor)?;
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
            params.push(Param { name, ann, default });
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
    // Nativa: `import native "ext";` (Fase Next-Gen 6).
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Native)) {
        cursor.next();
        let name = match cursor.next() {
            Some(t) => match &t.kind {
                TokenKind::StringLiteral(s) => s.clone(),
                TokenKind::Ident(s) => s.clone(),
                other => {
                    return Err(ParseError::new(
                        format!(
                            "esperado nome da extensao apos `native`, encontrado {}",
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
                    "esperado nome da extensao apos `native`".to_string(),
                    line,
                    col,
                ));
            }
        };
        expect_semicolon(cursor)?;
        return Ok(Stmt::ImportNative { name });
    }
    // Qualificado por namespace: `import A\B\C;` ou `import A\B as C;`
    // (Fase Next-Gen 2). Distingue de string pelo primeiro token ser Ident.
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Ident(_))) {
        let path = parse_qualified_name(cursor)?;
        // Alias opcional: `as Nome` (contextual).
        let mut alias = None;
        let is_as =
            matches!(cursor.peek(), Some(t) if matches!(&t.kind, TokenKind::Ident(w) if w == "as"));
        if is_as {
            cursor.next();
            alias = Some(parse_qualified_name(cursor)?);
        }
        // `import A\B from "x"` nao existe; qualificado termina com `;`.
        expect_semicolon(cursor)?;
        return Ok(Stmt::ImportQualified { path, alias });
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
        Some(TokenKind::Fn) => parse_fn_def(cursor, false)?,
        Some(TokenKind::Async) => parse_async_or_await_stmt(cursor)?,
        Some(TokenKind::Let) => parse_let(cursor)?,
        Some(TokenKind::Const) => parse_const(cursor)?,
        Some(TokenKind::Class) => parse_class_def(cursor)?,
        Some(TokenKind::Interface) => parse_interface_def(cursor)?,
        Some(TokenKind::Trait) => parse_trait_def(cursor)?,
        _ => {
            return Err(ParseError::new(
                "`export` so admite `fn`, `let` ou `const`",
                line,
                col,
            ));
        }
    };
    Ok(Stmt::Export {
        def: Box::new(StmtNode::from(def)),
    })
}

fn parse_block(cursor: &mut Cursor<'_>) -> Result<Vec<StmtNode>, ParseError> {
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
        let then_branch = cursor.nest("ramo do ternario", parse_ternary)?;
        expect_token(cursor, &TokenKind::Colon, "esperado `:` no ternario")?;
        let else_branch = cursor.nest("ramo do ternario", parse_ternary)?;
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

/// Resto do `??` apos o operador (associativo a direita via `nest`, com
/// teto — sem isso, `a ?? b ?? ...` arbitrariamente longo estoura a
/// pilha nativa; ver RHP-AUD-001).
fn parse_null_coalesce_rest(cursor: &mut Cursor<'_>, left: Expr) -> Result<Expr, ParseError> {
    let right = cursor.nest("operador `??`", parse_null_coalesce)?;
    Ok(Expr::Binary {
        op: BinOp::NullCoalesce,
        left: Box::new(left),
        right: Box::new(right),
    })
}

/// `||` com curto-circuito (RFC 0031): `a || b` avalia `b` so se `a`
/// for falso. Precedencia: abaixo do ternario, acima de `&&`.
fn parse_logical_or(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_logical_and(cursor)?;
    let mut chain = 0u32;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LogicalOr)) {
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
        let right = parse_logical_and(cursor)?;
        expr = Expr::Binary {
            op: BinOp::LogicalOr,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

/// `&&` com curto-circuito (RFC 0031): `a && b` avalia `b` so se `a`
/// for verdadeiro. Precedencia: acima de `||`, abaixo de `??`.
fn parse_logical_and(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_equality(cursor)?;
    let mut chain = 0u32;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LogicalAnd)) {
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
        let right = parse_equality(cursor)?;
        expr = Expr::Binary {
            op: BinOp::LogicalAnd,
            left: Box::new(expr),
            right: Box::new(right),
        };
    }
    Ok(expr)
}

fn parse_null_coalesce(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    // `??` e o mais frouxo dos tres (RFC 0031, como PHP/C#): os operandos
    // sao `||` (que contem `&&`), entao `a ?? b || c` = `a ?? (b || c)`.
    let left = parse_logical_or(cursor)?;
    if matches!(
        cursor.peek().map(|t| &t.kind),
        Some(TokenKind::QuestionQuestion)
    ) {
        cursor.next();
        // Associativo a direita: `a ?? b ?? c` = `a ?? (b ?? c)`.
        return parse_null_coalesce_rest(cursor, left);
    }
    Ok(left)
}

fn parse_equality(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_bitor(cursor)?;
    let mut chain = 0u32;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::EqEq) => BinOp::Eq,
            Some(TokenKind::BangEq) => BinOp::Ne,
            _ => break,
        };
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
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
    let mut chain = 0u32;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::BitOr)) {
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
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
    let mut chain = 0u32;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::BitXor)) {
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
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
    let mut chain = 0u32;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::BitAnd)) {
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
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
    let mut chain = 0u32;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Lt) => BinOp::Lt,
            Some(TokenKind::LtEq) => BinOp::Le,
            Some(TokenKind::Gt) => BinOp::Gt,
            Some(TokenKind::GtEq) => BinOp::Ge,
            _ => break,
        };
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
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
    let mut chain = 0u32;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Shl) => BinOp::Shl,
            Some(TokenKind::Shr) => BinOp::Shr,
            _ => break,
        };
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
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
    let mut chain = 0u32;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Plus) => BinOp::Add,
            Some(TokenKind::Minus) => BinOp::Sub,
            _ => break,
        };
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
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
    let mut chain = 0u32;
    loop {
        let op = match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::Star) => BinOp::Mul,
            Some(TokenKind::Slash) => BinOp::Div,
            Some(TokenKind::Percent) => BinOp::Mod,
            _ => break,
        };
        cursor.next();
        chain += 1;
        cursor.check_chain(chain, "expressao")?;
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
    // Recursao por operador contida por `nest` (teto de profundidade):
    // sem isso, `!!!...x` arbitrariamente longo estoura a pilha nativa
    // (RHP-AUD-001). Cada prefixo consome um nivel, como um aninhamento.
    match cursor.peek().map(|t| &t.kind) {
        Some(TokenKind::Bang) => {
            cursor.next();
            Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(cursor.nest("operador unario", parse_unary)?),
            })
        }
        Some(TokenKind::Minus) => {
            cursor.next();
            Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(cursor.nest("operador unario", parse_unary)?),
            })
        }
        Some(TokenKind::BitNot) => {
            cursor.next();
            Ok(Expr::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(cursor.nest("operador unario", parse_unary)?),
            })
        }
        // `+x` e identidade (compativel; sem no dedicado na AST).
        Some(TokenKind::Plus) => {
            cursor.next();
            cursor.nest("operador unario", parse_unary)
        }
        // `await expr` (Fase Next-Gen 3): maior precedencia unaria.
        Some(TokenKind::Await) => {
            cursor.next();
            Ok(Expr::Await {
                expr: Box::new(cursor.nest("operador unario", parse_unary)?),
            })
        }
        _ => parse_postfix(cursor),
    }
}

fn parse_postfix(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let mut expr = parse_primary(cursor)?;
    let mut chain = 0u32;
    loop {
        match cursor.peek().map(|t| &t.kind) {
            Some(TokenKind::LBracket) => {
                chain += 1;
                cursor.check_chain(chain, "expressao")?;
                expr = parse_index_suffix(cursor, expr)?;
            }
            Some(TokenKind::LParen) => {
                chain += 1;
                cursor.check_chain(chain, "expressao")?;
                expr = parse_call_suffix(cursor, expr)?;
            }
            Some(TokenKind::Dot) => {
                chain += 1;
                cursor.check_chain(chain, "expressao")?;
                expr = parse_member_suffix(cursor, expr)?;
            }
            _ => break,
        }
    }
    Ok(expr)
}

/// Corpo do sufixo `.nome` (RFC 0017): acesso a campo/metodo.
#[inline(never)]
fn parse_member_suffix(cursor: &mut Cursor<'_>, obj: Expr) -> Result<Expr, ParseError> {
    cursor.next();
    let tok = cursor.next().ok_or_else(|| {
        let (line, col) = cursor.end_pos();
        ParseError::new("esperado nome apos `.`", line, col)
    })?;
    match &tok.kind {
        TokenKind::Ident(name) => Ok(Expr::Member {
            obj: Box::new(obj),
            field: name.clone(),
        }),
        other => Err(ParseError::new(
            format!("esperado nome apos `.`, encontrado {}", describe(other)),
            tok.line,
            tok.col,
        )),
    }
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
        Expr::Var(name) => Expr::Call(Box::new(CallData {
            name,
            type_args: Vec::new(),
            args,
        })),
        callee => Expr::CallValue {
            callee: Box::new(callee),
            args,
        },
    })
}

/// `Ident` qualificado ou chamada `f<T>(...)` (fora do caminho quente).
#[inline(never)]
fn parse_ident_or_call(cursor: &mut Cursor<'_>, first: String) -> Result<Expr, ParseError> {
    let mut name = first;
    while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Backslash)) {
        cursor.next();
        name.push('\\');
        name.push_str(&expect_ident(cursor)?);
    }
    // Argumentos de tipo `f<T>(...)`: guardados para checagem (item bounds).
    let type_args = parse_type_args_opt(cursor)?;
    if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
        let args = parse_call_args(cursor)?;
        Ok(Expr::Call(Box::new(CallData {
            name,
            type_args,
            args,
        })))
    } else {
        Ok(Expr::Var(name))
    }
}

/// `async fn (...) { }` como valor (fora do caminho quente).
#[inline(never)]
fn parse_async_closure(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    match cursor.peek().map(|t| &t.kind) {
        Some(TokenKind::Fn) => {
            cursor.next();
            let (params, variadic) =
                parse_params_list(cursor, "apos `fn` (funcao anonima usa `fn (args) { }`)")?;
            let return_ann = parse_type_ann(cursor)?;
            if matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::FatArrow)) {
                cursor.next();
                let value = parse_expr(cursor)?;
                return Ok(Expr::Closure(Box::new(ClosureData {
                    params,
                    variadic,
                    return_ann,
                    body: vec![StmtNode::from(Stmt::Return { expr: Some(value) })],
                    is_async: true,
                })));
            }
            let body = parse_block(cursor)?;
            Ok(Expr::Closure(Box::new(ClosureData {
                params,
                variadic,
                return_ann,
                body,
                is_async: true,
            })))
        }
        _ => {
            let (line, col) = cursor.end_pos();
            Err(ParseError::new(
                "esperado `fn` apos `async` em expressao".to_string(),
                line,
                col,
            ))
        }
    }
}

/// `new Nome<T>(args)` (fora do caminho quente).
#[inline(never)]
fn parse_new_expr(cursor: &mut Cursor<'_>) -> Result<Expr, ParseError> {
    let tok = cursor.next().ok_or_else(|| {
        let (line, col) = cursor.end_pos();
        ParseError::new("esperado nome da classe apos `new`", line, col)
    })?;
    match &tok.kind {
        TokenKind::Ident(first) => {
            let mut name = first.clone();
            while matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::Backslash)) {
                cursor.next();
                name.push('\\');
                name.push_str(&expect_ident(cursor)?);
            }
            // Argumentos de tipo `new Caixa<T>(...)`: guardados (item bounds).
            let type_args = parse_type_args_opt(cursor)?;
            if !matches!(cursor.peek().map(|t| &t.kind), Some(TokenKind::LParen)) {
                return Err(ParseError::new(
                    "esperado `(` apos nome da classe".to_string(),
                    tok.line,
                    tok.col,
                ));
            }
            let args = parse_call_args(cursor)?;
            Ok(Expr::New(Box::new(NewData {
                class: Box::new(Expr::Var(name)),
                type_args,
                args,
            })))
        }
        other => Err(ParseError::new(
            format!(
                "esperado nome da classe apos `new`, encontrado {}",
                describe(other)
            ),
            tok.line,
            tok.col,
        )),
    }
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
        TokenKind::Ident(name) => parse_ident_or_call(cursor, name.clone()),
        TokenKind::Async => parse_async_closure(cursor),
        TokenKind::Super => Ok(Expr::Super),
        TokenKind::Await => {
            // `await` em posicao primaria (ex.: `await f()` sem unario previo).
            Ok(Expr::Await {
                expr: Box::new(parse_unary(cursor)?),
            })
        }
        TokenKind::Fn => {
            // `tok` ja consumiu `fn`; `(` vem a seguir ou e erro claro.
            let (params, variadic) =
                parse_params_list(cursor, "apos `fn` (funcao anonima usa `fn (args) { }`)")?;
            let mut closure = parse_fn_body(cursor, params, variadic)?;
            // `parse_fn_body` cria `is_async: false`; mantem aqui.
            if let Expr::Closure(data) = &mut closure {
                data.is_async = false;
            }
            Ok(closure)
        }
        TokenKind::New => parse_new_expr(cursor),
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
        TokenKind::Yield => "`yield`".to_string(),
        TokenKind::Export => "`export`".to_string(),
        TokenKind::Fn => "`fn`".to_string(),
        TokenKind::Const => "`const`".to_string(),
        TokenKind::Static => "`static`".to_string(),
        TokenKind::Break => "`break`".to_string(),
        TokenKind::Continue => "`continue`".to_string(),
        TokenKind::Return => "`return`".to_string(),
        TokenKind::Import => "`import`".to_string(),
        TokenKind::Class => "`class`".to_string(),
        TokenKind::New => "`new`".to_string(),
        TokenKind::Extends => "`extends`".to_string(),
        TokenKind::Implements => "`implements`".to_string(),
        TokenKind::Interface => "`interface`".to_string(),
        TokenKind::Trait => "`trait`".to_string(),
        TokenKind::Use => "`use`".to_string(),
        TokenKind::Super => "`super`".to_string(),
        TokenKind::Async => "`async`".to_string(),
        TokenKind::Await => "`await`".to_string(),
        TokenKind::Namespace => "`namespace`".to_string(),
        TokenKind::Native => "`native`".to_string(),
        TokenKind::Backslash => "`\\`".to_string(),
        TokenKind::Dot => "`.`".to_string(),
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
        TokenKind::LogicalAnd => "`&&`".to_string(),
        TokenKind::LogicalOr => "`||`".to_string(),
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
        // Inalcancavel na pratica (o `Cursor` filtra comentarios), mas o
        // match precisa ser exaustivo.
        TokenKind::Comment(_) => "comentario".to_string(),
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
            program.statements,
            vec![Stmt::Echo {
                expr: Expr::StringLiteral("Hello from RHP!".to_string()),
            }],
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
            program.statements[0].stmt,
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
        assert!(matches!(program.statements[0].stmt, Stmt::Echo { expr: _ }));
    }

    #[test]
    fn parses_let_assign_and_vars() {
        let program = lex_and_parse("let x = 1; x = x + 2; echo x;").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![
                Stmt::Let {
                    name: "x".to_string(),
                    ann: None,
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
                ann: None,
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
            program.statements[0].stmt,
            Stmt::If {
                else_branch: Some(_),
                ..
            }
        ));
        assert!(matches!(program.statements[1].stmt, Stmt::While { .. }));
    }

    #[test]
    fn parses_for_in_loop() {
        let program = lex_and_parse("for x in [1, 2] { echo x; }").expect("parse deve passar");
        assert_eq!(
            program.statements,
            vec![Stmt::For {
                var: "x".to_string(),
                iter: Expr::Array(vec![Expr::Int(1), Expr::Int(2)]),
                body: vec![StmtNode::from(Stmt::Echo {
                    expr: Expr::Var("x".to_string()),
                })],
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
                        body: vec![StmtNode::from(Stmt::Echo {
                            expr: Expr::StringLiteral("a".to_string()),
                        })],
                    },
                    SwitchCase {
                        cond: None,
                        body: vec![StmtNode::from(Stmt::Echo {
                            expr: Expr::StringLiteral("d".to_string()),
                        })],
                    },
                    SwitchCase {
                        cond: Some(Expr::Int(2)),
                        body: vec![StmtNode::from(Stmt::Echo {
                            expr: Expr::StringLiteral("b".to_string()),
                        })],
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
                        ann: None,
                        default: None,
                    },
                    Param {
                        name: "b".to_string(),
                        ann: None,
                        default: Some(Expr::Int(2)),
                    },
                    Param {
                        name: "c".to_string(),
                        ann: None,
                        default: Some(Expr::Binary {
                            op: BinOp::Add,
                            left: Box::new(Expr::Int(1)),
                            right: Box::new(Expr::Int(2)),
                        }),
                    },
                ],
                variadic: Some("rest".to_string()),
                return_ann: None,
                body: vec![StmtNode::from(Stmt::Return {
                    expr: Some(Expr::Var("a".to_string())),
                })],
                is_async: false,
                generic_params: vec![],
            }]
        );
        // Closure aceita a mesma forma.
        let program =
            lex_and_parse("let f = fn (a = \"x\") { return a; };").expect("parse deve passar");
        match &program.statements[0].stmt {
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
                expr: Expr::Call(Box::new(CallData {
                    name: "f".to_string(),
                    type_args: vec![],
                    args: vec![
                        CallArg::Pos(Expr::Int(1)),
                        CallArg::Named("x".to_string(), Expr::Int(2)),
                        CallArg::Spread(Expr::Var("r".to_string())),
                    ],
                }))
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
    fn parses_generic_bounds_and_type_args() {
        // Bounds: `T`, `U: Base`, qualificado `V: App\Base`.
        let program = lex_and_parse("fn f<T, U: Base, V: App\\Modelo>() { return 1; }")
            .expect("parse deve passar");
        match &program.statements[0].stmt {
            Stmt::FnDef { generic_params, .. } => {
                assert_eq!(generic_params.len(), 3);
                assert_eq!(generic_params[0].name, "T");
                assert_eq!(generic_params[0].bound, None);
                assert_eq!(generic_params[1].name, "U");
                assert_eq!(generic_params[1].bound.as_deref(), Some("Base"));
                assert_eq!(generic_params[2].name, "V");
                assert_eq!(generic_params[2].bound.as_deref(), Some("App\\Modelo"));
            }
            other => panic!("esperado FnDef, obteve {other:?}"),
        }
        // Type args preservados em chamada e `new`.
        let program = lex_and_parse("echo f<int, string>(1);").expect("parse deve passar");
        match &program.statements[0].stmt {
            Stmt::Echo {
                expr: Expr::Call(data),
            } => {
                assert_eq!(data.name, "f");
                assert_eq!(data.type_args, vec!["int", "string"]);
            }
            other => panic!("esperado Call, obteve {other:?}"),
        }
        let program = lex_and_parse("let c = new Caixa<Usuario>(1);").expect("parse deve passar");
        match &program.statements[0].stmt {
            Stmt::Let {
                init: Some(Expr::New(data)),
                ..
            } => {
                assert_eq!(data.type_args, vec!["Usuario"]);
            }
            other => panic!("esperado New, obteve {other:?}"),
        }
        // Duplicata de parametro de tipo = erro.
        let err = lex_and_parse("fn f<T, T>() { }").expect_err("deve falhar");
        assert!(err.to_string().contains("duplicado"), "obteve: {err}");
        // `<` binario continua comparacao (nao vira generics).
        lex_and_parse("echo a < b;").expect("comparacao deve passar");
        lex_and_parse("for let i = 0; i < n; i = i + 1 { echo i; }")
            .expect("for com `<` deve passar");
    }

    #[test]
    fn parses_async_methods() {
        // Metodos `async fn` em classe e trait (item metodos-async).
        let program = lex_and_parse(
            "class A { fn novo() { this[\"x\"] = 0; } async fn busca(id) { return id; } }",
        )
        .expect("parse deve passar");
        match &program.statements[0].stmt {
            Stmt::ClassDef { methods, .. } => {
                assert_eq!(methods.len(), 2);
                assert!(!methods[0].is_async);
                assert!(methods[1].is_async);
                assert_eq!(methods[1].name, "busca");
            }
            other => panic!("esperado ClassDef, obteve {other:?}"),
        }
        let program =
            lex_and_parse("trait T { async fn m() { return 1; } }").expect("parse deve passar");
        match &program.statements[0].stmt {
            Stmt::TraitDef { methods, .. } => {
                assert_eq!(methods.len(), 1);
                assert!(methods[0].is_async);
            }
            other => panic!("esperado TraitDef, obteve {other:?}"),
        }
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
                return_ann: None,
                body: vec![
                    StmtNode::from(Stmt::Static {
                        name: "n".to_string(),
                        init: Expr::Int(0),
                    }),
                    StmtNode::from(Stmt::Return {
                        expr: Some(Expr::Var("n".to_string())),
                    }),
                ],
                is_async: false,
                generic_params: vec![],
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
        match &program.statements[0].stmt {
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
        match &program.statements[0].stmt {
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
        assert!(matches!(program.statements[0].stmt, Stmt::ForC { .. }));
        let program = lex_and_parse("for ;; { break; }").expect("deve passar");
        assert!(matches!(program.statements[0].stmt, Stmt::ForC { .. }));
        // `for x in` continua for-in (lookahead preservado).
        let program = lex_and_parse("for x in [1] { echo x; }").expect("deve passar");
        assert!(matches!(program.statements[0].stmt, Stmt::For { .. }));
        // Niveis.
        let program = lex_and_parse("while true { break 2; }").expect("deve passar");
        assert!(matches!(program.statements[0].stmt, Stmt::While { .. }));
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
        assert!(matches!(program.statements[0].stmt, Stmt::Let { .. }));
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
        assert!(matches!(program.statements[0].stmt, Stmt::FnDef { .. }));
        assert!(matches!(program.statements[1].stmt, Stmt::Echo { .. }));
    }

    #[test]
    fn parses_anonymous_fn_and_general_calls() {
        // `let f = fn (x) { return x; };`
        let program = lex_and_parse("let f = fn (x) { return x; };").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Let {
                init: Some(Expr::Closure(_)),
                ..
            }
        ));
        // Chamada direta de literal: IIFE.
        let program = lex_and_parse("echo (fn (x) { return x; })(41);").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::CallValue { .. }
            }
        ));
        // `f(x)` direto continua `Call` (precedencia de builtins).
        let program = lex_and_parse("echo soma(1);").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::Call(_)
            }
        ));
        // Chamada encadeada e por indice: `CallValue`.
        let program = lex_and_parse("echo f(1)(2);").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
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
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::BitOr,
                    ..
                }
            }
        ));
        let program = lex_and_parse("echo 1 + 2 << 3;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::Binary { op: BinOp::Shl, .. }
            }
        ));
    }

    #[test]
    fn parses_ternary_and_null_coalescing() {
        let program = lex_and_parse("echo true ? 1 : 2;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::Ternary { .. }
            }
        ));
        let program = lex_and_parse("echo a ?? b;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::NullCoalesce,
                    ..
                }
            }
        ));
    }

    #[test]
    fn parses_logical_and_or_with_precedence() {
        // RFC 0031: `||` abaixo de `&&` abaixo de `??` abaixo de `==`.
        let program = lex_and_parse("echo a && b;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::LogicalAnd,
                    ..
                }
            }
        ));
        // `a || b && c` => Or(a, And(b, c)).
        let program = lex_and_parse("echo a || b && c;").expect("parse deve passar");
        match &program.statements[0].stmt {
            Stmt::Echo {
                expr:
                    Expr::Binary {
                        op: BinOp::LogicalOr,
                        right,
                        ..
                    },
            } => {
                assert!(
                    matches!(
                        right.as_ref(),
                        Expr::Binary {
                            op: BinOp::LogicalAnd,
                            ..
                        }
                    ),
                    "lado direito deveria ser &&: {right:?}"
                );
            }
            outra => panic!("forma inesperada: {outra:?}"),
        }
        // `a ?? b || c` => ??(a, Or(b, c)) — `??` o mais frouxo dos tres.
        let program = lex_and_parse("echo a ?? b || c;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::NullCoalesce,
                    ..
                }
            }
        ));
        // `&` sozinho continua bitwise (sem regressao).
        let program = lex_and_parse("echo a & b;").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Echo {
                expr: Expr::Binary {
                    op: BinOp::BitAnd,
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
        assert!(matches!(program.statements[0].stmt, Stmt::While { .. }));
        assert!(matches!(program.statements[2].stmt, Stmt::Const { .. }));
    }

    #[test]
    fn parses_import() {
        let program = lex_and_parse("import \"lib.rhp\"; echo 1;").expect("parse deve passar");
        assert_eq!(
            program.statements[0].stmt,
            Stmt::Import("lib.rhp".to_string())
        );
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
                    body: vec![StmtNode::from(Stmt::Echo { expr: Expr::Int(1) })],
                    catches: vec![CatchClause {
                        type_name: None,
                        var: Some("e".to_string()),
                        body: vec![StmtNode::from(Stmt::Echo {
                            expr: Expr::Var("e".to_string()),
                        })],
                    }],
                    finally_body: Some(vec![StmtNode::from(Stmt::Echo { expr: Expr::Int(2) })]),
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
            program.statements[0].stmt,
            Stmt::Try {
                finally_body: None,
                ..
            }
        ));
        let program =
            lex_and_parse("try { echo 1; } finally { echo 2; }").expect("parse deve passar");
        assert!(matches!(
            program.statements[0].stmt,
            Stmt::Try {
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
        assert!(matches!(program.statements[0].stmt, Stmt::Export { .. }));
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
        // Margem sob o teto (64): 50 niveis passam (medicao RHP-AUD-001:
        // ~120 ja estouram 2 MiB em debug; o teto tem folga real).
        let src = format!("echo {}1{};", "(".repeat(50), ")".repeat(50));
        lex_and_parse(&src).expect("50 niveis devem passar");
    }

    /// RHP-AUD-001: cadeias planas aprofundam a AST como aninhamento —
    /// sem teto, `1+1+...` estourava a pilha nativa (SIGABRT) em vez de
    /// erro limpo. Todas falham com E1000; nenhuma aborta o processo.
    #[test]
    fn rejects_excessive_flat_chains() {
        let casos: Vec<(String, &str)> = vec![
            (vec!["1"; 2000].join("+"), "soma longa"),
            (format!("echo {};", "!".repeat(2000) + "true"), "bangs"),
            (
                format!("echo {};", "true ? 1 : ".repeat(500) + "0"),
                "ternarios",
            ),
            (
                format!("echo {};", vec!["a"; 2000].join(" ?? ")),
                "coalescencia",
            ),
            (format!("echo m{};", ".x".repeat(2000)), "membros"),
            (format!("echo a{};", "[0]".repeat(2000)), "indices"),
            (format!("echo f{};", "()".repeat(2000)), "chamadas"),
            (
                format!("echo {};", vec!["1"; 200].join(" * ")),
                "multiplicacao longa",
            ),
        ];
        for (src, o_que) in &casos {
            let err = lex_and_parse(src).expect_err(&format!("cadeia plana deve falhar: {o_que}"));
            assert!(
                err.message.contains("profundidade maxima"),
                "{o_que}: {}",
                err.message
            );
        }
        // Dentro do teto, cadeias passam (inclui mistura aninhada).
        lex_and_parse(&format!("echo {};", vec!["1"; 50].join("+"))).expect("50 elos passam");
        lex_and_parse("echo true ? 1 : false ? 2 : 3;").expect("ternario curto passa");
        lex_and_parse("echo a ?? b;").expect("coalescencia curta passa");
    }
}
