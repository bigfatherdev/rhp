//! Diagnosticos estruturados do RHP.
//!
//! Separa o *dado* (codigo estavel, arquivo, posicao, mensagem, ajuda) da
//! *apresentacao* (`render` para terminal; o dado serve a futuro LSP).
//! Codigos sao estaveis e testados; categorias:
//! `E0xxx` lexicos, `E1xxx` sintaticos, `E2xxx` compilacao,
//! `E3xxx` execucao, `E4xxx` carregador/modulos.

use rhp_lexer::LexError;
use rhp_parser::ParseError;
use std::fmt;

/// Severidade (hoje so erros; avisos/notas entram com lints futuros).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Posicao com comprimento (`len == 0` = ate o fim da linha).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

impl Span {
    #[must_use]
    pub fn point(line: usize, col: usize) -> Self {
        Self { line, col, len: 1 }
    }

    #[must_use]
    pub fn to_end_of_line(line: usize, col: usize) -> Self {
        Self { line, col, len: 0 }
    }
}

/// Diagnostico estruturado e renderizavel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub path: String,
    pub span: Span,
    pub message: String,
    pub help: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn new(
        code: &'static str,
        path: impl Into<String>,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity: Severity::Error,
            path: path.into(),
            span,
            message: message.into(),
            help: None,
        }
    }

    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

/// Renderiza no formato de referencia (terminal).
///
/// ```text
/// Error[E1004]: esperava `;`
///   --> app.rhp:1:16
/// 1 | echo "x"
///   |                ^ esperado `;` aqui? (exemplo ilustrativo do formato)
/// help: toda instrucao termina com `;`
/// ```
///
/// Com `source = None`, imprime so cabecalho, localizacao e ajuda.
#[must_use]
pub fn render(diag: &Diagnostic, source: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}[{}]: {}\n",
        diag.severity, diag.code, diag.message
    ));
    out.push_str(&format!(
        "  --> {}:{}:{}\n",
        diag.path, diag.span.line, diag.span.col
    ));
    if let Some(snippet) = snippet_for(source, diag.span) {
        out.push_str(&snippet);
        out.push('\n');
    }
    if let Some(help) = &diag.help {
        out.push_str(&format!("help: {help}\n"));
    }
    out
}

fn snippet_for(source: Option<&str>, span: Span) -> Option<String> {
    let source = source?;
    let line_text = source.lines().nth(span.line.saturating_sub(1))?;
    let width = line_text.chars().count();
    let mut out = String::new();
    out.push_str(&format!("{} | {line_text}\n", span.line));
    let prefix = " ".repeat(span.line.to_string().len());
    let mut carets = String::new();
    let start = span.col.saturating_sub(1);
    for _ in 0..start {
        carets.push(' ');
    }
    let len = if span.len == 0 {
        width.saturating_sub(start).max(1)
    } else {
        span.len.min(width.saturating_sub(start).max(1))
    };
    for _ in 0..len {
        carets.push('^');
    }
    out.push_str(&format!("{prefix} | {carets}"));
    Some(out)
}

