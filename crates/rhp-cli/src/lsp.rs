//! Servidor LSP minimo do RHP (Etapa 4, DevX): `rhp lsp`.
//!
//! Fala JSON-RPC 2.0 sobre stdio com enquadramento `Content-Length`
//! (o mesmo do protocolo LSP) e publica `textDocument/publishDiagnostics`
//! a cada `didOpen`/`didChange` (erros lexicos e sintaticos com posicao).
//! Sem dependencias externas: parser JSON minimo proprio, so para as
//! formas usadas aqui (objetos, arrays, strings com escapes, numeros,
//! `true`/`false`/`null`). Sem `unsafe`.

use std::collections::HashMap;
use std::io::{BufRead, Write};

/// Valor JSON (suficiente para o subset LSP usado aqui).
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Escapa string para emissao JSON (`"` envolve fora).
fn escape_into(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn render(value: &Json, out: &mut String) {
    match value {
        Json::Null => out.push_str("null"),
        Json::Bool(true) => out.push_str("true"),
        Json::Bool(false) => out.push_str("false"),
        Json::Num(raw) => out.push_str(raw),
        Json::Str(s) => escape_into(out, s),
        Json::Arr(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                render(item, out);
            }
            out.push(']');
        }
        Json::Obj(entries) => {
            out.push('{');
            for (i, (key, value)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                escape_into(out, key);
                out.push(':');
                render(value, out);
            }
            out.push('}');
        }
    }
}

/// Parser JSON minimo (objetos, arrays, strings com escapes incl. `\u`,
/// numeros, `true`/`false`/`null`; rejeita o resto com posicao).
struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub message: String,
    pub pos: usize,
}

fn json_error(message: impl Into<String>, pos: usize) -> JsonError {
    JsonError {
        message: message.into(),
        pos,
    }
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), JsonError> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(json_error(format!("esperado `{}`", byte as char), self.pos))
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, JsonError> {
        if self.input.get(self.pos..self.pos + word.len()) == Some(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(json_error(
                format!("literal invalido (esperado `{word}`)"),
                self.pos,
            ))
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| json_error("string nao terminada", self.pos))?;
            self.pos += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let esc = self
                        .peek()
                        .ok_or_else(|| json_error("escape incompleto", self.pos))?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let mut code: u32 = 0;
                            for _ in 0..4 {
                                let h = self
                                    .peek()
                                    .ok_or_else(|| json_error("escape \\u incompleto", self.pos))?;
                                self.pos += 1;
                                let digit = (h as char).to_digit(16).ok_or_else(|| {
                                    json_error("hexadecimal invalido em \\u", self.pos)
                                })?;
                                code = code * 16 + digit;
                            }
                            out.push(char::from_u32(code).ok_or_else(|| {
                                json_error("codepoint invalido em \\u", self.pos)
                            })?);
                        }
                        _ => return Err(json_error("escape desconhecido", self.pos)),
                    }
                }
                _ => {
                    // UTF-8: consome a sequencia completa do primeiro byte.
                    let width = utf8_width(byte)
                        .ok_or_else(|| json_error("byte UTF-8 invalido", self.pos - 1))?;
                    let start = self.pos - 1;
                    for _ in 1..width {
                        let cont = self
                            .peek()
                            .ok_or_else(|| json_error("sequencia UTF-8 truncada", self.pos))?;
                        if cont & 0xC0 != 0x80 {
                            return Err(json_error("continuacao UTF-8 invalida", self.pos));
                        }
                        self.pos += 1;
                    }
                    let text = std::str::from_utf8(&self.input[start..self.pos])
                        .map_err(|_| json_error("UTF-8 invalido em string", start))?;
                    out.push_str(text);
                }
            }
        }
    }

    fn number(&mut self) -> Result<Json, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let mut digits = 0;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
            digits += 1;
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
                digits += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return Err(json_error("numero invalido", start));
        }
        let raw = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| json_error("numero invalido", start))?;
        Ok(Json::Num(raw.to_string()))
    }

    fn value(&mut self) -> Result<Json, JsonError> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => {
                self.pos += 1;
                let mut entries = Vec::new();
                self.skip_ws();
                if self.peek() == Some(b'}') {
                    self.pos += 1;
                    return Ok(Json::Obj(entries));
                }
                loop {
                    self.skip_ws();
                    if self.peek() != Some(b'"') {
                        return Err(json_error("chave de objeto deve ser string", self.pos));
                    }
                    let key = self.string()?;
                    self.skip_ws();
                    self.expect(b':')?;
                    let value = self.value()?;
                    entries.push((key, value));
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => {
                            self.pos += 1;
                        }
                        Some(b'}') => {
                            self.pos += 1;
                            return Ok(Json::Obj(entries));
                        }
                        _ => return Err(json_error("esperado `,` ou `}`", self.pos)),
                    }
                }
            }
            Some(b'[') => {
                self.pos += 1;
                let mut items = Vec::new();
                self.skip_ws();
                if self.peek() == Some(b']') {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                loop {
                    items.push(self.value()?);
                    self.skip_ws();
                    match self.peek() {
                        Some(b',') => {
                            self.pos += 1;
                        }
                        Some(b']') => {
                            self.pos += 1;
                            return Ok(Json::Arr(items));
                        }
                        _ => return Err(json_error("esperado `,` ou `]`", self.pos)),
                    }
                }
            }
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(json_error("valor JSON invalido", self.pos)),
        }
    }
}

