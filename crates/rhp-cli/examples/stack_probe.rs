// Sonda o pipeline completo (parse+compile+run) numa thread de 2 MiB.
// Uso: stack_probe <arquivo.rhp>
fn main() {
    let path: String = std::env::args().nth(1).unwrap();
    let src = std::fs::read_to_string(&path).unwrap();
    let child = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || {
            let tokens = rhp_lexer::lex(&src).expect("lex");
            let program = rhp_parser::parse(&tokens).expect("parse");
            let chunk = rhp_bytecode::compile(&program).expect("compile");
            chunk.validate().expect("validate");
            let mut vm = rhp_vm::Vm::new();
            let mut sink = std::io::sink();
            match vm.run(&chunk, &mut sink) {
                Ok(()) => println!("RUN_OK"),
                Err(e) => println!("RUN_ERR {}", e.message),
            }
        })
        .unwrap();
    match child.join() {
        Ok(()) => println!("THREAD_OK"),
        Err(_) => println!("THREAD_PANIC_OR_ABORT"),
    }
}
