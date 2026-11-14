//! Formatador oficial do RHP (Fase 5).
//!
//! Formato canonico: indentacao de 2 espacos, `;` sempre terminando a
//! instrucao, parenteses minimos por precedencia, arquivo terminado por
//! exatamente uma quebra de linha. O formatador e idempotente:
//! `format(parse(format(p))) == format(p)`.

use rhp_parser::{AssignTarget, BinOp, Expr, Program, Stmt, UnaryOp};
use std::fmt::Write as _;

/// Formata o programa no estilo canonico.
///
/// A saida termina sempre com exatamente um `\n` (programa vazio => `""`).
#[must_use]
pub fn format(program: &Program) -> String {
    let mut out = String::new();
    for stmt in &program.statements {
        fmt_stmt(stmt, 0, &mut out);
    }
    out
}

/// Formata fonte RHP (atalho lex+parse+format).
///
/// # Erros
///
/// Retorna string de erro ja formatada quando o fonte nao analisa.
pub fn format_source(source: &str) -> Result<String, String> {
    let tokens = rhp_lexer::lex(source).map_err(|e| format!("erro lexico em {e}"))?;
    let program = rhp_parser::parse(&tokens).map_err(|e| format!("erro sintatico em {e}"))?;
    Ok(format(&program))
}

fn indent(level: usize, out: &mut String) {
    for _ in 0..level {
        out.push_str("  ");
    }
}

fn fmt_stmt(stmt: &Stmt, level: usize, out: &mut String) {
    match stmt {
        Stmt::Echo { expr } => {
            indent(level, out);
            let _ = writeln!(out, "echo {};", fmt_expr(expr, 0));
        }
        Stmt::Let { name, init } => {
            indent(level, out);
            match init {
                Some(expr) => {
                    let _ = writeln!(out, "let {name} = {};", fmt_expr(expr, 0));
                }
                None => {
                    let _ = writeln!(out, "let {name};");
                }
            }
        }
        Stmt::Assign { target, expr } => {
            indent(level, out);
            match target {
                AssignTarget::Var(name) => {
                    let _ = writeln!(out, "{name} = {};", fmt_expr(expr, 0));
                }
                AssignTarget::Index { name, index } => {
                    let _ = writeln!(
                        out,
                        "{name}[{}] = {};",
                        fmt_expr(index, 0),
                        fmt_expr(expr, 0)
                    );
                }
            }
        }
        Stmt::If {
            cond,
            then_branch,
            else_branch,
        } => {
            indent(level, out);
            let _ = writeln!(out, "if {} {{", fmt_expr(cond, 0));
            fmt_block(then_branch, level + 1, out);
            indent(level, out);
            match else_branch {
                Some(else_branch) => {
                    out.push_str("} else {\n");
                    fmt_block(else_branch, level + 1, out);
                    indent(level, out);
                    out.push_str("}\n");
                }
                None => out.push_str("}\n"),
            }
        }
        Stmt::While { cond, body } => {
            indent(level, out);
            let _ = writeln!(out, "while {} {{", fmt_expr(cond, 0));
            fmt_block(body, level + 1, out);
            indent(level, out);
            out.push_str("}\n");
        }
        Stmt::FnDef { name, params, body } => {
            indent(level, out);
            let _ = writeln!(out, "fn {name}({}) {{", params.join(", "));
            fmt_block(body, level + 1, out);
            indent(level, out);
            out.push_str("}\n");
        }
        Stmt::Return { expr } => {
            indent(level, out);
            match expr {
                Some(expr) => {
                    let _ = writeln!(out, "return {};", fmt_expr(expr, 0));
                }
                None => out.push_str("return;\n"),
            }
        }
        Stmt::Import(path) => {
            indent(level, out);
            let _ = writeln!(out, "import {path:?};");
        }
        Stmt::Block(stmts) => {
            indent(level, out);
            out.push_str("{\n");
            fmt_block(stmts, level + 1, out);
            indent(level, out);
            out.push_str("}\n");
        }
        Stmt::ExprStmt { expr } => {
            indent(level, out);
            let _ = writeln!(out, "{};", fmt_expr(expr, 0));
        }
    }
}

fn fmt_block(stmts: &[Stmt], level: usize, out: &mut String) {
    for stmt in stmts {
        fmt_stmt(stmt, level, out);
    }
}

/// Precedencia para parenteses minimos (maior = liga mais forte).
fn prec_of(expr: &Expr) -> u8 {
    match expr {
        Expr::Binary { op, .. } => match op {
            BinOp::Eq | BinOp::Ne => 1,
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 2,
            BinOp::Add | BinOp::Sub => 3,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 4,
        },
        Expr::Unary { .. } => 5,
        _ => 6,
    }
}

