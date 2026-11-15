//! JSON std-only do RHP: parser e serializador proprios, sem dependencias.
//!
//! Limites (protecao contra exaustao; documentados em `LANGUAGE_SPEC.md`):
//! - Profundidade maxima de aninhamento: [`MAX_JSON_DEPTH`] (128, como o
//!   teto sintatico). Alem disso = erro, nunca estouro de pilha.
//! - `json_parse` rejeita entradas malformadas com a posicao (byte) do erro.
//! - Numeros seguem a gramatica JSON estrita (sem `+`, sem zeros a esquerda,
//!   sem `.5`/`5.`, sem hex); inteiro em `i64` vira `Int`, resto `Float`.
//! - Objetos viram `Map` com **primeira ocorrencia vencendo** (mesma regra
//!   dos literais de mapa do RHP); chaves duplicadas nao sao erro.
//! - `json_stringify` de `Float` nao-finito (`NaN`/infinito) e de `Func`
//!   e erro (JSON nao os representa). Mapas saem com chaves ordenadas
//!   (saida deterministica, como o `Display`).

use super::{type_name, VmError};
use rhp_bytecode::Value;
use std::collections::HashMap;

/// Profundidade maxima de aninhamento em parse e serializacao.
pub const MAX_JSON_DEPTH: usize = 128;

/// Converte texto JSON em valor do RHP.
///
/// # Erros
///
/// Retorna [`VmError`] se a entrada nao for string JSON valida, exceder a
/// profundidade maxima ou contiver numero fora do representavel.
pub fn parse_json(text: &str) -> Result<Value, VmError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    parser.skip_ws();
    let value = parser.parse_value(0)?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error("conteudo apos o valor JSON"));
    }
    Ok(value)
}

