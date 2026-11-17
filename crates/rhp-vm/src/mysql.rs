//! Cliente MySQL (protocolo 4.1, texto) sobre TCP (Etapa 5, RFC 0020).
//!
//! `db_open("mysql:host=.. port=.. user=.. [password=..] [dbname=..]")`
//! usa handshake + `mysql_native_password`; `db_exec`/`db_query` via
//! `COM_QUERY` texto (params `?` interpolados com escape no cliente);
//! `db_close` envia `COM_QUIT`. Tipos comuns mapeados; resto vira `Str`.
//! SHA-1 proprio (sem deps) para a auth; `conninfo` nunca aparece em erro.

use super::tcp::TcpConn;
use std::collections::HashMap;

/// Capacidades minimas do cliente (4.1 + secure + plugin-auth [+db]).
const CAPS_BASE: u32 = 0x0000_0200 | 0x0000_8000 | 0x0008_0000;
const CAPS_DB: u32 = 0x0000_0008;
/// Conjunto de caracteres (utf8mb4).
const CHARSET: u8 = 45;
/// Teto de pacote lido (anti-DoS; MySQL permite ate 16 MiB por padrao).
const MAX_PACKET: usize = 16 * 1024 * 1024;

/// Conexao MySQL (sequencia de pacotes + handle TCP).
#[derive(Debug)]
pub struct Connection {
    conn: TcpConn,
    seq: u8,
}

/// SHA-1 compacto (FIPS 180-4) para `mysql_native_password`.
/// Testado contra os vetores do NIST em `#[cfg(test)]`.
fn sha1(message: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let bit_len = (message.len() as u64).wrapping_mul(8);
    let mut padded = message.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());
    let mut w = [0u32; 80];
    let (blocks, _) = padded.as_chunks::<64>();
    for block in blocks {
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[4 * i],
                block[4 * i + 1],
                block[4 * i + 2],
                block[4 * i + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, &word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// `mysql_native_password`: SHA1(senha) XOR SHA1(seed + SHA1(SHA1(senha))).
fn native_password(password: &[u8], seed: &[u8]) -> [u8; 20] {
    let stage1 = sha1(password);
    let mut both = Vec::with_capacity(seed.len() + 20);
    both.extend_from_slice(seed);
    both.extend_from_slice(&sha1(&stage1));
    let stage2 = sha1(&both);
    let mut out = [0u8; 20];
    for i in 0..20 {
        out[i] = stage1[i] ^ stage2[i];
    }
    out
}

fn read_lenenc(bytes: &[u8], pos: &mut usize) -> Result<u64, String> {
    let first = *bytes
        .get(*pos)
        .ok_or_else(|| "lenenc truncado".to_string())?;
    *pos += 1;
    match first {
        0xfb => Err("lenenc NULL onde numero era esperado".to_string()),
        0xfc => {
            let end = *pos + 2;
            let raw = bytes
                .get(*pos..end)
                .ok_or_else(|| "lenenc truncado".to_string())?;
            *pos = end;
            Ok(u16::from_le_bytes([raw[0], raw[1]]) as u64)
        }
        0xfd => {
            let end = *pos + 3;
            let raw = bytes
                .get(*pos..end)
                .ok_or_else(|| "lenenc truncado".to_string())?;
            *pos = end;
            Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], 0]) as u64)
        }
        0xfe => {
            let end = *pos + 8;
            let raw = bytes
                .get(*pos..end)
                .ok_or_else(|| "lenenc truncado".to_string())?;
            *pos = end;
            Ok(u64::from_le_bytes([
                raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
            ]))
        }
        n => Ok(n as u64),
    }
}

fn read_lenenc_str<'a>(bytes: &'a [u8], pos: &mut usize) -> Result<&'a [u8], String> {
    if bytes.get(*pos) == Some(&0xfb) {
        return Err("NULL onde string era esperada".to_string());
    }
    let len = read_lenenc(bytes, pos)? as usize;
    let end = pos
        .checked_add(len)
        .ok_or_else(|| "lenenc alem do pacote".to_string())?;
    let raw = bytes
        .get(*pos..end)
        .ok_or_else(|| "string truncada".to_string())?;
    *pos = end;
    Ok(raw)
}

