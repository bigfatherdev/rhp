//! SQLite via FFI minimo ao `libsqlite3` do sistema (ADR 0011).
//!
//! Escopo deliberado: `open/prepare/bind/step/column/finalize/close` +
//! `changes/errmsg`. Sem extensoes, sem backup API, sem BLOB (RHP nao tem
//! bytes; BLOB lido = erro claro).
//!
//! # Seguranca do `unsafe` (invariantes, auditaveis)
//!
//! - Tipos opacos (`sqlite3`, `sqlite3_stmt`) nunca sao dereferenciados;
//!   so trafegam como `*mut` entre chamadas da mesma C ABI estavel.
//! - Toda string RHP -> C passa por `CString::new` (NUL interior = erro
//!   antes de qualquer chamada); toda string C -> RHP e copiada
//!   imediatamente (`CStr::to_bytes` + tamanho de `column_bytes`).
//! - `bind_text` usa `SQLITE_TRANSIENT` (a lib copia antes de retornar).
//! - `finalize` roda em todos os caminhos via guarda com `Drop` (sem
//!   vazamento de prepared statement, inclusive em erro).
//! - Ponteiros de erro/coluna sao checados contra nulo antes do uso.
//! - `Connection` e `!Send` (`PhantomData<Rc<()>>`): handle confinado a
//!   thread da VM (single-thread por desenho).

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_uchar, c_void};
use std::rc::Rc;

#[repr(C)]
enum sqlite3 {
    #[allow(dead_code)]
    __Opaque,
}

#[repr(C)]
enum sqlite3_stmt {
    #[allow(dead_code)]
    __Opaque,
}

const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;

const SQLITE_INTEGER: c_int = 1;
const SQLITE_FLOAT: c_int = 2;
const SQLITE_TEXT: c_int = 3;
const SQLITE_NULL: c_int = 5;

const SQLITE_TRANSIENT: *mut c_void = -1isize as *mut c_void;