/// Mapeia erro lexico para diagnostico (codigo estavel + ajuda).
#[must_use]
pub fn lex_diagnostic(path: &str, err: &LexError) -> Diagnostic {
    let (code, span, help): (&'static str, Span, Option<&str>) =
        if err.message.contains("caractere inesperado") {
            (
                "E0001",
                Span::point(err.line, err.col),
                Some("use apenas caracteres suportados pela linguagem"),
            )
        } else if err.message.contains("nao terminada") {
            (
                "E0002",
                Span::to_end_of_line(err.line, err.col),
                Some("feche a string com `\"` antes do fim da linha"),
            )
        } else if err.message.contains("quebra de linha") {
            (
                "E0002",
                Span::point(err.line, err.col),
                Some("use `\\n` para quebras de linha dentro de strings"),
            )
        } else if err.message.contains("escape") {
            (
                "E0003",
                Span::point(err.line, err.col),
                Some("escapes validos: \\\\ \\n \\r \\t \\\" \\u{HEX}"),
            )
        } else {
            (
                "E0004",
                Span::point(err.line, err.col),
                Some("verifique o literal numerico"),
            )
        };
    let mut diag = Diagnostic::new(code, path, span, err.to_string());
    if let Some(help) = help {
        diag = diag.with_help(help);
    }
    diag
}

/// Mapeia erro sintatico para diagnostico (codigo estavel + ajuda).
#[must_use]
pub fn parse_diagnostic(path: &str, err: &ParseError) -> Diagnostic {
    let (code, help): (&'static str, Option<&str>) = if err.message.contains("alvo de atribuicao") {
        (
            "E1003",
            Some("atribua a `nome` ou `nome[indice]`, nao a expressoes"),
        )
    } else if err.message.contains("esperado `}`") {
        ("E1002", Some("feche o bloco com `}`"))
    } else if err.message.contains("esperado expressao") {
        (
            "E1001",
            Some("expressoes validas: literal, variavel, chamada, `(…)`"),
        )
    } else if err.message.contains("esperado `;`") {
        ("E1004", Some("toda instrucao termina com `;`"))
    } else if err.message.contains("esperado identificador") {
        (
            "E1005",
            Some("palavras-chave nao podem ser usadas como nomes"),
        )
    } else if err.message.contains("caminho de modulo") || err.message.contains("nivel superior") {
        (
            "E1007",
            Some("`import \"arquivo.rhp\";` apenas no nivel superior"),
        )
    } else if err.message.contains("esperado") {
        (
            "E1006",
            Some("confira a sintaxe em docs/LANGUAGE_REFERENCE.md"),
        )
    } else {
        ("E1000", None)
    };
    let span = if err.message.contains("fim de arquivo") {
        Span::to_end_of_line(err.line, err.col)
    } else {
        Span::point(err.line, err.col)
    };
    let mut diag = Diagnostic::new(code, path, span, err.to_string());
    if let Some(help) = help {
        diag = diag.with_help(help);
    }
    diag
}

/// Classifica erro de compilacao (mensagem ja formatada; sem posicao de
/// origem no bytecode — o diagnostico carrega so codigo e ajuda).
#[must_use]
pub fn compile_code(message: &str) -> (&'static str, Option<&'static str>) {
    if message.contains("fora de funcao") {
        ("E2001", Some("`return` so existe dentro de `fn`"))
    } else if message.contains("espera") && message.contains("argumento") {
        ("E2002", Some("confira a aridade do builtin/funcao"))
    } else if message.contains("limite") {
        ("E2003", Some("reduza constantes/nomes/funcoes do programa"))
    } else if message.contains("profundidade maxima") {
        ("E2003", Some("reduza o aninhamento (limite 128 niveis)"))
    } else if message.contains("invalido") || message.contains("fora do") {
        ("E2004", Some("bytecode inconsistente — reporte como bug"))
    } else {
        ("E2000", None)
    }
}

/// Classifica erro de runtime (mensagem ja formatada; sem posicao de
/// origem — a VM executa bytecode, nao fonte).
#[must_use]
pub fn runtime_code(message: &str) -> (&'static str, Option<&'static str>) {
    if message.contains("incompativeis")
        || message.contains("exige")
        || message.contains("nao converte")
    {
        (
            "E3001",
            Some("converta explicitamente com int()/float()/str()"),
        )
    } else if message.contains("nao definida") || message.contains("ja declarada") {
        (
            "E3002",
            Some("declare com `let` antes de usar; evite redeclare"),
        )
    } else if message.contains("deve ser bool") {
        ("E3003", Some("use comparacao explicita, ex.: `if x != 0`"))
    } else if message.contains("como funcao")
        || message.contains("espera")
        || message.contains("aridade")
    {
        ("E3004", Some("confira nome e numero de argumentos"))
    } else if message.contains("fora de funcao") {
        ("E3001", Some("`return` so existe dentro de `fn`"))
    } else if message.contains("limite de chamadas") {
        (
            "E3006",
            Some("reduza a profundidade de recursao (limite 512)"),
        )
    } else if message.contains("fora do intervalo") || message.contains("chave inexistente") {
        (
            "E3007",
            Some("verifique tamanho com len()/keys() antes de indexar"),
        )
    } else if message.contains("nao e UTF-8") || message.contains("escrever saida") {
        ("E3008", Some("verifique a saida/encoding"))
    } else if message.contains("modulo")
        || message.contains("circular")
        || message.contains("fora da arvore")
        || message.contains("muito grande")
        || message.contains("nivel superior")
    {
        (
            "E3009",
            Some("confira imports em docs/LANGUAGE_REFERENCE.md"),
        )
    } else if message.contains("limite de instrucoes") || message.contains("limite de corpo") {
        (
            "E3010",
            Some("aumente o teto ou reduza o trabalho (ver --max-steps)"),
        )
    } else if message.contains("http_get") {
        ("E3011", Some("use http://, 2xx e corpo UTF-8 ate 1 MiB"))
    } else {
        ("E3000", None)
    }
}

/// Diagnostico de compilacao/runtime sem posicao de origem.
#[must_use]
pub fn code_diagnostic(
    code: &'static str,
    help: Option<&'static str>,
    origin: &str,
    message: &str,
) -> Diagnostic {
    let mut diag = Diagnostic::new(
        code,
        origin,
        Span {
            line: 1,
            col: 1,
            len: 0,
        },
        message.to_string(),
    );
    if let Some(help) = help {
        diag = diag.with_help(help);
    }
    diag
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhp_parser::parse_source;

    fn lex_err(src: &str) -> LexError {
        rhp_lexer::lex(src).expect_err("deve falhar no lexer")
    }

    fn parse_err(src: &str) -> ParseError {
        let tokens = rhp_lexer::lex(src).expect("lex deve passar");
        rhp_parser::parse(&tokens).expect_err("deve falhar no parser")
    }

    #[test]
    fn renders_reference_format_with_snippet() {
        let diag = Diagnostic::new(
            "E1004",
            "app.rhp",
            Span {
                line: 1,
                col: 9,
                len: 1,
            },
            "1:9: esperado `;`, encontrado fim de arquivo",
        )
        .with_help("toda instrucao termina com `;`");
        let out = render(&diag, Some("echo \"x\""));
        let expected = "Error[E1004]: 1:9: esperado `;`, encontrado fim de arquivo\n  --> app.rhp:1:9\n1 | echo \"x\"\n  |         ^\nhelp: toda instrucao termina com `;`\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn renders_without_source() {
        let diag = Diagnostic::new("E3000", "<teste>", Span::point(1, 1), "boom");
        assert_eq!(
            render(&diag, None),
            "Error[E3000]: boom\n  --> <teste>:1:1\n"
        );
    }

    #[test]
    fn maps_lexical_errors() {
        assert_eq!(lex_diagnostic("a.rhp", &lex_err("@")).code, "E0001");
        assert_eq!(lex_diagnostic("a.rhp", &lex_err("\"abc")).code, "E0002");
        assert_eq!(lex_diagnostic("a.rhp", &lex_err("\"\\q\";")).code, "E0003");
        assert_eq!(
            lex_diagnostic("a.rhp", &lex_err("99999999999999999999")).code,
            "E0004"
        );
    }

    #[test]
    fn maps_syntax_errors() {
        assert_eq!(
            parse_diagnostic("a.rhp", &parse_err("echo 1")).code,
            "E1004"
        );
        assert_eq!(
            parse_diagnostic("a.rhp", &parse_err("echo ;")).code,
            "E1001"
        );
        assert_eq!(
            parse_diagnostic("a.rhp", &parse_err("if x {")).code,
            "E1002"
        );
        assert_eq!(
            parse_diagnostic("a.rhp", &parse_err("f() = 1;")).code,
            "E1003"
        );
        assert_eq!(
            parse_diagnostic("a.rhp", &parse_err("let echo = 1;")).code,
            "E1005"
        );
        assert_eq!(
            parse_diagnostic("a.rhp", &parse_err("import 1;")).code,
            "E1007"
        );
        let _ = parse_source("echo 1;");
    }

    #[test]
    fn maps_compile_and_runtime_codes() {
        assert_eq!(compile_code("`return` fora de funcao").0, "E2001");
        assert_eq!(compile_code("chunk invalido: x").0, "E2004");
        assert_eq!(runtime_code("tipos incompativeis").0, "E3001");
        assert_eq!(runtime_code("variavel nao definida: x").0, "E3002");
        assert_eq!(runtime_code("deve ser bool").0, "E3003");
        assert_eq!(runtime_code("como funcao").0, "E3004");
        assert_eq!(runtime_code("limite de chamadas").0, "E3006");
        assert_eq!(runtime_code("fora do intervalo").0, "E3007");
        assert_eq!(runtime_code("importacao circular").0, "E3009");
        assert_eq!(runtime_code("limite de instrucoes").0, "E3010");
        assert_eq!(runtime_code("http_get falhou").0, "E3011");
        assert_eq!(runtime_code("algo totalmente novo").0, "E3000");
    }
}
