# Formal Grammar (EBNF)

```ebnf
(* Ni Language Grammar -- EBNF *)

program        = { declaration } EOF ;

(* === Declarations === *)
declaration    = class_decl
               | fun_decl
               | var_decl
               | const_decl
               | enum_decl
               | import_decl
               | spec_decl
               | statement ;

class_decl     = "class" IDENTIFIER [ "extends" IDENTIFIER ] ":"
                 INDENT { class_member } DEDENT ;

class_member   = var_decl
               | fun_decl
               | property_decl
               | static_decl ;

property_decl  = "get" IDENTIFIER [ "->" type ] ":" block
               | "set" IDENTIFIER "(" parameter ")" ":" block ;

static_decl    = "static" ( var_decl | fun_decl ) ;

fun_decl       = "fun" IDENTIFIER "(" [ param_list ] ")" [ "->" type ] ":"
                 block ;

param_list     = parameter { "," parameter } ;
parameter      = IDENTIFIER [ ":" type ] [ "=" expression ] ;

var_decl       = "var" IDENTIFIER [ ":" type ] "=" expression NEWLINE ;
const_decl     = "const" IDENTIFIER "=" expression NEWLINE ;

enum_decl      = "enum" IDENTIFIER ":"
                 INDENT { IDENTIFIER [ "=" expression ] NEWLINE } DEDENT ;

import_decl    = "import" module_path [ "as" IDENTIFIER ]
               | "from" module_path "import" import_names ;

module_path    = IDENTIFIER { "." IDENTIFIER } ;
import_names   = IDENTIFIER { "," IDENTIFIER } | "*" ;

spec_decl      = "spec" STRING ":" ( block | spec_body ) ;

spec_body      = INDENT [ each_clause ] { spec_section } DEDENT ;

each_clause    = "each" expression_list ":"
               | "each" "(" expression_list ")" ":" ;

spec_section   = ( "given" | "when" | "then" ) STRING ":"
                 INDENT { declaration | spec_section } DEDENT ;

(* === Statements === *)
statement      = expr_stmt
               | if_stmt
               | while_stmt
               | for_stmt
               | match_stmt
               | return_stmt
               | break_stmt
               | continue_stmt
               | pass_stmt
               | try_stmt
               | fail_stmt
               | assert_stmt ;

expr_stmt      = expression NEWLINE ;

if_stmt        = "if" expression ":" block
                 { "elif" expression ":" block }
                 [ "else" ":" block ] ;

while_stmt     = "while" expression ":" block ;

for_stmt       = "for" target_list "in" expression ":" block ;

target_list    = IDENTIFIER { "," IDENTIFIER } ;

match_stmt     = "match" expression ":"
                 INDENT { when_clause } DEDENT ;

when_clause    = "when" pattern { "," pattern } [ "if" expression ] ":" block ;

pattern        = "_"
               | literal
               | IDENTIFIER
               | IDENTIFIER "is" IDENTIFIER ;

return_stmt    = "return" [ expression ] NEWLINE ;
break_stmt     = "break" NEWLINE ;
continue_stmt  = "continue" NEWLINE ;
pass_stmt      = "pass" NEWLINE ;
fail_stmt      = "fail" expression NEWLINE ;
assert_stmt    = "assert" expression [ "," expression ] NEWLINE ;

try_stmt       = "try" ":" block
                 "catch" [ IDENTIFIER ] ":" ( catch_match | block ) ;

catch_match    = INDENT { case_clause } DEDENT ;

(* === Expressions === *)
expression     = assignment ;

assignment     = ( call "." IDENTIFIER | call "[" expression "]" | IDENTIFIER )
                 ( "=" | "+=" | "-=" | "*=" | "/=" | "%=" ) assignment
               | ternary ;

ternary        = or_expr [ "if" or_expr "else" ternary ] ;

or_expr        = and_expr { "or" and_expr } ;
and_expr       = equality { "and" equality } ;
equality       = comparison { ( "==" | "!=" | "is" ) comparison } ;
comparison     = range { ( "<" | ">" | "<=" | ">=" ) range } ;
range          = addition [ ( ".." | "..=" ) addition ] ;
addition       = multiplication { ( "+" | "-" ) multiplication } ;
multiplication = unary { ( "*" | "/" | "%" ) unary } ;
unary          = ( "-" | "not" | "try" | "fail" | "spawn" | "await" ) unary
               | "yield" [ expression ]
               | "wait" expression
               | call ;

call           = primary { "(" [ arg_list ] ")"
                         | "[" expression "]"
                         | "." IDENTIFIER
                         | "?." IDENTIFIER
                         | "?" } ;

arg_list       = argument { "," argument } ;
argument       = [ IDENTIFIER "=" ] expression ;

primary        = NUMBER | STRING | "true" | "false" | "none"
               | IDENTIFIER
               | "(" expression ")"
               | "[" [ expression { "," expression } ] "]"
               | "[" expression ":" expression { "," expression ":" expression } "]"
               | "fun" "(" [ param_list ] ")" [ ":" expression | ":" NEWLINE block ]
               | "self" | "super" ;

(* === Types === *)
type           = IDENTIFIER [ "?" ]
               | "list" "[" type "]"
               | "map" "[" type "," type "]"
               | "fun" "(" [ type_list ] ")" [ "->" type ] ;

type_list      = type { "," type } ;

(* === Lexical === *)
block          = INDENT { declaration } DEDENT ;

IDENTIFIER     = LETTER { LETTER | DIGIT | "_" } ;
NUMBER         = DIGIT { DIGIT | "_" } [ "." DIGIT { DIGIT | "_" } ]
               | "0x" HEX_DIGIT { HEX_DIGIT | "_" }
               | "0b" BIN_DIGIT { BIN_DIGIT | "_" } ;
STRING         = '"' { CHAR | ESCAPE } '"'
               | "'" { CHAR | ESCAPE } "'"
               | '"""' { ANY } '"""'
               | "'''" { ANY } "'''"
               | '`' { CHAR | ESCAPE | "{" expression "}" } '`'
               | '```' { ANY | "{" expression "}" } '```' ;

LETTER         = "a".."z" | "A".."Z" | "_" ;
DIGIT          = "0".."9" ;
HEX_DIGIT      = DIGIT | "a".."f" | "A".."F" ;
BIN_DIGIT      = "0" | "1" ;
ESCAPE         = "\\" ( "n" | "t" | "r" | "\\" | '"' | "'" | "`" | "{" | "}" | "0" ) ;

INDENT         = (* increase in indentation level *) ;
DEDENT         = (* decrease in indentation level *) ;
NEWLINE        = (* logical line terminator -- suppressed inside brackets
                    and after trailing binary operators, commas, dots,
                    assignments, and keyword operators *) ;
EOF            = (* end of file *) ;
```