fn fmt_expr(expr: &Expr, parent_prec: u8) -> String {
    match expr {
        Expr::StringLiteral(s) => format!("{s:?}"),
        Expr::Int(v) => v.to_string(),
        // `{:?}` em float garante ponto decimal (`2.0`, nao `2`).
        Expr::Float(v) => format!("{v:?}"),
        Expr::Bool(b) => b.to_string(),
        Expr::Null => "null".to_string(),
        Expr::Var(name) => name.clone(),
        Expr::Array(items) => {
            let inner: Vec<String> = items.iter().map(|item| fmt_expr(item, 0)).collect();
            format!("[{}]", inner.join(", "))
        }
        Expr::Map(entries) => {
            let inner: Vec<String> = entries
                .iter()
                .map(|(key, value)| format!("{key}: {}", fmt_expr(value, 0)))
                .collect();
            format!("{{{}}}", inner.join(", "))
        }
        Expr::Index { base, index } => {
            format!("{}[{}]", fmt_atom(base), fmt_expr(index, 0))
        }
        Expr::Unary { op, expr } => {
            let symbol = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            // O proprio filho binario adiciona parenteses quando
            // `prec < 5`; aqui basta concatenar.
            format!("{symbol}{}", fmt_expr(expr, 5))
        }
        Expr::Binary { op, left, right } => {
            let symbol = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Mod => "%",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Lt => "<",
                BinOp::Le => "<=",
                BinOp::Gt => ">",
                BinOp::Ge => ">=",
            };
            let my_prec = prec_of(expr);
            let left_text = fmt_expr(left, my_prec);
            // Associatividade a esquerda: o lado direito com mesma
            // precedencia precisa de parenteses (`a - (b - c)`).
            let right_text = fmt_expr(right, my_prec + 1);
            let inner = format!("{left_text} {symbol} {right_text}");
            if my_prec < parent_prec {
                format!("({inner})")
            } else {
                inner
            }
        }
        Expr::Call { name, args } => {
            let inner: Vec<String> = args.iter().map(|arg| fmt_expr(arg, 0)).collect();
            format!("{name}({})", inner.join(", "))
        }
    }
}

/// Atomo posfixado: variavel, literal, chamada, indice ou literal composto.
fn fmt_atom(expr: &Expr) -> String {
    match expr {
        Expr::Var(_)
        | Expr::Call { .. }
        | Expr::Index { .. }
        | Expr::Array(_)
        | Expr::Map(_)
        | Expr::StringLiteral(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::Bool(_)
        | Expr::Null => fmt_expr(expr, 0),
        _ => format!("({})", fmt_expr(expr, 0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format_source(src).expect("fonte de teste deve formatar")
    }

    fn check_idempotent(src: &str) {
        let once = fmt(src);
        let twice = fmt(&once);
        assert_eq!(once, twice, "formatador deve ser idempotente");
        // Reparse do formatado preserva a AST.
        let ast1 = rhp_parser::parse_source(src).expect("parse deve passar");
        let ast2 = rhp_parser::parse_source(&once).expect("reparse deve passar");
        assert_eq!(ast1, ast2, "reparse deve preservar a AST");
    }

    #[test]
    fn formats_hello_canonically() {
        assert_eq!(
            fmt("echo   \"Hello from RHP!\";"),
            "echo \"Hello from RHP!\";\n"
        );
    }

    #[test]
    fn keeps_minimal_parens() {
        assert_eq!(fmt("echo 1 + 2 * 3;"), "echo 1 + 2 * 3;\n");
        assert_eq!(fmt("echo (1 + 2) * 3;"), "echo (1 + 2) * 3;\n");
        assert_eq!(fmt("echo 10 - (4 - 3);"), "echo 10 - (4 - 3);\n");
        assert_eq!(fmt("echo -(1 + 2);"), "echo -(1 + 2);\n");
        assert_eq!(fmt("echo !true;"), "echo !true;\n");
    }

    #[test]
    fn formats_control_flow_and_functions() {
        let src = "fn f(a,b){if a{echo 1;}else{echo 2;}while a{a=a-1;}return a;}";
        let expected = "fn f(a, b) {\n  if a {\n    echo 1;\n  } else {\n    echo 2;\n  }\n  while a {\n    a = a - 1;\n  }\n  return a;\n}\n";
        assert_eq!(fmt(src), expected);
        check_idempotent(src);
    }

    #[test]
    fn formats_compounds_and_import() {
        let src = "import\"l.rhp\";let a=[1,2];let m={x:1};a[0]=m[\"x\"];";
        let expected = "import \"l.rhp\";\nlet a = [1, 2];\nlet m = {x: 1};\na[0] = m[\"x\"];\n";
        assert_eq!(fmt(src), expected);
        check_idempotent(src);
    }

    #[test]
    fn formats_fib_example_idempotently() {
        let fib = include_str!("../../../examples/fib.rhp");
        check_idempotent(fib);
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(format_source("echo \"x\"").is_err());
    }
}
