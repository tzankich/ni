mod c_tests;
mod coroutine_tests;
mod rust_tests;

fn parse_source(source: &str) -> ni_parser::Program {
    let tokens = ni_lexer::lex(source).expect("lex failed");
    ni_parser::parse(tokens).expect("parse failed")
}