/// Converte valor do RHP em texto JSON canonico (chaves ordenadas).
///
/// # Erros
///
/// Retorna [`VmError`] em `Float` nao-finito, `Func` ou profundidade alem
/// de [`MAX_JSON_DEPTH`].
pub fn stringify_json(value: &Value) -> Result<String, VmError> {
    let mut out = String::new();
    stringify_into(value, &mut out, 0)?;
    Ok(out)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn error(&self, what: impl Into<String>) -> VmError {
        VmError::new(format!(
            "`json_parse` invalido no byte {}: {}",
            self.pos,
            what.into()
        ))
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len()
            && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect_byte(&mut self, expected: u8, what: &str) -> Result<(), VmError> {
        if self.peek() == Some(expected) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(format!("esperado `{what}`")))
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value, VmError> {
        if depth > MAX_JSON_DEPTH {
            return Err(self.error(format!("profundidade maxima excedida ({MAX_JSON_DEPTH})")));
        }
        match self.peek() {
            Some(b'n') => self.parse_literal("null", Value::Null),
            Some(b't') => self.parse_literal("true", Value::Bool(true)),
            Some(b'f') => self.parse_literal("false", Value::Bool(false)),
            Some(b'"') => Ok(Value::Str(self.parse_string()?)),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_object(depth),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.parse_number(),
            Some(_) => Err(self.error("valor inesperado")),
            None => Err(self.error("fim inesperado da entrada")),
        }
    }

    fn parse_literal(&mut self, literal: &str, value: Value) -> Result<Value, VmError> {
        if self.bytes[self.pos..].starts_with(literal.as_bytes()) {
            self.pos += literal.len();
            Ok(value)
        } else {
            Err(self.error("literal invalido"))
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, VmError> {
        self.pos += 1; // `[`
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(Value::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.parse_value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("esperado `,` ou `]`")),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, VmError> {
        self.pos += 1; // `{`
        let mut entries: HashMap<String, Value> = HashMap::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(Value::Map(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(self.error("esperado chave (string)"));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect_byte(b':', ":")?;
            self.skip_ws();
            let value = self.parse_value(depth + 1)?;
            // Primeira ocorrencia vence (regra dos literais de mapa do RHP).
            entries.entry(key).or_insert(value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Map(entries));
                }
                _ => return Err(self.error("esperado `,` ou `}`")),
            }
        }
    }

    fn parse_string(&mut self) -> Result<String, VmError> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.pos += 1;
        let mut out = String::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| self.error("string nao terminada"))?;
            match byte {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let esc = self.peek().ok_or_else(|| self.error("escape invalido"))?;
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
                        b'u' => out.push(self.parse_unicode_escape()?),
                        _ => return Err(self.error("escape invalido")),
                    }
                }
                0x00..=0x1F => return Err(self.error("controle literal em string")),
                _ => {
                    // Avanca um escalar UTF-8 inteiro (entrada e `&str`).
                    let rest = &self.bytes[self.pos..];
                    let text =
                        std::str::from_utf8(rest).map_err(|_| self.error("UTF-8 invalido"))?;
                    let ch = text.chars().next().expect("byte existente");
                    out.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, VmError> {
        let high = self.parse_hex4()?;
        if (0xD800..0xDC00).contains(&high) {
            // Par substituto: espera `\uDC00..\uDFFF`.
            if self.peek() == Some(b'\\') {
                self.pos += 1;
                if self.peek() == Some(b'u') {
                    self.pos += 1;
                    let low = self.parse_hex4()?;
                    if (0xDC00..0xE000).contains(&low) {
                        let point = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                        return char::from_u32(point).ok_or_else(|| self.error("unicode invalido"));
                    }
                }
            }
            return Err(self.error("metade de par substituto sem par"));
        }
        if (0xDC00..0xE000).contains(&high) {
            return Err(self.error("metade baixa de par substituto isolada"));
        }
        char::from_u32(high).ok_or_else(|| self.error("unicode invalido"))
    }

    fn parse_hex4(&mut self) -> Result<u32, VmError> {
        if self.pos + 4 > self.bytes.len() {
            return Err(self.error("escape \\u incompleto"));
        }
        let mut code: u32 = 0;
        for i in 0..4 {
            let digit = self.bytes[self.pos + i];
            let value = match digit {
                b'0'..=b'9' => (digit - b'0') as u32,
                b'a'..=b'f' => (digit - b'a' + 10) as u32,
                b'A'..=b'F' => (digit - b'A' + 10) as u32,
                _ => return Err(self.error("digito hexadecimal invalido")),
            };
            code = code * 16 + value;
        }
        self.pos += 4;
        Ok(code)
    }

    fn parse_number(&mut self) -> Result<Value, VmError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                // JSON estrito: sem zeros a esquerda (`01` e invalido).
                if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    return Err(self.error("numero invalido"));
                }
            }
            Some(c) if c.is_ascii_digit() => {
                while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.error("numero invalido")),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                return Err(self.error("numero invalido"));
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                return Err(self.error("numero invalido"));
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| self.error("numero invalido"))?;
        if !is_float {
            if let Ok(int) = text.parse::<i64>() {
                return Ok(Value::Int(int));
            }
            // Inteiro fora de `i64`: tenta `Float` (perda documentada).
        }
        text.parse::<f64>()
            .map(Value::Float)
            .map_err(|_| self.error("numero invalido"))
    }
}

fn stringify_into(value: &Value, out: &mut String, depth: usize) -> Result<(), VmError> {
    if depth > MAX_JSON_DEPTH {
        return Err(VmError::new(format!(
            "`json_stringify` excedeu a profundidade maxima ({MAX_JSON_DEPTH})"
        )));
    }
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(v) => out.push_str(&v.to_string()),
        Value::Float(v) => {
            if !v.is_finite() {
                return Err(VmError::new(
                    "`json_stringify` nao representa NaN/infinito".to_string(),
                ));
            }
            let mut text = format!("{v}");
            if !text.contains('.') && !text.contains('e') && !text.contains('E') {
                text.push_str(".0");
            }
            out.push_str(&text);
        }
        Value::Str(s) => stringify_str(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                stringify_into(item, out, depth + 1)?;
            }
            out.push(']');
        }
        Value::Map(entries) => {
            let mut keys: Vec<&String> = entries.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                stringify_str(key, out);
                out.push(':');
                let item = entries.get(*key).expect("chave existe");
                stringify_into(item, out, depth + 1)?;
            }
            out.push('}');
        }
        Value::Func(func) => {
            return Err(VmError::new(format!(
                "`json_stringify` nao representa {} (`{}`)",
                type_name(value),
                func.name
            )));
        }
    }
    Ok(())
}

fn stringify_str(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
