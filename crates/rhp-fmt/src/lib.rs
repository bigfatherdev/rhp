//! Formatador oficial do RHP (Fase 5).
//!
//! Formato canonico: indentacao de 2 espacos, `;` sempre terminando a
//! instrucao, parenteses minimos por precedencia, arquivo terminado por
//! exatamente uma quebra de linha. O formatador e idempotente:
//! `format(parse(format(p))) == format(p)`.

use rhp_parser::{
    AssignTarget, BinOp, CallArg, DestructureTarget, Expr, Param, Program, Stmt, StmtNode, UnaryOp,
};
use std::fmt::Write as _;

/// Formata o programa no estilo canonico.
///
/// Comentarios e linhas em branco (Etapa 4): cada statement reemite seus
/// `leading` (um por linha), `trailing` (fim da linha final) e ate uma
/// linha em branco antes; comentarios apos o ultimo statement vao ao fim.
/// Idempotente: `format(parse(format(p))) == format(p)`.
#[must_use]
pub fn format(program: &Program) -> String {
    let mut out = String::new();
    for (i, node) in program.statements.iter().enumerate() {
        fmt_node(node, 0, i > 0, &mut out);
    }
    for text in &program.trailing_comments {
        let _ = writeln!(out, "//{text}");
    }
    out
}