/// Largura em bytes da sequencia UTF-8 iniciada por `byte`.
fn utf8_width(byte: u8) -> Option<usize> {
    if byte < 0x80 {
        Some(1)
    } else if byte & 0xE0 == 0xC0 {
        Some(2)
    } else if byte & 0xF0 == 0xE0 {
        Some(3)
    } else if byte & 0xF8 == 0xF0 {
        Some(4)
    } else {
        None
    }
}

/// Analisa um documento JSON completo (sem sobra alem de espacos).
pub fn parse_json(input: &[u8]) -> Result<Json, JsonError> {
    let mut parser = Parser::new(input);
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos != input.len() {
        return Err(json_error("sobra apos o documento JSON", parser.pos));
    }
    Ok(value)
}

/// Diagnostico LSP (faixa 0-based; RHP so tem ponto inicial).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

/// Extrai diagnosticos do fonte (lex + parse; vazio = limpo).
pub fn diagnose(source: &str) -> Vec<Diagnostic> {
    let tokens = match rhp_lexer::lex(source) {
        Ok(tokens) => tokens,
        Err(e) => {
            return vec![Diagnostic {
                line: e.line,
                col: e.col,
                message: e.message,
            }]
        }
    };
    match rhp_parser::parse(&tokens) {
        Ok(_) => Vec::new(),
        Err(e) => vec![Diagnostic {
            line: e.line,
            col: e.col,
            message: e.message,
        }],
    }
}

fn obj(entries: Vec<(&str, Json)>) -> Json {
    Json::Obj(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}

/// Le uma mensagem com enquadramento `Content-Length` (`None` = EOF).
fn read_message(input: &mut impl BufRead) -> Result<Option<Vec<u8>>, String> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = input
            .read_line(&mut line)
            .map_err(|e| format!("falha lendo stdin do LSP: {e}"))?;
        if n == 0 {
            return Ok(None);
        }
        let line = line.trim();
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            content_length = Some(
                rest.trim()
                    .parse()
                    .map_err(|_| format!("Content-Length invalido: `{rest}`"))?,
            );
        }
    }
    let len = content_length.ok_or_else(|| "mensagem sem Content-Length".to_string())?;
    let mut buf = vec![0u8; len];
    input
        .read_exact(&mut buf)
        .map_err(|e| format!("corpo LSP truncado ({len} bytes): {e}"))?;
    Ok(Some(buf))
}

fn write_message(output: &mut impl Write, body: &str) -> Result<(), String> {
    write!(output, "Content-Length: {}\r\n\r\n{body}", body.len())
        .and_then(|()| output.flush())
        .map_err(|e| format!("falha escrevendo resposta LSP: {e}"))
}

fn respond(output: &mut impl Write, id: &Json, result: Json) -> Result<(), String> {
    let mut id_raw = String::new();
    render(id, &mut id_raw);
    let mut body = String::new();
    body.push_str("{\"jsonrpc\":\"2.0\",\"id\":");
    body.push_str(&id_raw);
    body.push_str(",\"result\":");
    render(&result, &mut body);
    body.push('}');
    write_message(output, &body)
}