fn read_nul_str(bytes: &[u8], pos: &mut usize) -> Result<Vec<u8>, String> {
    let start = *pos;
    while *pos < bytes.len() && bytes[*pos] != 0 {
        *pos += 1;
    }
    if *pos >= bytes.len() {
        return Err("string NUL-terminada truncada".to_string());
    }
    let out = bytes[start..*pos].to_vec();
    *pos += 1;
    Ok(out)
}

fn read_u16(bytes: &[u8], pos: &mut usize) -> Result<u16, String> {
    let end = *pos + 2;
    let raw = bytes
        .get(*pos..end)
        .ok_or_else(|| "u16 truncado".to_string())?;
    *pos = end;
    Ok(u16::from_le_bytes([raw[0], raw[1]]))
}

fn read_u32(bytes: &[u8], pos: &mut usize) -> Result<u32, String> {
    let end = *pos + 4;
    let raw = bytes
        .get(*pos..end)
        .ok_or_else(|| "u32 truncado".to_string())?;
    *pos = end;
    Ok(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]))
}

impl Connection {
    fn read_packet(&mut self) -> Result<(Vec<u8>, u8), String> {
        let mut header = Vec::new();
        self.conn.read_exact(4, &mut header)?;
        let len = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
        let seq = header[3];
        if len > MAX_PACKET {
            return Err(format!("pacote MySQL de {len} bytes (limite {MAX_PACKET})"));
        }
        let mut payload = Vec::new();
        self.conn.read_exact(len, &mut payload)?;
        Ok((payload, seq))
    }

    fn write_packet(&mut self, payload: &[u8]) -> Result<(), String> {
        let mut out = Vec::with_capacity(payload.len() + 4);
        let len = payload.len();
        if len > 0xFF_FFFF {
            return Err("pacote MySQL grande demais".to_string());
        }
        out.extend_from_slice(&[
            (len & 0xFF) as u8,
            ((len >> 8) & 0xFF) as u8,
            ((len >> 16) & 0xFF) as u8,
            self.seq,
        ]);
        out.extend_from_slice(payload);
        self.seq = self.seq.wrapping_add(1);
        self.conn.write_all(&out).map(|_| ())
    }

    /// `db_open("mysql:host=.. port=.. user=.. [password=..] [dbname=..]")`.
    /// Senha nunca aparece em mensagens de erro.
    pub fn connect(conninfo: &str) -> Result<Self, String> {
        let mut opts = HashMap::new();
        for part in conninfo.split_whitespace() {
            let (key, value) = part.split_once('=').ok_or_else(|| {
                "`db_open`: `mysql:` exige `chave=valor` (host, port, user, [password], [dbname])".to_string()
            })?;
            opts.insert(key.to_string(), value.to_string());
        }
        let host = opts
            .get("host")
            .cloned()
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let port: u16 = opts
            .get("port")
            .map(|p| {
                p.parse()
                    .map_err(|_| "`db_open`: `port` invalida".to_string())
            })
            .transpose()?
            .unwrap_or(3306);
        let user = opts
            .get("user")
            .cloned()
            .unwrap_or_else(|| "root".to_string());
        let password = opts.get("password").cloned().unwrap_or_default();
        let dbname = opts.get("dbname").cloned();
        let mut conn = TcpConn::connect(&host, port).map_err(|e| format!("`db_open`: {e}"))?;
        // Saudacao do servidor.
        let (greeting, _) = Self::read_packet_static(&mut conn)?;
        let mut pos = 0;
        let proto = *greeting
            .get(pos)
            .ok_or_else(|| "`db_open`: saudacao truncada".to_string())?;
        if proto != 10 {
            return Err("`db_open`: protocolo MySQL != 10".to_string());
        }
        pos += 1;
        let _version = read_nul_str(&greeting, &mut pos)?;
        let _conn_id = read_u32(&greeting, &mut pos)?;
        if greeting.len() < pos + 8 {
            return Err("`db_open`: saudacao truncada (seed)".to_string());
        }
        let mut seed = greeting[pos..pos + 8].to_vec();
        pos += 8 + 1; // seed + filler
        let _caps_lo = read_u16(&greeting, &mut pos)?;
        let _charset = *greeting
            .get(pos)
            .ok_or_else(|| "`db_open`: saudacao truncada".to_string())?;
        pos += 1 + 2; // charset + status
        let _caps_hi = read_u16(&greeting, &mut pos)?;
        let _plugin_len = *greeting.get(pos).unwrap_or(&0);
        pos += 1 + 10; // len + reservado
        let rest_start = pos;
        while pos < greeting.len() && greeting[pos] != 0 {
            pos += 1;
        }
        seed.extend_from_slice(&greeting[rest_start..pos]);
        // (nome do plugin ignorado: sempre `mysql_native_password` aqui.)
        let mut this = Self { conn, seq: 1 };
        // Resposta de handshake.
        let mut caps = CAPS_BASE;
        if dbname.is_some() {
            caps |= CAPS_DB;
        }
        let mut resp = Vec::new();
        resp.extend_from_slice(&caps.to_le_bytes());
        resp.extend_from_slice(&0x0100_0000u32.to_le_bytes());
        resp.push(CHARSET);
        resp.extend_from_slice(&[0u8; 23]);
        resp.extend_from_slice(user.as_bytes());
        resp.push(0);
        if password.is_empty() {
            resp.push(0);
        } else {
            let token = native_password(password.as_bytes(), &seed);
            resp.push(token.len() as u8);
            resp.extend_from_slice(&token);
        }
        if let Some(db) = dbname {
            resp.extend_from_slice(db.as_bytes());
            resp.push(0);
        }
        resp.extend_from_slice(b"mysql_native_password\0");
        this.write_packet(&resp)?;
        let (reply, _) = this.read_packet()?;
        Self::check_ok_or_err(&reply, "`db_open`")?;
        Ok(this)
    }