/// Statement com decoracao (comentarios + linha em branco).
fn fmt_node(node: &StmtNode, level: usize, not_first: bool, out: &mut String) {
    if not_first && node.blank_before {
        out.push('\n');
    }
    for text in &node.leading {
        indent(level, out);
        let _ = writeln!(out, "//{text}");
    }
    fmt_stmt(&node.stmt, level, out);
    if let Some(text) = node.trailing.as_ref() {
        // Todo braco de `fmt_stmt` termina a linha com `\n`: reinsere o
        // comentario de fim de linha antes dela.
        debug_assert!(out.ends_with('\n'));
        out.pop();
        let _ = writeln!(out, " //{text}");
    }
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
        Stmt::Let { name, ann, init } => {
            indent(level, out);
            let ann = ann.map(|t| format!(": {}", t.name())).unwrap_or_default();
            match init {
                Some(expr) => {
                    let _ = writeln!(out, "let {name}{ann} = {};", fmt_expr(expr, 0));
                }
                None => {
                    let _ = writeln!(out, "let {name}{ann};");
                }
            }
        }
        Stmt::Const { name, ann, init } => {
            indent(level, out);
            let ann = ann.map(|t| format!(": {}", t.name())).unwrap_or_default();
            let _ = writeln!(out, "const {name}{ann} = {};", fmt_expr(init, 0));
        }
        Stmt::Static { name, init } => {
            indent(level, out);
            let _ = writeln!(out, "static {name} = {};", fmt_expr(init, 0));
        }
        Stmt::Destructure {
            target,
            init,
            is_const,
        } => {
            indent(level, out);
            let keyword = if *is_const { "const" } else { "let" };
            let pattern = match target {
                DestructureTarget::Positional(names) => {
                    format!("[{}]", names.join(", "))
                }
                DestructureTarget::ByKey(pairs) => {
                    let parts: Vec<String> = pairs
                        .iter()
                        .map(|(key, name)| {
                            if key == name {
                                key.clone()
                            } else {
                                format!("{key}: {name}")
                            }
                        })
                        .collect();
                    format!("{{{}}}", parts.join(", "))
                }
            };
            let _ = writeln!(out, "{keyword} {pattern} = {};", fmt_expr(init, 0));
        }
        Stmt::Break => {
            indent(level, out);
            out.push_str("break;\n");
        }
        Stmt::Continue => {
            indent(level, out);
            out.push_str("continue;\n");
        }
        Stmt::BreakN(n) => {
            indent(level, out);
            let _ = writeln!(out, "break {n};");
        }
        Stmt::ContinueN(n) => {
            indent(level, out);
            let _ = writeln!(out, "continue {n};");
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
        Stmt::ForC {
            init,
            cond,
            step,
            body,
        } => {
            indent(level, out);
            let _ = writeln!(out, "{} {{", fmt_for_c_head(init, cond, step));
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
        Stmt::FnDef {
            name,
            params,
            variadic,
            return_ann,
            body,
        } => {
            indent(level, out);
            let ret = return_ann
                .map(|t| format!(": {}", t.name()))
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "fn {name}({}){ret} {{",
                fmt_params(params, variadic.as_deref())
            );
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
        Stmt::ReExport { path, names } => {
            indent(level, out);
            let parts: Vec<String> = names
                .iter()
                .map(|n| match &n.alias {
                    Some(alias) => format!("{} as {alias}", n.name),
                    None => n.name.clone(),
                })
                .collect();
            let _ = writeln!(out, "export {{{}}} from {path:?};", parts.join(", "));
        }
        Stmt::Export { def } => {
            indent(level, out);
            out.push_str("export ");
            // `def` e `fn`/`let`/`const`: formata sem indentacao extra e
            // troca a quebra final por `;`/bloco na mesma linha logica.
            let mut inner = String::new();
            fmt_node(def, 0, false, &mut inner);
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

fn fmt_block(stmts: &[StmtNode], level: usize, out: &mut String) {
    for (i, node) in stmts.iter().enumerate() {
        fmt_node(node, level, i > 0, out);
    }
}

/// Lista de parametros canonica: `a, b = <expr>, ...rest` (RFC 0006).
fn fmt_params(params: &[Param], variadic: Option<&str>) -> String {
    let mut parts: Vec<String> = params
        .iter()
        .map(|p| {
            let ann = p.ann.map(|t| format!(": {}", t.name())).unwrap_or_default();
            match &p.default {
                Some(d) => format!("{}{ann} = {}", p.name, fmt_expr(d, 0)),
                None => format!("{}{ann}", p.name),
            }
        })
        .collect();
    if let Some(rest) = variadic {
        parts.push(format!("...{rest}"));
    }
    parts.join(", ")
}

/// Statement em uma linha (cabecalho do for-C; RFC 0008): formata com
/// nivel 0 e remove a quebra final. So usado com `let`/`const`/atribuicao
/// (o parser restringe init/passo a formas de linha unica).
fn fmt_stmt_one_line(stmt: &Stmt) -> String {
    let mut out = String::new();
    fmt_stmt(stmt, 0, &mut out);
    out.trim_end().to_string()
}

/// Cabecalho `for init; cond; passo` (partes vazias ok; RFC 0008).
fn fmt_for_c_head(
    init: &Option<Box<StmtNode>>,
    cond: &Option<Expr>,
    step: &Option<Box<StmtNode>>,
) -> String {
    let mut head = String::from("for");
    match init {
        Some(node) => {
            head.push(' ');
            head.push_str(&fmt_stmt_one_line(&node.stmt));
        }
        None => head.push_str(" ;"),
    }
    match cond {
        Some(expr) => {
            head.push(' ');
            head.push_str(&fmt_expr(expr, 0));
            head.push(';');
        }
        None => head.push_str(" ;"),
    }
    if let Some(node) = step {
        head.push(' ');
        head.push_str(
            fmt_stmt_one_line(&node.stmt)
                .trim_end_matches(';')
                .trim_end(),
        );
    }
    head
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
            let inner: Vec<String> = args.iter().map(fmt_call_arg).collect();
            format!("{name}({})", inner.join(", "))
        }
        Expr::CallValue { callee, args } => {
            let inner: Vec<String> = args.iter().map(fmt_call_arg).collect();
            format!("{}({})", fmt_atom(callee), inner.join(", "))
        }
        Expr::Closure(data) => {
            // Multilinha com indentacao relativa: estavel e reparsavel
            // (idempotencia garantida; closures aninhadas nao herdam o
            // nivel absoluto — aceito nesta etapa, ver RFC 0003 §7).
            let mut body = String::new();
            fmt_block(&data.body, 1, &mut body);
            let ret = data
                .return_ann
                .map(|t| format!(": {}", t.name()))
                .unwrap_or_default();
            format!(
                "fn ({}){ret} {{\n{body}}}",
                fmt_params(&data.params, data.variadic.as_deref())
            )
        }
        Expr::Match { subject, arms } => {
            // Como closures: indentacao relativa (estavel e reparsavel;
            // `match` aninhado nao herda o nivel absoluto).
            let mut out = format!("match {} {{\n", fmt_expr(subject, 0));
            for arm in arms {
                let cond = match &arm.cond {
                    Some(cond) => fmt_expr(cond, 0),
                    None => "default".to_string(),
                };
                out.push_str(&format!("  {cond} => {},\n", fmt_expr(&arm.value, 0)));
            }
            out.push('}');
            out
        }
    }
}

/// Argumento de chamada canonico (RFC 0009): posicional, `nome: expr`
/// ou `...expr`.
fn fmt_call_arg(arg: &CallArg) -> String {
    match arg {
        CallArg::Pos(expr) => fmt_expr(expr, 0),
        CallArg::Named(name, expr) => format!("{name}: {}", fmt_expr(expr, 0)),
        CallArg::Spread(expr) => format!("...{}", fmt_expr(expr, 0)),
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
    fn formats_defaults_and_variadic() {
        assert_eq!(
            fmt("fn f(a,b=2,...rest){return a;}"),
            "fn f(a, b = 2, ...rest) {\n  return a;\n}\n"
        );
        check_idempotent("let g = fn (a = 1 + 2, ...r) { return a; };");
        check_idempotent("fn f(a = [1, 2], b = {x: 1}) { return a; }");
    }

    #[test]
    fn formats_destructuring() {
        assert_eq!(fmt("let [a,b]=[1,2];"), "let [a, b] = [1, 2];\n");
        assert_eq!(
            fmt("const {k:v,x}={k:1,x:2};"),
            "const {k: v, x} = {k: 1, x: 2};\n"
        );
        check_idempotent("let [a, b] = f([1, 2]);");
        check_idempotent("const {k: v} = m;");
    }

    #[test]
    fn formats_match_expression() {
        assert_eq!(
            fmt("echo match x{1=>\"a\",default=>\"b\"};"),
            "echo match x {\n  1 => \"a\",\n  default => \"b\",\n};\n"
        );
        // Aninhado: indentacao relativa, mas estavel e reparsavel.
        check_idempotent(
            "echo match x { 1 => match y { 1 => \"a\", default => \"b\" }, default => \"c\" };",
        );
        check_idempotent("let v = match f(1) { 1 => [1], default => [] };");
    }

    #[test]
    fn formats_mixed_calls() {
        assert_eq!(fmt("echo f(1,x:2,...r);"), "echo f(1, x: 2, ...r);\n");
        check_idempotent("echo f(...a, ...b);");
        check_idempotent("echo f(x: g(1), y: [1, {k: 2}]);");
    }

    #[test]
    fn formats_static_statement() {
        assert_eq!(
            fmt("fn f(){static n=0;echo n;}"),
            "fn f() {\n  static n = 0;\n  echo n;\n}\n"
        );
        check_idempotent("fn f() { static n = [1, {x: 2}]; return n; }");
    }

    #[test]
    fn formats_reexport_statement() {
        assert_eq!(
            fmt("export{a,b as c}from\"m.rhp\";"),
            "export {a, b as c} from \"m.rhp\";\n"
        );
        check_idempotent("export { a } from \"lib.rhp\";");
    }

    #[test]
    fn formats_elvis_arrow_forc_and_break_levels() {
        // Elvis desugara para `??` no parse (fmt mostra a forma canonica).
        assert_eq!(fmt("echo a?:b;"), "echo a ?? b;\n");
        // Arrow normaliza para bloco.
        assert_eq!(
            fmt("let f=fn(x)=>x+1;"),
            "let f = fn (x) {\n  return x + 1;\n};\n"
        );
        // For-C canonico; niveis de break/continue preservados.
        assert_eq!(
            fmt("for let i=0;i<3;i=i+1{echo i;}"),
            "for let i = 0; i < 3; i = i + 1 {\n  echo i;\n}\n"
        );
        assert_eq!(fmt("for;;{break 2;}"), "for ; ; {\n  break 2;\n}\n");
        check_idempotent("for ; x < 3; x = x + 1 { echo x; }");
        check_idempotent("while true { switch x { case 1 { continue 2; } default { break; } } }");
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
    fn preserves_type_annotations() {
        // Etapa 5 (RFC 0016): anotacoes reemitidas no canonico.
        assert_eq!(fmt("let x:int=1;"), "let x: int = 1;\n");
        assert_eq!(
            fmt("const N : string = \"a\";"),
            "const N: string = \"a\";\n"
        );
        assert_eq!(
            fmt("fn soma(a:int,b=1):int{return a+b;}"),
            "fn soma(a: int, b = 1): int {\n  return a + b;\n}\n"
        );
        assert_eq!(
            fmt("let d=fn(v:int):int=>v*2;"),
            "let d = fn (v: int): int {\n  return v * 2;\n};\n"
        );
        check_idempotent("let x: int = 1;\nfn f(a: string): bool {\n  return true;\n}\n");
    }

    #[test]
    fn reports_syntax_errors() {
        assert!(format_source("echo \"x\"").is_err());
    }

    #[test]
    fn preserves_line_comments() {
        // Etapa 4: comentarios de linha propria, trailing e de fim de
        // arquivo sobrevivem ao fmt, na ordem relativa.
        let src = "// cabecalho\nlet x = 1; // fim da linha\necho x;\n// eof\n";
        let out = fmt(src);
        assert_eq!(
            out,
            "// cabecalho\nlet x = 1; // fim da linha\necho x;\n// eof\n"
        );
        check_idempotent(src);
    }

    #[test]
    fn preserves_comments_in_blocks() {
        let src = "if true {\n  // dentro\n  echo 1;\n} // fim\n";
        let out = fmt(src);
        assert_eq!(out, "if true {\n  // dentro\n  echo 1;\n} // fim\n");
        check_idempotent(src);
    }

    #[test]
    fn preserves_single_blank_line() {
        let src = "echo 1;\n\n\necho 2;\n";
        assert_eq!(fmt(src), "echo 1;\n\necho 2;\n");
        check_idempotent(src);
    }

    #[test]
    fn comments_do_not_change_semantics() {
        // Mesmo AST com e sem comentarios (comparacao estrutural).
        let with = rhp_parser::parse_source("// c\necho 1; // t\n").expect("parse deve passar");
        let without = rhp_parser::parse_source("echo 1;\n").expect("parse deve passar");
        let ws: Vec<_> = with.statements.iter().map(|n| &n.stmt).collect();
        let wo: Vec<_> = without.statements.iter().map(|n| &n.stmt).collect();
        assert_eq!(ws, wo);
    }
}
