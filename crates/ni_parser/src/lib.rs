pub mod ast;
mod expr;
mod parser;

pub use ast::*;
pub use parser::Parser;

use ni_error::NiResult;
use ni_lexer::Token;

pub fn parse(tokens: Vec<Token>) -> NiResult<Program> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}