    /// Confere OK (`0x00`) ou converte ERR (`0xFF`) em erro legivel
    /// (sem credenciais: servidor nunca as ecoa aqui).
    fn check_ok_or_err(packet: &[u8], context: &str) -> Result<(), String> {
        match packet.first() {
            Some(0x00) => Ok(()),
            Some(0xFF) => {
                let mut pos = 1;
                let code = read_u16(packet, &mut pos).unwrap_or(0);
                let message = if packet.get(pos) == Some(&b'#') {
                    pos += 6;
                    String::from_utf8_lossy(&packet[pos..]).into_owned()
                } else {
                    String::from_utf8_lossy(&packet[pos..]).into_owned()
                };
                Err(format!("`{context}`: erro {code} do MySQL: {message}"))
            }
            _ => Err(format!("`{context}`: resposta inesperada do MySQL")),
        }
    }

    fn command(&mut self, byte: u8, body: &[u8]) -> Result<Vec<u8>, String> {
        let mut payload = Vec::with_capacity(body.len() + 1);
        payload.push(byte);
        payload.extend_from_slice(body);
        self.write_packet(&payload)?;
        let (reply, _) = self.read_packet()?;
        Ok(reply)
    }

    /// Escapa `?` com literais (strings com escape MySQL padrao).
    fn interpolate(sql: &str, params: &[super::sqlite::SqlValue]) -> Result<String, String> {
        let mut out = String::new();
        let mut iter = params.iter();
        for ch in sql.chars() {
            if ch == '?' {
                let value = iter
                    .next()
                    .ok_or_else(|| "`db`: mais `?` que parametros".to_string())?;
                match value {
                    super::sqlite::SqlValue::Int(n) => out.push_str(&n.to_string()),
                    super::sqlite::SqlValue::Float(n) => out.push_str(&format!("{n}")),
                    super::sqlite::SqlValue::Text(s) => {
                        out.push('\'');
                        for c in s.chars() {
                            match c {
                                '\0' => out.push_str("\\0"),
                                '\n' => out.push_str("\\n"),
                                '\r' => out.push_str("\\r"),
                                '\\' => out.push_str("\\\\"),
                                '\'' => out.push_str("\\'"),
                                '"' => out.push_str("\\\""),
                                '\x1a' => out.push_str("\\Z"),
                                c => out.push(c),
                            }
                        }
                        out.push('\'');
                    }
                    super::sqlite::SqlValue::Null => out.push_str("NULL"),
                }
            } else {
                out.push(ch);
            }
        }
        if iter.next().is_some() {
            return Err("`db`: mais parametros que `?`".to_string());
        }
        Ok(out)
    }

