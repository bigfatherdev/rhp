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
        Stmt::Const { name, init } => {
            indent(level, out);
            let _ = writeln!(out, "const {name} = {};", fmt_expr(init, 0));
        }
        Stmt::Break => {
            indent(level, out);
            out.push_str("break;\n");
        }
        Stmt::Continue => {
            indent(level, out);
            out.push_str("continue;\n");
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
        Stmt::Try {
            body,
            catch_var,
            catch_body,
            finally_body,
        } => {
            indent(level, out);
            out.push_str("try {\n");
            fmt_block(body, level + 1, out);
            indent(level, out);
            if let Some(catch_body) = catch_body {
                match catch_var {
                    Some(var) => out.push_str(&format!("}} catch {var} {{\n")),
                    None => out.push_str("} catch {\n"),
                }
                fmt_block(catch_body, level + 1, out);
                indent(level, out);
            }
            match finally_body {
                Some(finally_body) => {
                    out.push_str("} finally {\n");
                    fmt_block(finally_body, level + 1, out);
                    indent(level, out);
                    out.push_str("}\n");
                }
                None => out.push_str("}\n"),
            }
        }
        Stmt::Throw { expr } => {
            indent(level, out);
            let _ = writeln!(out, "throw {};", fmt_expr(expr, 0));
        }
        Stmt::For { var, iter, body } => {
            indent(level, out);
            let _ = writeln!(out, "for {var} in {} {{", fmt_expr(iter, 0));
            fmt_block(body, level + 1, out);
            indent(level, out);
            out.push_str("}\n");
        }
        Stmt::Switch {
            discriminant,
            cases,
        } => {
            indent(level, out);
            let _ = writeln!(out, "switch {} {{", fmt_expr(discriminant, 0));
            for case in cases {
                indent(level + 1, out);
                match &case.cond {
                    Some(cond) => {
                        let _ = writeln!(out, "case {} {{", fmt_expr(cond, 0));
                    }
                    None => out.push_str("default {\n"),
                }
                fmt_block(&case.body, level + 2, out);
                indent(level + 1, out);
                out.push_str("}\n");
            }
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
        Stmt::ImportNames { path, names } => {
            indent(level, out);
            let parts: Vec<String> = names
                .iter()
                .map(|n| match &n.alias {
                    Some(alias) => format!("{} as {alias}", n.name),
                    None => n.name.clone(),
                })
                .collect();
            let _ = writeln!(out, "import {{{}}} from {path:?};", parts.join(", "));
        }
        Stmt::Export { def } => {
            indent(level, out);
            out.push_str("export ");
            // `def` e `fn`/`let`/`const`: formata sem indentacao extra e
            // troca a quebra final por `;`/bloco na mesma linha logica.
            let mut inner = String::new();
            fmt_stmt(def, 0, &mut inner);
            let inner = inner.trim_end();
            let _ = writeln!(out, "{inner}");
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
/// Espelha a gramatica do parser (ternario < `??` < igualdade < bits).
fn prec_of(expr: &Expr) -> u8 {
    match expr {
        Expr::Ternary { .. } => 0,
        Expr::Binary { op, .. } => match op {
            BinOp::NullCoalesce => 1,
            BinOp::Eq | BinOp::Ne => 2,
            BinOp::BitOr => 3,
            BinOp::BitXor => 4,
            BinOp::BitAnd => 5,
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 6,
            BinOp::Shl | BinOp::Shr => 7,
            BinOp::Add | BinOp::Sub => 8,
            BinOp::Mul | BinOp::Div | BinOp::Mod => 9,
        },
        Expr::Unary { .. } => 10,
        _ => 11,
    }
}

fn fmt_expr(expr: &Expr, parent_prec: u8) -> String {
    match expr {
        Expr::StringLiteral(s) => format!("{s:?}"),
        Expr::Int(v) => v.to_string(),
        // `Display` de `f64` nunca usa notacao cientifica (ao contrario de
        // `{:?}`); sem ponto, completa com `.0` para continuar literal
        // float valido e reparavel (AUD-101).
        Expr::Float(v) => {
            let mut text = format!("{v}");
            if !text.contains('.') {
                text.push_str(".0");
            }
            text
        }
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
                .map(|(key, value)| format!("{}: {}", fmt_map_key(key), fmt_expr(value, 0)))
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
                UnaryOp::BitNot => "~",
            };
            // O proprio filho binario adiciona parenteses quando
            // `prec < 10`; aqui basta concatenar.
            format!("{symbol}{}", fmt_expr(expr, 10))
        }
        Expr::Ternary {
            cond,
            then_branch,
            else_branch,
        } => {
            let inner = format!(
                "{} ? {} : {}",
                fmt_expr(cond, 1),
                fmt_expr(then_branch, 1),
                fmt_expr(else_branch, 0)
            );
            if 0 < parent_prec {
                format!("({inner})")
            } else {
                inner
            }
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
                BinOp::BitAnd => "&",
                BinOp::BitOr => "|",
                BinOp::BitXor => "^",
                BinOp::Shl => "<<",
                BinOp::Shr => ">>",
                BinOp::NullCoalesce => "??",
            };
            let my_prec = prec_of(expr);
            let left_text = fmt_expr(left, my_prec);
            // Associatividade a esquerda (padrao): o lado direito com mesma
            // precedencia precisa de parenteses (`a - (b - c)`).
            // Excecao: `??` e associativo a direita (`a ?? b ?? c`).
            let right_text = match op {
                BinOp::NullCoalesce => fmt_expr(right, my_prec),
                _ => fmt_expr(right, my_prec + 1),
            };
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
        Expr::CallValue { callee, args } => {
            let inner: Vec<String> = args.iter().map(|arg| fmt_expr(arg, 0)).collect();
            format!("{}({})", fmt_atom(callee), inner.join(", "))
        }
        Expr::Closure(data) => {
            // Multilinha com indentacao relativa: estavel e reparsavel
            // (idempotencia garantida; closures aninhadas nao herdam o
            // nivel absoluto — aceito nesta etapa, ver RFC 0003 §7).
            let mut body = String::new();
            fmt_block(&data.body, 1, &mut body);
            format!("fn ({}) {{\n{body}}}", data.params.join(", "))
        }
    }
}

/// Chave de mapa: nua se identificador valido, com aspas caso contrario
/// (chaves com espacos/simbolos precisam de `"..."` para reparsear).
fn fmt_map_key(key: &str) -> String {
    let mut bytes = key.bytes();
    let bare = !key.is_empty()
        && bytes
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if bare {
        key.to_string()
    } else {
        format!("{key:?}")
    }
}

/// Atomo posfixado: variavel, literal, chamada, indice ou literal composto.
fn fmt_atom(expr: &Expr) -> String {
    match expr {
        Expr::Var(_)
        | Expr::Call { .. }
        | Expr::CallValue { .. }
        | Expr::Closure(_)
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
    fn formats_switch_statement() {
        assert_eq!(
            fmt("switch x{case 1{echo \"a\";}default{echo \"d\";}}"),
            "switch x {\n  case 1 {\n    echo \"a\";\n  }\n  default {\n    echo \"d\";\n  }\n}\n"
        );
        // Idempotente + reparse preserva a AST (inclusive fallthrough e aninhado).
        check_idempotent(
            "switch x { case 1 { echo \"a\"; } default { echo \"d\"; } case 2 { echo \"b\"; } }",
        );
        check_idempotent(
            "while i < 3 { switch i { case 2 { continue; } default { echo i; break; } } }",
        );
    }

    #[test]
    fn formats_closures_and_general_calls() {
        assert_eq!(
            fmt("let f=fn(x){return x+1;};"),
            "let f = fn (x) {\n  return x + 1;\n};\n"
        );
        // IIFE dispa parenteses externos redundantes, mantem os da chamada.
        check_idempotent("echo (fn (x) { return x * 2; })(21);");
        check_idempotent("let f = fn (x) { return fn (y) { return x + y; }; };");
        check_idempotent("echo f(40)(2);");
    }

    #[test]
    fn formats_control_flow_and_functions() {
        let src = "fn f(a,b){if a{echo 1;}else{echo 2;}while a{a=a-1;}return a;}";
        let expected = "fn f(a, b) {\n  if a {\n    echo 1;\n  } else {\n    echo 2;\n  }\n  while a {\n    a = a - 1;\n  }\n  return a;\n}\n";
        assert_eq!(fmt(src), expected);
        check_idempotent(src);
    }

    #[test]
    fn formats_bitwise_ternary_and_control() {
        assert_eq!(fmt("echo 1 + 2 << 3;"), "echo 1 + 2 << 3;\n");
        assert_eq!(fmt("echo a & b | c;"), "echo a & b | c;\n");
        assert_eq!(fmt("echo ~x;"), "echo ~x;\n");
        assert_eq!(fmt("echo a ? b : c;"), "echo a ? b : c;\n");
        assert_eq!(fmt("echo a ?? b;"), "echo a ?? b;\n");
        // `??` e associativo a direita: formata sem parenteses redundantes.
        assert_eq!(fmt("echo a ?? b ?? c;"), "echo a ?? b ?? c;\n");
        // Ternario aninhado a esquerda precisa de parenteses.
        assert_eq!(
            fmt("echo (a ? b : c) ? d : e;"),
            "echo (a ? b : c) ? d : e;\n"
        );
        check_idempotent("const N = 10;\nwhile true {\n  if N > 1 {\n    break;\n  } else {\n    continue;\n  }\n}\n");
    }

    #[test]
    fn formats_for_loop_canonically() {
        assert_eq!(
            fmt("for x in [1,2]{echo x;}"),
            "for x in [1, 2] {\n  echo x;\n}\n"
        );
        check_idempotent(
            "for k in {b: 1, a: 2} {\n  if k == \"a\" {\n    continue;\n  }\n  break;\n}\n",
        );
        // `in` continua identificador comum fora do `for`.
        check_idempotent("let in = 1;\necho in;\n");
    }

    #[test]
    fn formats_try_catch_finally_and_throw() {
        assert_eq!(
            fmt("try{echo 1;}catch e{echo e;}finally{echo 2;}"),
            "try {\n  echo 1;\n} catch e {\n  echo e;\n} finally {\n  echo 2;\n}\n"
        );
        assert_eq!(
            fmt("try{echo 1;}catch{echo 2;}"),
            "try {\n  echo 1;\n} catch {\n  echo 2;\n}\n"
        );
        assert_eq!(fmt("throw 1+2;"), "throw 1 + 2;\n");
        check_idempotent(
            "try {\n  echo 1;\n} catch e {\n  echo e;\n} finally {\n  echo 2;\n}\nthrow \"x\";\n",
        );
    }

    #[test]
    fn formats_export_and_selective_import() {
        assert_eq!(
            fmt("export fn dobro(x){return x+x;}"),
            "export fn dobro(x) {\n  return x + x;\n}\n"
        );
        assert_eq!(fmt("export let base=40;"), "export let base = 40;\n");
        assert_eq!(
            fmt("import {dobro,base as b} from \"mat.rhp\";"),
            "import {dobro, base as b} from \"mat.rhp\";\n"
        );
        check_idempotent(
            "export fn dobro(x) {\n  return x + x;\n}\nimport {dobro as d} from \"m.rhp\";\n",
        );
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
    fn formats_floats_reparseably() {
        assert_eq!(fmt("echo 0.000001;"), "echo 0.000001;\n");
        assert_eq!(fmt("echo 2.0;"), "echo 2.0;\n");
        check_idempotent("echo 0.000001;\necho 2.0;\necho 1.5;\n");
    }

    #[test]
    fn quotes_map_keys_when_needed() {
        assert_eq!(fmt("echo {\"a b\": 1};"), "echo {\"a b\": 1};\n");
        assert_eq!(fmt("echo {x: 1};"), "echo {x: 1};\n");
        check_idempotent("echo {\"a b\": 1, x: 2};\n");
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(format_source("echo \"x\"").is_err());
    }
}