fn respond_error(
    output: &mut impl Write,
    id: &Json,
    code: i64,
    message: &str,
) -> Result<(), String> {
    let mut id_raw = String::new();
    render(id, &mut id_raw);
    let mut errmsg = String::new();
    escape_into(&mut errmsg, message);
    let body = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id_raw},\"error\":{{\"code\":{code},\"message\":{errmsg}}}}}"
    );
    write_message(output, &body)
}

fn notify(output: &mut impl Write, method: &str, params: Json) -> Result<(), String> {
    let mut body = String::new();
    body.push_str("{\"jsonrpc\":\"2.0\",\"method\":");
    escape_into(&mut body, method);
    body.push_str(",\"params\":");
    render(&params, &mut body);
    body.push('}');
    write_message(output, &body)
}

fn publish_diagnostics(
    output: &mut impl Write,
    uri: &str,
    diagnostics: &[Diagnostic],
) -> Result<(), String> {
    let items: Vec<Json> = diagnostics
        .iter()
        .map(|d| {
            // RHP informa ponto inicial (base 1); faixa cobre 1 caractere.
            let line = d.line.saturating_sub(1) as i64;
            let col = d.col.saturating_sub(1) as i64;
            obj(vec![
                (
                    "range",
                    obj(vec![
                        (
                            "start",
                            obj(vec![
                                ("line", Json::Num(line.to_string())),
                                ("character", Json::Num(col.to_string())),
                            ]),
                        ),
                        (
                            "end",
                            obj(vec![
                                ("line", Json::Num(line.to_string())),
                                ("character", Json::Num((col + 1).to_string())),
                            ]),
                        ),
                    ]),
                ),
                ("severity", Json::Num("1".to_string())),
                ("source", Json::Str("rhp".to_string())),
                ("message", Json::Str(d.message.clone())),
            ])
        })
        .collect();
    notify(
        output,
        "textDocument/publishDiagnostics",
        obj(vec![
            ("uri", Json::Str(uri.to_string())),
            ("diagnostics", Json::Arr(items)),
        ]),
    )
}