#[link(name = "sqlite3")]
extern "C" {
    fn sqlite3_open(filename: *const c_char, pp_db: *mut *mut sqlite3) -> c_int;
    fn sqlite3_close(db: *mut sqlite3) -> c_int;
    fn sqlite3_prepare_v2(
        db: *mut sqlite3,
        sql: *const c_char,
        n_byte: c_int,
        pp_stmt: *mut *mut sqlite3_stmt,
        tail: *mut *const c_char,
    ) -> c_int;
    fn sqlite3_step(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_finalize(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_bind_parameter_count(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_bind_int64(stmt: *mut sqlite3_stmt, idx: c_int, value: i64) -> c_int;
    fn sqlite3_bind_double(stmt: *mut sqlite3_stmt, idx: c_int, value: c_double) -> c_int;
    fn sqlite3_bind_text(
        stmt: *mut sqlite3_stmt,
        idx: c_int,
        value: *const c_char,
        n: c_int,
        destructor: *mut c_void,
    ) -> c_int;
    fn sqlite3_bind_null(stmt: *mut sqlite3_stmt, idx: c_int) -> c_int;
    fn sqlite3_column_count(stmt: *mut sqlite3_stmt) -> c_int;
    fn sqlite3_column_name(stmt: *mut sqlite3_stmt, col: c_int) -> *const c_char;
    fn sqlite3_column_type(stmt: *mut sqlite3_stmt, col: c_int) -> c_int;
    fn sqlite3_column_int64(stmt: *mut sqlite3_stmt, col: c_int) -> i64;
    fn sqlite3_column_double(stmt: *mut sqlite3_stmt, col: c_int) -> c_double;
    fn sqlite3_column_text(stmt: *mut sqlite3_stmt, col: c_int) -> *const c_uchar;
    fn sqlite3_column_bytes(stmt: *mut sqlite3_stmt, col: c_int) -> c_int;
    fn sqlite3_errmsg(db: *mut sqlite3) -> *const c_char;
    fn sqlite3_changes(db: *mut sqlite3) -> c_int;
}

/// Valor de parametro/coluna suportado (sem BLOB: RHP nao tem bytes).
#[derive(Debug, Clone)]
pub enum SqlValue {
    Int(i64),
    Float(f64),
    Text(String),
    Null,
}

/// Conexao SQLite (handle bruto + guarda de fechamento).
#[derive(Debug)]
pub struct Connection {
    db: *mut sqlite3,
    _not_send: std::marker::PhantomData<Rc<()>>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        if !self.db.is_null() {
            // `Drop` nao pode falhar; erro aqui seria duplo (ja reportado
            // no caminho normal ou fim da VM).
            unsafe {
                sqlite3_close(self.db);
            }
            self.db = std::ptr::null_mut();
        }
    }
}

fn last_error(db: *mut sqlite3, context: &str) -> String {
    if db.is_null() {
        return format!("`db`: {context} (sem conexao)");
    }
    let message = unsafe {
        let raw = sqlite3_errmsg(db);
        if raw.is_null() {
            return format!("`db`: {context} (erro desconhecido)");
        }
        CStr::from_ptr(raw).to_string_lossy().into_owned()
    };
    format!("`db`: {context}: {message}")
}

fn c_string(value: &str, context: &str) -> Result<CString, String> {
    CString::new(value).map_err(|_| format!("`db`: {context} com NUL embutido"))
}

impl Connection {
    /// Abre (ou cria) o banco em `path`.
    pub fn open(path: &str) -> Result<Self, String> {
        let filename = c_string(path, "abrir banco")?;
        let mut db: *mut sqlite3 = std::ptr::null_mut();
        let code = unsafe { sqlite3_open(filename.as_ptr(), &mut db) };
        if code != SQLITE_OK || db.is_null() {
            if !db.is_null() {
                unsafe {
                    sqlite3_close(db);
                }
            }
            return Err(format!("`db`: abrir banco `{path}` falhou (codigo {code})"));
        }
        Ok(Self {
            db,
            _not_send: std::marker::PhantomData,
        })
    }

    fn prepare(&self, sql: &str) -> Result<*mut sqlite3_stmt, String> {
        // CONN-005: `""` caia no `sqlite3_errmsg` generico ("not an error").
        if sql.trim().trim_matches(';').trim().is_empty() {
            return Err("`db`: SQL vazio".to_string());
        }
        let text = c_string(sql, "preparar SQL")?;
        let mut stmt: *mut sqlite3_stmt = std::ptr::null_mut();
        let mut tail: *const c_char = std::ptr::null();
        let code = unsafe {
            sqlite3_prepare_v2(
                self.db,
                text.as_ptr(),
                -1,
                &mut stmt,
                &mut tail as *mut *const c_char,
            )
        };
        if code != SQLITE_OK || stmt.is_null() {
            return Err(last_error(self.db, "preparar SQL"));
        }
        // Uma instrucao por chamada (resto nao-branco = erro claro).
        let rest = unsafe {
            if tail.is_null() {
                ""
            } else {
                CStr::from_ptr(tail).to_str().unwrap_or("")
            }
        };
        if !rest.trim().trim_matches(';').trim().is_empty() {
            unsafe {
                sqlite3_finalize(stmt);
            }
            return Err("`db`: uma instrucao por chamada".to_string());
        }
        Ok(stmt)
    }

    fn bind_all(&self, stmt: *mut sqlite3_stmt, params: &[SqlValue]) -> Result<(), String> {
        let expected = unsafe { sqlite3_bind_parameter_count(stmt) };
        if expected as usize != params.len() {
            return Err(format!(
                "`db`: esperava {expected} parametro(s), recebeu {}",
                params.len()
            ));
        }
        for (i, param) in params.iter().enumerate() {
            let idx = (i + 1) as c_int;
            let code = match param {
                SqlValue::Int(v) => unsafe { sqlite3_bind_int64(stmt, idx, *v) },
                SqlValue::Float(v) => unsafe { sqlite3_bind_double(stmt, idx, *v) },
                SqlValue::Text(s) => {
                    let text = c_string(s, "parametro texto")?;
                    unsafe {
                        sqlite3_bind_text(
                            stmt,
                            idx,
                            text.as_ptr(),
                            s.len() as c_int,
                            SQLITE_TRANSIENT,
                        )
                    }
                }
                SqlValue::Null => unsafe { sqlite3_bind_null(stmt, idx) },
            };
            if code != SQLITE_OK {
                return Err(last_error(self.db, "vincular parametro"));
            }
        }
        Ok(())
    }

    /// Executa (INSERT/UPDATE/DELETE/DDL...); devolve linhas afetadas.
    /// Linhas de SELECT sao descartadas (use `query` para ler).
    pub fn exec(&self, sql: &str, params: &[SqlValue]) -> Result<i64, String> {
        let stmt = self.prepare(sql)?;
        let guard = StmtGuard { stmt };
        self.bind_all(guard.stmt, params)?;
        // CONN-004: `exec(SELECT)` devolvia `sqlite3_changes` obsoleto (o do
        // DML anterior). SELECT nao tem "linhas afetadas": devolve 0
        // (o guard descarta o statement; sem stepped rows).
        // (Vinculo antes: aridade/tipo de parametro continua checada.)
        if unsafe { sqlite3_column_count(guard.stmt) } > 0 {
            return Ok(0);
        }
        loop {
            let code = unsafe { sqlite3_step(guard.stmt) };
            match code {
                SQLITE_ROW => {}
                SQLITE_DONE => break,
                _ => return Err(last_error(self.db, "executar SQL")),
            }
        }
        Ok(unsafe { sqlite3_changes(self.db) } as i64)
    }

    /// Consulta; devolve linhas como mapas coluna => valor.
    pub fn query(
        &self,
        sql: &str,
        params: &[SqlValue],
    ) -> Result<Vec<HashMap<String, SqlColumn>>, String> {
        let stmt = self.prepare(sql)?;
        let guard = StmtGuard { stmt };
        self.bind_all(guard.stmt, params)?;
        let count = unsafe { sqlite3_column_count(guard.stmt) };
        if count < 0 {
            return Err(last_error(self.db, "ler colunas"));
        }
        let mut names = Vec::with_capacity(count as usize);
        for col in 0..count {
            let raw = unsafe { sqlite3_column_name(guard.stmt, col) };
            if raw.is_null() {
                return Err(last_error(self.db, "ler nome de coluna"));
            }
            let name = unsafe { CStr::from_ptr(raw) }
                .to_str()
                .map_err(|_| "`db`: nome de coluna nao UTF-8".to_string())?
                .to_string();
            names.push(name);
        }
        let mut rows = Vec::new();
        loop {
            let code = unsafe { sqlite3_step(guard.stmt) };
            match code {
                SQLITE_ROW => {
                    let mut row = HashMap::with_capacity(names.len());
                    for (col, name) in names.iter().enumerate() {
                        row.insert(name.clone(), Self::column_value(guard.stmt, col as c_int)?);
                    }
                    rows.push(row);
                }
                SQLITE_DONE => break,
                _ => return Err(last_error(self.db, "executar consulta")),
            }
        }
        Ok(rows)
    }

    fn column_value(stmt: *mut sqlite3_stmt, col: c_int) -> Result<SqlColumn, String> {
        let kind = unsafe { sqlite3_column_type(stmt, col) };
        match kind {
            SQLITE_NULL => Ok(SqlColumn::Null),
            SQLITE_INTEGER => Ok(SqlColumn::Int(unsafe { sqlite3_column_int64(stmt, col) })),
            SQLITE_FLOAT => Ok(SqlColumn::Float(unsafe {
                sqlite3_column_double(stmt, col)
            })),
            SQLITE_TEXT => {
                let raw = unsafe { sqlite3_column_text(stmt, col) };
                if raw.is_null() {
                    return Err("`db`: texto nulo da coluna".to_string());
                }
                let len = unsafe { sqlite3_column_bytes(stmt, col) }.max(0) as usize;
                let bytes = unsafe { std::slice::from_raw_parts(raw.cast::<u8>(), len) };
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| "`db`: texto nao UTF-8".to_string())?
                    .to_string();
                Ok(SqlColumn::Text(text))
            }
            _ => Err("`db`: tipo nao suportado (BLOB/binario; RHP nao tem bytes)".to_string()),
        }
    }
}

/// Guarda que finaliza o prepared statement em todos os caminhos.
struct StmtGuard {
    stmt: *mut sqlite3_stmt,
}

impl Drop for StmtGuard {
    fn drop(&mut self) {
        if !self.stmt.is_null() {
            unsafe {
                sqlite3_finalize(self.stmt);
            }
            self.stmt = std::ptr::null_mut();
        }
    }
}

/// Valor lido de coluna.
#[derive(Debug, Clone)]
pub enum SqlColumn {
    Int(i64),
    Float(f64),
    Text(String),
    Null,
}