    /// Executa (INSERT/UPDATE/...); devolve linhas afetadas.
    pub fn exec(&mut self, sql: &str, params: &[super::sqlite::SqlValue]) -> Result<i64, String> {
        let sql = Self::interpolate(sql, params)?;
        let reply = self.command(0x03, sql.as_bytes())?;
        match reply.first() {
            Some(0x00) => {
                let mut pos = 1;
                Ok(read_lenenc(&reply, &mut pos)? as i64)
            }
            Some(0xFF) => Err(Self::err_text(&reply, "`db_exec`")),
            _ => Err("`db_exec`: resposta inesperada (use `db_query` para SELECT)".to_string()),
        }
    }

    /// Consulta; devolve linhas como mapas coluna => valor.
    pub fn query(
        &mut self,
        sql: &str,
        params: &[super::sqlite::SqlValue],
    ) -> Result<Vec<HashMap<String, super::sqlite::SqlValue>>, String> {
        let sql = Self::interpolate(sql, params)?;
        let reply = self.command(0x03, sql.as_bytes())?;
        match reply.first() {
            Some(0xFF) => return Err(Self::err_text(&reply, "`db_query`")),
            Some(0x00) => return Ok(Vec::new()),
            _ => {}
        }
        let mut pos = 0;
        let ncol = read_lenenc(&reply, &mut pos)? as usize;
        // Definicoes de coluna: nome + tipo.
        let mut names = Vec::with_capacity(ncol);
        let mut types = Vec::with_capacity(ncol);
        for _ in 0..ncol {
            let (def, _) = self.read_packet()?;
            let mut p = 0;
            // catalog, schema, table, org_table (lenenc-str, ignorados).
            for _ in 0..4 {
                read_lenenc_str(&def, &mut p)?;
            }
            let name_raw = read_lenenc_str(&def, &mut p)?;
            let name = String::from_utf8_lossy(name_raw).into_owned();
            // org_name (lenenc-str), marcador de tamanho fixo (1),
            // charset (2), tamanho (4), tipo (1), flags (2), decimais (1).
            read_lenenc_str(&def, &mut p)?;
            p += 1 + 2 + 4;
            let coltype = *def
                .get(p)
                .ok_or_else(|| "`db_query`: coluna truncada".to_string())?;
            names.push(name);
            types.push(coltype);
        }
        // EOF apos colunas (sem CLIENT_DEPRECATE_EOF).
        let (eof, _) = self.read_packet()?;
        if eof.first() != Some(&0xFE) {
            return Err("`db_query`: EOF de colunas ausente".to_string());
        }
        let mut rows = Vec::new();
        loop {
            let (row, _) = self.read_packet()?;
            match row.first() {
                Some(0xFE) => break,
                Some(0xFF) => return Err(Self::err_text(&row, "`db_query`")),
                _ => {}
            }
            let mut p = 0;
            let mut map = HashMap::with_capacity(ncol);
            for i in 0..ncol {
                let value = if row.get(p) == Some(&0xFB) {
                    p += 1;
                    super::sqlite::SqlValue::Null
                } else {
                    let raw = read_lenenc_str(&row, &mut p)?;
                    let text = String::from_utf8_lossy(raw);
                    map_mysql_value(types[i], &text)
                };
                map.insert(names[i].clone(), value);
            }
            rows.push(map);
        }
        Ok(rows)
    }

    /// Fecha com `COM_QUIT` (erro de rede no quit e ignorado).
    pub fn close(mut self) -> Result<(), String> {
        let _ = self.command(0x01, &[]);
        Ok(())
    }

    fn err_text(packet: &[u8], context: &str) -> String {
        let mut pos = 1;
        let code = read_u16(packet, &mut pos).unwrap_or(0);
        let message = if packet.get(pos) == Some(&b'#') {
            pos += 6;
            String::from_utf8_lossy(&packet[pos..]).into_owned()
        } else {
            String::from_utf8_lossy(&packet[pos..]).into_owned()
        };
        format!("`{context}`: erro {code} do MySQL: {message}")
    }