/// Laco principal do servidor LSP sobre `input`/`output` (stdio na CLI).
pub fn run(input: impl BufRead, output: impl Write) -> Result<(), String> {
    let mut input = input;
    let mut output = output;
    let mut documents: HashMap<String, String> = HashMap::new();
    let mut shutdown_requested = false;
    while let Some(raw) = read_message(&mut input)? {
        let msg = parse_json(&raw).map_err(|e| {
            format!(
                "JSON invalido do cliente LSP (byte {}): {}",
                e.pos, e.message
            )
        })?;
        let method = msg.get("method").and_then(Json::as_str).unwrap_or("");
        let id = msg.get("id").cloned().unwrap_or(Json::Null);
        let has_id = !matches!(id, Json::Null);
        let params = msg.get("params").cloned().unwrap_or(Json::Null);
        match method {
            "initialize" => {
                respond(
                    &mut output,
                    &id,
                    obj(vec![
                        (
                            "capabilities",
                            obj(vec![("textDocumentSync", Json::Num("1".to_string()))]),
                        ),
                        (
                            "serverInfo",
                            obj(vec![
                                ("name", Json::Str("rhp-lsp".to_string())),
                                ("version", Json::Str(env!("CARGO_PKG_VERSION").to_string())),
                            ]),
                        ),
                    ]),
                )?;
            }
            "initialized" | "$/cancelRequest" | "$/progress" => {}
            "shutdown" => {
                shutdown_requested = true;
                if has_id {
                    respond(&mut output, &id, Json::Null)?;
                }
            }
            "exit" => {
                let code = if shutdown_requested { 0 } else { 1 };
                std::process::exit(code);
            }
            "textDocument/didOpen" => {
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri"))
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                let text = params
                    .get("textDocument")
                    .and_then(|d| d.get("text"))
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                documents.insert(uri.clone(), text.clone());
                publish_diagnostics(&mut output, &uri, &diagnose(&text))?;
            }
            "textDocument/didChange" => {
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri"))
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string();
                let mut text = documents.get(&uri).cloned().unwrap_or_default();
                if let Json::Arr(changes) =
                    params.get("contentChanges").cloned().unwrap_or(Json::Null)
                {
                    for change in &changes {
                        if let Some(t) = change.get("text").and_then(Json::as_str) {
                            text = t.to_string();
                        }
                    }
                }
                documents.insert(uri.clone(), text.clone());
                publish_diagnostics(&mut output, &uri, &diagnose(&text))?;
            }
            _ => {
                if has_id {
                    respond_error(&mut output, &id, -32601, "metodo desconhecido")?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn to_json(value: &Json) -> String {
        let mut out = String::new();
        render(value, &mut out);
        out
    }

    fn parse(text: &str) -> Json {
        parse_json(text.as_bytes()).expect("JSON valido")
    }

    #[test]
    fn parses_json_shapes() {
        assert_eq!(parse("null"), Json::Null);
        assert_eq!(parse("true"), Json::Bool(true));
        assert_eq!(parse("42"), Json::Num("42".to_string()));
        assert_eq!(parse("-2.5e3"), Json::Num("-2.5e3".to_string()));
        assert_eq!(parse("\"a\\nb\\\"c\""), Json::Str("a\nb\"c".to_string()));
        assert_eq!(parse("\"\\u00e9\""), Json::Str("é".to_string()));
        assert_eq!(
            parse("{\"a\": [1, null], \"b\": {}}"),
            obj(vec![
                ("a", Json::Arr(vec![Json::Num("1".to_string()), Json::Null])),
                ("b", obj(vec![])),
            ])
        );
        assert!(parse_json(b"{").is_err());
        assert!(parse_json(b"{\"a\"}").is_err());
        assert!(parse_json(b"[1,]").is_err());
        assert!(parse_json(b"\"\\x\"").is_err());
        assert!(parse_json(b"nul").is_err());
        assert!(parse_json(b"{} {}").is_err());
    }

    #[test]
    fn round_trips_escaping() {
        let value = Json::Str("a\"b\\c\nd\u{1}é".to_string());
        let mut out = String::new();
        render(&value, &mut out);
        assert_eq!(parse(&out), value);
    }

    #[test]
    fn diagnoses_broken_source() {
        let bad = diagnose("echo \"x\"");
        assert_eq!(bad.len(), 1);
        assert!(bad[0].message.contains(';'));
        assert!(diagnose("echo 1;\n").is_empty());
    }

    #[test]
    fn serves_initialize_and_diagnostics() {
        // Sessao scriptada: initialize -> didOpen (quebrado) -> didChange
        // (sao) -> shutdown/exit. Sem rede, so bytes.
        let script = [
            frame(&to_json(&obj(vec![
                ("jsonrpc", Json::Str("2.0".to_string())),
                ("id", Json::Num("1".to_string())),
                ("method", Json::Str("initialize".to_string())),
                ("params", obj(vec![])),
            ]))),
            frame(&to_json(&obj(vec![
                ("jsonrpc", Json::Str("2.0".to_string())),
                ("method", Json::Str("textDocument/didOpen".to_string())),
                (
                    "params",
                    obj(vec![(
                        "textDocument",
                        obj(vec![
                            ("uri", Json::Str("file:///a.rhp".to_string())),
                            ("text", Json::Str("echo \"x\"".to_string())),
                        ]),
                    )]),
                ),
            ]))),
            frame(&to_json(&obj(vec![
                ("jsonrpc", Json::Str("2.0".to_string())),
                ("method", Json::Str("textDocument/didChange".to_string())),
                (
                    "params",
                    obj(vec![
                        (
                            "textDocument",
                            obj(vec![("uri", Json::Str("file:///a.rhp".to_string()))]),
                        ),
                        (
                            "contentChanges",
                            Json::Arr(vec![obj(vec![(
                                "text",
                                Json::Str("echo 1;\n".to_string()),
                            )])]),
                        ),
                    ]),
                ),
            ]))),
        ]
        .concat();
        let mut out = Vec::new();
        let input = std::io::BufReader::new(&script[..]);
        run(input, &mut out).expect("sessao LSP sem erro");
        // `run` so sai por EOF (exit() so via mensagem `exit`); aqui EOF.
        let text = String::from_utf8(out).expect("saida utf8");
        assert!(text.contains("\"result\""), "initialize responde: {text}");
        assert!(text.contains("publishDiagnostics"), "publica: {text}");
        assert!(text.contains("file:///a.rhp"), "uri: {text}");
    }

    fn frame(body: &str) -> Vec<u8> {
        format!("Content-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
    }
}
