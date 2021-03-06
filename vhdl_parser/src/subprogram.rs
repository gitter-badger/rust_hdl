// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2018, Olof Kraigher olof.kraigher@gmail.com

use ast::{
    Declaration, Designator, FunctionSpecification, ProcedureSpecification, Signature,
    SubprogramBody, SubprogramDeclaration,
};
use declarative_part::parse_declarative_part;
use interface_declaration::parse_parameter_interface_list;
use message::{error, MessageHandler, ParseResult};
use names::parse_selected_name;
use sequential_statement::parse_labeled_sequential_statements;
use source::WithPos;
use tokenizer::Kind::*;
use tokenstream::TokenStream;

pub fn parse_signature(stream: &mut TokenStream) -> ParseResult<Signature> {
    stream.expect_kind(LeftSquare)?;
    let mut type_marks = Vec::new();
    let mut return_mark = None;

    loop {
        let token = stream.peek_expect()?;

        try_token_kind!(
            token,

            Identifier => {
                type_marks.push(parse_selected_name(stream)?);
                let sep_token = stream.expect()?;

                try_token_kind!(
                    sep_token,
                    Comma => {},
                    RightSquare => {
                        break;
                    },
                    Return => {
                        if return_mark.is_some() {
                            return Err(error(sep_token, "Duplicate return in signature"));
                        }
                        return_mark = Some(parse_selected_name(stream)?);
                    }
                )
            },
            Return => {
                if return_mark.is_some() {
                    return Err(error(token, "Duplicate return in signature"));
                }
                stream.move_after(&token);
                return_mark = Some(parse_selected_name(stream)?);
            },
            RightSquare => {
                stream.move_after(&token);
                break;
            }
        )
    }

    Ok(match return_mark {
        Some(return_mark) => Signature::Function(type_marks, return_mark),
        None => Signature::Procedure(type_marks),
    })
}

fn parse_designator(stream: &mut TokenStream) -> ParseResult<WithPos<Designator>> {
    let token = stream.expect()?;
    Ok(try_token_kind!(
        token,
        Identifier => token.expect_ident()?.map_into(Designator::Identifier),
        StringLiteral => WithPos {
            item: Designator::OperatorSymbol(token.expect_string()?),
            pos: token.pos,
        }
    ))
}

pub fn parse_subprogram_declaration_no_semi(
    stream: &mut TokenStream,
    messages: &mut MessageHandler,
) -> ParseResult<SubprogramDeclaration> {
    let token = stream.expect()?;

    let (is_function, is_pure) = {
        try_token_kind!(
            token,
            Procedure => (false, false),
            Function => (true, true),
            Impure => {
                stream.expect_kind(Function)?;
                (true, false)
            }
        )
    };

    let designator = parse_designator(stream)?;

    let parameter_list = {
        if stream.peek_kind()? == Some(LeftPar) {
            parse_parameter_interface_list(stream, messages)?
        } else {
            Vec::new()
        }
    };

    if is_function {
        stream.expect_kind(Return)?;
        let return_type = parse_selected_name(stream)?;
        Ok(SubprogramDeclaration::Function(FunctionSpecification {
            pure: is_pure,
            designator: designator,
            parameter_list: parameter_list,
            return_type: return_type,
        }))
    } else {
        Ok(SubprogramDeclaration::Procedure(ProcedureSpecification {
            designator: designator,
            parameter_list: parameter_list,
        }))
    }
}

pub fn parse_subprogram_declaration(
    stream: &mut TokenStream,
    messages: &mut MessageHandler,
) -> ParseResult<SubprogramDeclaration> {
    let res = parse_subprogram_declaration_no_semi(stream, messages);
    stream.expect_kind(SemiColon)?;
    res
}

/// LRM 4.3 Subprogram bodies
pub fn parse_subprogram_body(
    stream: &mut TokenStream,
    specification: SubprogramDeclaration,
    messages: &mut MessageHandler,
) -> ParseResult<SubprogramBody> {
    let end_kind = {
        match specification {
            SubprogramDeclaration::Procedure(..) => Procedure,
            SubprogramDeclaration::Function(..) => Function,
        }
    };
    let declarations = parse_declarative_part(stream, messages, true)?;

    let (statements, end_token) = parse_labeled_sequential_statements(stream, messages)?;
    try_token_kind!(
        end_token,
        End => {
            stream.pop_if_kind(end_kind)?;
            stream.pop_if_kind(Identifier)?;
            stream.pop_if_kind(StringLiteral)?;
            stream.expect_kind(SemiColon)?;
        }
    );
    Ok(SubprogramBody {
        specification,
        declarations,
        statements,
    })
}