    /// Le um pacote sem instancia (usado no handshake, antes do `seq`).
    fn read_packet_static(conn: &mut TcpConn) -> Result<(Vec<u8>, u8), String> {
        let mut header = vec![0u8; 0];
        conn.read_exact(4, &mut header)?;
        let len = u32::from_le_bytes([header[0], header[1], header[2], 0]) as usize;
        let seq = header[3];
        if len > MAX_PACKET {
            return Err(format!("pacote MySQL de {len} bytes (limite {MAX_PACKET})"));
        }
        let mut payload = Vec::new();
        conn.read_exact(len, &mut payload)?;
        Ok((payload, seq))
    }
}

/// Mapeia tipo de coluna MySQL + texto para valor (`Int` quando inteiro
/// valido e tipo numerico; `Float` idem; resto `Text`; BLOB nao-UTF-8 =
/// erro, como no SQLite).
fn map_mysql_value(coltype: u8, text: &str) -> super::sqlite::SqlValue {
    use super::sqlite::SqlValue;
    // TINY/SHORT/LONG/INT24/LONGLONG/YEAR.
    const INTS: [u8; 6] = [1, 2, 3, 9, 8, 13];
    // FLOAT/DOUBLE/DECIMAL/NEWDECIMAL.
    const FLOATS: [u8; 4] = [4, 5, 246, 0];
    if INTS.contains(&coltype) {
        if let Ok(n) = text.parse::<i64>() {
            return SqlValue::Int(n);
        }
    } else if FLOATS.contains(&coltype) {
        if let Ok(n) = text.parse::<f64>() {
            if n.is_finite() {
                return SqlValue::Float(n);
            }
        }
    }
    SqlValue::Text(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_nist_vectors() {
        // FIPS 180-4, apendice B ("abc") + vetores multibloco (55/56/57,
        // 64/119/200 bytes) conferidos com `hashlib.sha1` (CPython).
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        // Fronteiras de bloco (55/56/57, 64, 119 e 200 bytes).
        for (input, expected) in [
            (
                "u8jzPde0IgxLd6GncfBAepfJBd0Kh8oOOL8dKLzdocJ2isAjIhKtJ0R",
                "b67e0c29f9d50788f3212d1670da1181c28d598f",
            ),
            (
                "lgLKOmxgJTeKdNnFRIBXuDL7DxtpYlSXpfKtHF4vUCsMehGAkWvj7FAc",
                "67f424b7c0ec5bb3a8eabd3b2019dbe1ec67321d",
            ),
            (
                        "9QeWJKY40uvSwMFLZDe1f8rESQedUStPKR0CsTy4Qwb8DwkNhFdnXsiVp",
                        "72a8c1f850b6973016aa9e9c5cb73c9819eb294a",
            ),
            (
                "zz63FfkCzJr4i0B3JrTAwR4y9ojfljoQoaF1LlqsajAIxNKu8iS2G8NPRVdD53X8",
                "5e5c1b1397eab5b1dc99d52d8992eb263e7d2cd9",
            ),
            (
                "3RZJzzzzgEOzdmenCkhvMdgaKjIg8xNbe3nNyjOq9wMxEhh2FDEEtfjgVvVqE1SkHbn88HxjSI6bWHtP3fS2qHx6kwXoIIXGvOoNZYW2mZp0zVZomHFwUbb",
                "2cdea6468ae71dc889aa23f0ce6094edb1edf882",
            ),
            (
                "YrEqmSM9wCZ7Uw9xfogoEmvnEN5N1aE6PwZPf1Qh6yYTWmE4lBYOvfZ8UzDzV8fUkkibjL5DZPjN0MEQ7wjJJibaZUPgHV7iB3m03nbqnsGpWLuqIA1id6Vw5DQL05HA064GiIjHGb3CXlMaXZjljENUhJduRHHJEYXg4JdpmrcXgGCJbW56eCuNGMGmSrCGIZEG8pSH",
                "1097ed6a69f623d06d668c4103eac00a37f615ea",
            ),
        ] {
            assert_eq!(hex(&sha1(input.as_bytes())), expected, "entrada: {input}");
        }
        // `mysql_native_password` e deterministico e sensivel a senha/seed.
        let seed = [b'a'; 20];
        let token = native_password(b"mypass", &seed);
        assert_eq!(token.len(), 20);
        assert_eq!(native_password(b"mypass", &seed), token);
        assert_ne!(native_password(b"other", &seed), token);
        let mut seed2 = seed;
        seed2[0] = b'b';
        assert_ne!(native_password(b"mypass", &seed2), token);
    }

    #[test]
    fn lenenc_roundtrip_shapes() {
        let mut pos = 0;
        assert_eq!(read_lenenc(&[0x05], &mut pos), Ok(5));
        assert_eq!(read_lenenc(&[0xfc, 0x34, 0x12], &mut 0), Ok(0x1234));
        assert!(read_lenenc(&[0xfb], &mut 0).is_err());
    }

    // -- Servidor mock (greeting + auth verificada + consultas fixas) --

    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    const MOCK_SEED: &[u8; 20] = b"abcdefghijklmnopqrst";
    const MOCK_USER: &str = "teste";
    const MOCK_PASS: &str = "segredo";

    fn mock_packet(stream: &mut TcpStream, seq: &mut u8, payload: &[u8]) {
        let len = payload.len();
        stream
            .write_all(&[
                (len & 0xFF) as u8,
                ((len >> 8) & 0xFF) as u8,
                ((len >> 16) & 0xFF) as u8,
                *seq,
            ])
            .expect("escrever pacote mock");
        stream.write_all(payload).expect("escrever corpo mock");
        *seq = seq.wrapping_add(1);
    }

    fn mock_read(stream: &mut TcpStream) -> (Vec<u8>, u8) {
        let mut header = [0u8; 4];
        stream.read_exact(&mut header).expect("ler cabecalho mock");
        let len = header[0] as usize | ((header[1] as usize) << 8) | ((header[2] as usize) << 16);
        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).expect("ler corpo mock");
        (body, header[3])
    }

    fn mock_column(name: &str, coltype: u8, out: &mut Vec<u8>) {
        for part in ["def", "", "", "", name, name] {
            out.push(part.len() as u8);
            out.extend_from_slice(part.as_bytes());
        }
        out.push(0x0c);
        out.extend_from_slice(&33u16.to_le_bytes());
        out.extend_from_slice(&255u32.to_le_bytes());
        out.push(coltype);
        out.extend_from_slice(&[0, 0]);
        out.push(0);
        out.extend_from_slice(&[0, 0]);
    }

    /// Mock: greeting fixa, verifica usuario + `mysql_native_password`,
    /// responde `SELECT 1 AS um, 'x' AS s` com 1 linha e ok/erro p/ resto.
    /// Aceita varias conexoes (uma thread por conexao; o teste termina).
    fn mysql_mock(listener: TcpListener) {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || mysql_conn(stream));
        }
    }

    /// Le um pacote; `None` em EOF/erro (fim da conexao de teste).
    fn mock_read_opt(stream: &mut TcpStream) -> Option<(Vec<u8>, u8)> {
        let mut header = [0u8; 4];
        stream.read_exact(&mut header).ok()?;
        let len = header[0] as usize | ((header[1] as usize) << 8);
        let mut body = vec![0u8; len];
        stream.read_exact(&mut body).ok()?;
        Some((body, header[3]))
    }

    fn mysql_conn(mut stream: TcpStream) {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .expect("timeout");
        let mut seq = 0u8;
        // Saudacao: protocolo 10, versao, conn_id, seed[0..8], filler,
        // caps, charset, status, caps_hi, auth_len, reservado, seed[8..]+NUL.
        let mut greeting = vec![10u8];
        greeting.extend_from_slice(b"mock-8.0\0");
        greeting.extend_from_slice(&42u32.to_le_bytes());
        greeting.extend_from_slice(&MOCK_SEED[..8]);
        greeting.push(0);
        greeting.extend_from_slice(&0x0200u16.to_le_bytes());
        greeting.push(33);
        greeting.extend_from_slice(&2u16.to_le_bytes());
        greeting.extend_from_slice(&0x8087u16.to_le_bytes());
        greeting.push(21);
        greeting.extend_from_slice(&[0u8; 10]);
        greeting.extend_from_slice(&MOCK_SEED[8..]);
        greeting.push(0);
        greeting.extend_from_slice(b"mysql_native_password\0");
        mock_packet(&mut stream, &mut seq, &greeting);
        // Resposta do cliente: valida usuario + token.
        let (hs, _) = mock_read(&mut stream);
        let mut pos = 4 + 4 + 1 + 23;
        let mut user = Vec::new();
        while hs[pos] != 0 {
            user.push(hs[pos]);
            pos += 1;
        }
        pos += 1;
        assert_eq!(user, MOCK_USER.as_bytes(), "usuario inesperado");
        let token_len = hs[pos] as usize;
        pos += 1;
        let token = &hs[pos..pos + token_len];
        let expected = native_password(MOCK_PASS.as_bytes(), MOCK_SEED);
        assert_eq!(token, &expected[..], "auth invalida");
        // OK de boas-vindas.
        mock_packet(&mut stream, &mut seq, &[0, 0, 0, 2, 0, 0, 0]);
        while let Some((cmd, _)) = mock_read_opt(&mut stream) {
            match cmd.first() {
                Some(0x01) => break, // COM_QUIT.
                Some(0x03) => {
                    let sql = String::from_utf8_lossy(&cmd[1..]).into_owned();
                    if sql.contains("um") {
                        // 2 colunas: `um` LONGLONG, `s` VAR_STRING.
                        mock_packet(&mut stream, &mut seq, &[2]);
                        let mut col_um = Vec::new();
                        mock_column("um", 8, &mut col_um);
                        mock_packet(&mut stream, &mut seq, &col_um);
                        let mut col_s = Vec::new();
                        mock_column("s", 253, &mut col_s);
                        mock_packet(&mut stream, &mut seq, &col_s);
                        mock_packet(&mut stream, &mut seq, &[0xFE, 0, 0, 2, 0]);
                        // Linha: 1, "x".
                        mock_packet(&mut stream, &mut seq, &[1, b'1', 1, b'x']);
                        mock_packet(&mut stream, &mut seq, &[0xFE, 0, 0, 2, 0]);
                    } else if sql.starts_with("INSERT") {
                        // OK: 1 linha afetada.
                        mock_packet(&mut stream, &mut seq, &[0, 1, 0, 2, 0, 0, 0]);
                    } else {
                        // ERRO 1064 (0x0428, little-endian).
                        let mut err = vec![0xFF, 0x28, 0x04, b'#', b'4', b'2', b'0', b'0', b'0'];
                        err.extend_from_slice(b"syntax mock");
                        mock_packet(&mut stream, &mut seq, &err);
                    }
                }
                _ => break,
            }
        }
    }

    fn mysql_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("porta").port();
        std::thread::spawn(move || mysql_mock(listener));
        port
    }

    fn run_mysql(src: &str) -> Result<String, String> {
        let program = rhp_parser::parse_source(src).expect("parse deve passar");
        let chunk = rhp_bytecode::compile(&program).expect("compile deve passar");
        let mut vm = crate::Vm::new();
        vm.set_program(std::sync::Arc::new(program));
        vm.run_to_string(&chunk).map_err(|e| e.message)
    }

    #[test]
    fn mysql_query_and_exec_against_mock() {
        let port = mysql_port();
        let open =
            format!("db_open(\"mysql:host=127.0.0.1 port={port} user=teste password=segredo\")");
        // SELECT com tipos mapeados (int + string).
        let out = run_mysql(&format!(
            "let h = {open};\nlet linhas = db_query(h, \"SELECT 1 AS um, \'x\' AS s\", []);\necho linhas;\necho db_close(h);\n"
        ))
        .expect("query mock");
        assert_eq!(out, "[{s: x, um: 1}]\nnull\n");
        // INSERT retorna linhas afetadas.
        let out = run_mysql(&format!(
            "let h = {open};\necho db_exec(h, \"INSERT INTO t VALUES (1)\", []);\necho db_close(h);\n"
        ))
        .expect("exec mock");
        assert_eq!(out, "1\nnull\n");
        // Erro SQL vira erro RHP.
        let err = run_mysql(&format!(
            "let h = {open};\necho db_query(h, \"OPS\", []);\n"
        ))
        .expect_err("SQL ruim deve falhar");
        assert!(err.contains("1064"), "obteve: {err}");
    }
}
