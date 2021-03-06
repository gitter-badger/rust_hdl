// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this file,
// You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2018, Olof Kraigher olof.kraigher@gmail.com

use ast::{
    Attribute, AttributeDeclaration, AttributeSpecification, Designator, EntityClass, EntityName,
    EntityTag,
};
use expression::parse_expression;
use message::ParseResult;
use names::parse_selected_name;
use subprogram::parse_signature;
use tokenizer::Kind::*;
use tokenstream::TokenStream;

fn parse_entity_class(stream: &mut TokenStream) -> ParseResult<EntityClass> {
    let token = stream.expect()?;
    Ok(try_token_kind!(
        token,
        Entity => EntityClass::Entity,
        Architecture => EntityClass::Architecture,
        Configuration => EntityClass::Configuration,
        Package => EntityClass::Package,
        Signal => EntityClass::Signal,
        Variable => EntityClass::Variable,
        Procedure => EntityClass::Procedure,
        Function => EntityClass::Function
    ))
}

pub fn parse_entity_name_list(stream: &mut TokenStream) -> ParseResult<Vec<EntityName>> {
    let token = stream.peek_expect()?;
    Ok(try_token_kind!(
        token,
        Identifier => {
            let mut entity_name_list = Vec::new();
            loop {
                let designator = stream.expect_ident()?.map_into(Designator::Identifier);
                let signature = {
                    if stream.peek_kind()? == Some(LeftSquare) {
                        Some(parse_signature(stream)?)
                    } else {
                        None
                    }
                };

                entity_name_list.push(EntityName::Name(EntityTag {
                    designator,
                    signature,
                }));

                let sep_token = stream.peek_expect()?;

                try_token_kind!(
                    sep_token,

                    Comma => {
                        stream.move_after(&sep_token);
                    },
                    Colon => {
                        break entity_name_list;
                    }
                )
            }
        },
        Others => {
            stream.move_after(&token);
            vec![EntityName::Others]
        },
        All => {
            stream.move_after(&token);
            vec![EntityName::All]
        }
    ))
}

pub fn parse_attribute(stream: &mut TokenStream) -> ParseResult<Vec<Attribute>> {
    stream.expect_kind(Attribute)?;
    let ident = stream.expect_ident()?;
    let token = stream.expect()?;

    Ok(try_token_kind!(
        token,
        Colon => {
            let type_mark = parse_selected_name(stream)?;
            stream.expect_kind(SemiColon)?;
            vec![Attribute::Declaration(AttributeDeclaration {
                ident,
                type_mark,
            })]
        },
        Of => {
            let entity_names = parse_entity_name_list(stream)?;
            stream.expect_kind(Colon)?;
            let entity_class = parse_entity_class(stream)?;
            stream.expect_kind(Is)?;
            let expr = parse_expression(stream)?;
            stream.expect_kind(SemiColon)?;

            let attributes = entity_names
                .into_iter()
                .map(|entity_name| {
                    Attribute::Specification(AttributeSpecification {
                        ident: ident.clone(),
                        entity_name: entity_name.clone(),
                        entity_class: entity_class,
                        expr: expr.clone(),
                    })
                }).collect();

            attributes
        }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::Designator;
    use test_util::with_stream;

    #[test]
    fn parse_simple_attribute_declaration() {
        let (util, result) = with_stream(parse_attribute, "attribute foo : lib.name;");
        assert_eq!(
            result,
            vec![Attribute::Declaration(AttributeDeclaration {
                ident: util.ident("foo"),
                type_mark: util.selected_name("lib.name")
            })]
        )
    }

    #[test]
    fn parse_simple_attribute_specification() {
        let (util, result) = with_stream(
            parse_attribute,
            "attribute attr_name of foo : signal is 0+1;",
        );
        assert_eq!(
            result,
            vec![Attribute::Specification(AttributeSpecification {
                ident: util.ident("attr_name"),
                entity_name: EntityName::Name(EntityTag {
                    designator: util.ident("foo").map_into(Designator::Identifier),
                    signature: None
                }),
                entity_class: EntityClass::Signal,
                expr: util.expr("0+1")
            })]
        )
    }

    #[test]
    fn parse_attribute_specification_list() {
        let (util, result) = with_stream(
            parse_attribute,
            "attribute attr_name of foo, bar : signal is 0+1;",
        );
        assert_eq!(
            result,
            vec![
                Attribute::Specification(AttributeSpecification {
                    ident: util.ident("attr_name"),
                    entity_name: EntityName::Name(EntityTag {
                        designator: util.ident("foo").map_into(Designator::Identifier),
                        signature: None
                    }),
                    entity_class: EntityClass::Signal,
                    expr: util.expr("0+1")
                }),
                Attribute::Specification(AttributeSpecification {
                    ident: util.ident("attr_name"),
                    entity_name: EntityName::Name(EntityTag {
                        designator: util.ident("bar").map_into(Designator::Identifier),
                        signature: None
                    }),
                    entity_class: EntityClass::Signal,
                    expr: util.expr("0+1")
                })
            ]
        )
    }

    #[test]
    fn parse_attribute_specification_all() {
        let (util, result) = with_stream(
            parse_attribute,
            "attribute attr_name of all : signal is 0+1;",
        );
        assert_eq!(
            result,
            vec![Attribute::Specification(AttributeSpecification {
                ident: util.ident("attr_name"),
                entity_name: EntityName::All,
                entity_class: EntityClass::Signal,
                expr: util.expr("0+1")
            })]
        )
    }

    #[test]
    fn parse_attribute_specification_others() {
        let (util, result) = with_stream(
            parse_attribute,
            "attribute attr_name of others : signal is 0+1;",
        );
        assert_eq!(
            result,
            vec![Attribute::Specification(AttributeSpecification {
                ident: util.ident("attr_name"),
                entity_name: EntityName::Others,
                entity_class: EntityClass::Signal,
                expr: util.expr("0+1")
            })]
        )
    }

    #[test]
    fn parse_attribute_specification_with_signature() {
        let (util, result) = with_stream(
            parse_attribute,
            "attribute attr_name of foo[return natural] : signal is 0+1;",
        );
        assert_eq!(
            result,
            vec![Attribute::Specification(AttributeSpecification {
                ident: util.ident("attr_name"),
                entity_name: EntityName::Name(EntityTag {
                    designator: util.ident("foo").map_into(Designator::Identifier),
                    signature: Some(util.signature("[return natural]"))
                }),
                entity_class: EntityClass::Signal,
                expr: util.expr("0+1")
            })]
        )
    }

}
