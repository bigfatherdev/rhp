//! Benchmarks std-only do RHP (sem crates externas).
//!
//! Metodologia em `docs/BENCHMARKS.md`. Rode em release:
//! `cargo run --release -p rhp-cli --example bench_rhp`

use std::time::Instant;

fn bench(name: &str, iters: u32, mut f: impl FnMut()) {
    f(); // aquecimento
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    println!(
        "{name:28} {iters:>4}x  {total_ms:>12.3} ms total  {:>12.3} ms/op",
        total_ms / f64::from(iters),
    );
}

fn compile(src: &str) -> rhp_bytecode::Chunk {
    let program = rhp_parser::parse_source(src).expect("fonte de bench deve parsear");
    rhp_bytecode::compile(&program).expect("fonte de bench deve compilar")
}

fn main() {
    println!("rhp bench — tempos medios por operacao (rode em release)");
    println!();

    // lex/parse/compile de fonte gerada (~85 KB, 5000 instrucoes).
    let big = "echo 1 + 2 * 3;\n".repeat(5000);
    bench("lex 85KB", 20, || {
        rhp_lexer::lex(&big).expect("lex deve passar");
    });
    let tokens = rhp_lexer::lex(&big).expect("lex deve passar");
    bench("parse 5000 stmts", 20, || {
        rhp_parser::parse(&tokens).expect("parse deve passar");
    });
    bench("compile 5000 stmts", 20, || {
        let program = rhp_parser::parse(&tokens).expect("parse deve passar");
        rhp_bytecode::compile(&program).expect("compile deve passar");
    });

    // Execucao: fib(20) recursivo e laco de 200k iteracoes (compilados 1x).
    let fib = compile(
        "fn fib(n) { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); } echo fib(20);",
    );
    bench("run fib(20)", 5, || {
        rhp_vm::Vm::new()
            .run_to_string(&fib)
            .expect("fib deve rodar");
    });
    let loop200k = compile("let i = 0; while i < 200000 { i = i + 1; }");
    bench("run while 200k", 3, || {
        rhp_vm::Vm::new()
            .run_to_string(&loop200k)
            .expect("laco deve rodar");
    });
    let arrays = compile(
        "let a = []; let i = 0; while i < 20000 { a = push(a, i); i = i + 1; } echo len(a);",
    );
    bench("run push 20k", 3, || {
        rhp_vm::Vm::new()
            .run_to_string(&arrays)
            .expect("arrays devem rodar");
    });
}