pub fn parse_subprogram(
    stream: &mut TokenStream,
    messages: &mut MessageHandler,
) -> ParseResult<Declaration> {
    let specification = parse_subprogram_declaration_no_semi(stream, messages)?;
    match_token_kind!(
        stream.expect()?,
        Is => {
            Ok(Declaration::SubprogramBody(parse_subprogram_body(stream, specification, messages)?))
        },
        SemiColon => {
            Ok(Declaration::SubprogramDeclaration(specification))
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use latin_1::Latin1String;
    use test_util::{with_partial_stream, with_stream, with_stream_no_messages};

    #[test]
    pub fn parses_procedure_specification() {
        let (util, result) = with_stream_no_messages(
            parse_subprogram_declaration,
            "\
procedure foo;
",
        );
        assert_eq!(
            result,
            SubprogramDeclaration::Procedure(ProcedureSpecification {
                designator: util.ident("foo").map_into(Designator::Identifier),
                parameter_list: Vec::new(),
            })
        );
    }

    #[test]
    pub fn parses_function_specification() {
        let (util, result) = with_stream_no_messages(
            parse_subprogram_declaration,
            "\
function foo return lib.foo.natural;
",
        );
        assert_eq!(
            result,
            SubprogramDeclaration::Function(FunctionSpecification {
                pure: true,
                designator: util.ident("foo").map_into(Designator::Identifier),
                parameter_list: Vec::new(),
                return_type: util.selected_name("lib.foo.natural")
            })
        );
    }

    #[test]
    pub fn parses_function_specification_operator() {
        let (util, result) = with_stream_no_messages(
            parse_subprogram_declaration,
            "\
function \"+\" return lib.foo.natural;
",
        );
        assert_eq!(
            result,
            SubprogramDeclaration::Function(FunctionSpecification {
                pure: true,
                designator: WithPos {
                    item: Designator::OperatorSymbol(Latin1String::from_utf8_unchecked("+")),
                    pos: util.first_substr_pos("\"+\"")
                },
                parameter_list: Vec::new(),
                return_type: util.selected_name("lib.foo.natural")
            })
        );
    }

    #[test]
    pub fn parses_impure_function_specification() {
        let (util, result) = with_stream_no_messages(
            parse_subprogram_declaration,
            "\
impure function foo return lib.foo.natural;
",
        );
        assert_eq!(
            result,
            SubprogramDeclaration::Function(FunctionSpecification {
                pure: false,
                designator: util.ident("foo").map_into(Designator::Identifier),
                parameter_list: Vec::new(),
                return_type: util.selected_name("lib.foo.natural")
            })
        );
    }

    #[test]
    pub fn parses_procedure_specification_with_parameters() {
        let (util, result) = with_stream_no_messages(
            parse_subprogram_declaration,
            "\
procedure foo(foo : natural);
",
        );
        assert_eq!(
            result,
            SubprogramDeclaration::Procedure(ProcedureSpecification {
                designator: util.ident("foo").map_into(Designator::Identifier),
                parameter_list: vec![util.parameter("foo : natural")],
            })
        );
    }

    #[test]
    pub fn parses_function_specification_with_parameters() {
        let (util, result) = with_stream_no_messages(
            parse_subprogram_declaration,
            "\
function foo(foo : natural) return lib.foo.natural;
",
        );
        assert_eq!(
            result,
            SubprogramDeclaration::Function(FunctionSpecification {
                pure: true,
                designator: util.ident("foo").map_into(Designator::Identifier),
                parameter_list: vec![util.parameter("foo : natural")],
                return_type: util.selected_name("lib.foo.natural")
            })
        );
    }

    #[test]
    pub fn parses_function_signature_only_return() {
        let (util, result) = with_stream(parse_signature, "[return bar.type_mark]");
        assert_eq!(
            result,
            Signature::Function(vec![], util.selected_name("bar.type_mark"))
        );
    }

    #[test]
    pub fn parses_function_signature_one_argument() {
        let (util, result) = with_stream(parse_signature, "[foo.type_mark return bar.type_mark]");
        assert_eq!(
            result,
            Signature::Function(
                vec![util.selected_name("foo.type_mark")],
                util.selected_name("bar.type_mark")
            )
        );
    }

    #[test]
    pub fn parses_procedure_signature() {
        let (util, result) = with_stream(parse_signature, "[foo.type_mark]");
        assert_eq!(
            result,
            Signature::Procedure(vec![util.selected_name("foo.type_mark")])
        );
    }

    #[test]
    pub fn parses_function_signature_many_arguments() {
        let (util, result) = with_stream(
            parse_signature,
            "[foo.type_mark, foo2.type_mark return bar.type_mark]",
        );
        assert_eq!(
            result,
            Signature::Function(
                vec![
                    util.selected_name("foo.type_mark"),
                    util.selected_name("foo2.type_mark")
                ],
                util.selected_name("bar.type_mark")
            )
        );
    }

    #[test]
    pub fn parses_function_signature_many_return_error() {
        let (util, result) =
            with_partial_stream(parse_signature, "[return bar.type_mark return bar2]");
        assert_eq!(
            result,
            Err(error(
                &util.substr_pos("return", 2),
                "Duplicate return in signature"
            ))
        );

        let (util, result) =
            with_partial_stream(parse_signature, "[foo return bar.type_mark return bar2]");
        assert_eq!(
            result,
            Err(error(
                &util.substr_pos("return", 2),
                "Duplicate return in signature"
            ))
        );
    }

    #[test]
    pub fn parses_subprogram_body() {
        let (util, decl) = with_stream_no_messages(
            parse_subprogram,
            "\
function foo(arg : natural) return natural is
  constant foo : natural := 0;
begin
  return foo + arg;
end function;
",
        );
        let specification = util.subprogram_decl("function foo(arg : natural) return natural");
        let declarations = util.declarative_part("constant foo : natural := 0;");
        let statements = vec![util.sequential_statement("return foo + arg;")];
        let body = SubprogramBody {
            specification,
            declarations,
            statements,
        };
        assert_eq!(decl, Declaration::SubprogramBody(body));
    }

    #[test]
    pub fn parses_subprogram_declaration() {
        let (util, decl) = with_stream_no_messages(
            parse_subprogram,
            "\
function foo(arg : natural) return natural;
",
        );
        let specification = util.subprogram_decl("function foo(arg : natural) return natural");
        assert_eq!(decl, Declaration::SubprogramDeclaration(specification));
    }

}
