use crate::intermediate::{constraints::*, *};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until},
    character::complete::char,
    combinator::{into, map, opt, value},
    error::Error,
    multi::{many0_count, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};

use super::{
    asn1_type, asn1_value,
    common::{
        extension_marker, identifier, in_braces, in_parentheses, range_seperator,
        skip_ws_and_comments,
    },
    information_object_class::object_set,
    parameterization::parameters,
    util::{opt_delimited, take_until_and_not, take_until_unbalanced},
};

pub fn constraint<'a>(input: &'a str) -> IResult<&'a str, Vec<Constraint>> {
    many1(alt((
        single_constraint,
        // Handle SIZE constraint without external parentheses
        map(size_constraint, |c| {
            Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(c),
                extensible: false,
            })
        }),
        map(parameters, |p| Constraint::Parameter(p)),
    )))(input)
}

pub fn single_constraint<'a>(input: &'a str) -> IResult<&'a str, Constraint> {
    skip_ws_and_comments(in_parentheses(alt((
        map(table_constraint, |t| Constraint::TableConstraint(t)),
        map(element_set, |set| Constraint::SubtypeConstraint(set)),
    ))))(input)
}

pub fn set_operator<'a>(input: &'a str) -> IResult<&'a str, SetOperator> {
    skip_ws_and_comments(alt((
        value(SetOperator::Intersection, tag(INTERSECTION)),
        value(SetOperator::Intersection, tag(CARET)),
        value(SetOperator::Union, tag(UNION)),
        value(SetOperator::Union, tag(PIPE)),
        value(SetOperator::Except, tag(EXCEPT)),
    )))(input)
}

fn element_set<'a>(input: &'a str) -> IResult<&'a str, ElementSet> {
    into(pair(
        alt((
            map(set_operation, |v| ElementOrSetOperation::SetOperation(v)),
            map(subtype_element, |v| ElementOrSetOperation::Element(v)),
        )),
        opt(skip_ws_and_comments(preceded(
            char(COMMA),
            extension_marker,
        ))),
    ))(input)
}

fn set_operation<'a>(input: &'a str) -> IResult<&'a str, SetOperation> {
    into(tuple((
        subtype_element,
        set_operator,
        alt((
            map(set_operation, |v| ElementOrSetOperation::SetOperation(v)),
            map(subtype_element, |v| ElementOrSetOperation::Element(v)),
        )),
    )))(input)
}

fn subtype_element<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    alt((
        single_type_constraint,
        multiple_type_constraints,
        size_constraint,
        pattern_constraint,
        user_defined_constraint,
        permitted_alphabet_constraint,
        value_range,
        single_value,
        contained_subtype,
    ))(input)
}

fn extension_additions<'a>(input: &'a str) -> IResult<&'a str, ()> {
    value(
        (),
        opt(pair(
            skip_ws_and_comments(char(COMMA)),
            skip_ws_and_comments(separated_list0(
                skip_ws_and_comments(char(COMMA)),
                skip_ws_and_comments(alt((
                    value(0, asn1_value),
                    value(
                        0,
                        pair(
                            terminated(
                                alt((value(None, tag(MIN)), map(asn1_value, |v| Some(v)))),
                                skip_ws_and_comments(opt(char(GREATER_THAN))),
                            ),
                            preceded(
                                range_seperator,
                                preceded(
                                    opt(char(LESS_THAN)),
                                    skip_ws_and_comments(alt((
                                        value(None, tag(MAX)),
                                        map(asn1_value, |v| Some(v)),
                                    ))),
                                ),
                            ),
                        ),
                    ),
                ))),
            )),
        )),
    )(input)
}

fn single_value<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    opt_delimited::<char, SubtypeElement, char, Error<&str>, _, _, _>(
        skip_ws_and_comments(char(LEFT_PARENTHESIS)),
        skip_ws_and_comments(into(pair(
            asn1_value,
            opt(skip_ws_and_comments(delimited(
                char(COMMA),
                extension_marker,
                extension_additions,
            ))),
        ))),
        skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
    )(input)
}

fn contained_subtype<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    opt_delimited::<char, SubtypeElement, char, Error<&str>, _, _, _>(
        skip_ws_and_comments(char(LEFT_PARENTHESIS)),
        skip_ws_and_comments(map(
            pair(
                preceded(opt(tag(INCLUDES)), skip_ws_and_comments(asn1_type)),
                opt(skip_ws_and_comments(preceded(
                    char(COMMA),
                    extension_marker,
                ))),
            ),
            |(t, ext)| SubtypeElement::ContainedSubtype {
                subtype: t,
                extensible: ext.is_some(),
            },
        )),
        skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
    )(input)
}

fn value_range<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    opt_delimited::<char, SubtypeElement, char, Error<&str>, _, _, _>(
        skip_ws_and_comments(char(LEFT_PARENTHESIS)),
        skip_ws_and_comments(map(
            tuple((
                terminated(
                    alt((value(None, tag(MIN)), map(asn1_value, |v| Some(v)))),
                    skip_ws_and_comments(opt(char(GREATER_THAN))),
                ),
                preceded(
                    range_seperator,
                    preceded(
                        opt(char(LESS_THAN)),
                        skip_ws_and_comments(alt((
                            value(None, tag(MAX)),
                            map(asn1_value, |v| Some(v)),
                        ))),
                    ),
                ),
                opt(skip_ws_and_comments(delimited(
                    char(COMMA),
                    extension_marker,
                    extension_additions,
                ))),
            )),
            |(min, max, ext)| SubtypeElement::ValueRange {
                min,
                max,
                extensible: ext.is_some(),
            },
        )),
        skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
    )(input)
}

fn size_constraint<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    opt_delimited::<char, SubtypeElement, char, Error<&str>, _, _, _>(
        skip_ws_and_comments(char(LEFT_PARENTHESIS)),
        skip_ws_and_comments(into(preceded(tag(SIZE), single_constraint))),
        skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
    )(input)
}

fn pattern_constraint<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    map(
        opt_delimited::<char, PatternConstraint, char, Error<&str>, _, _, _>(
            skip_ws_and_comments(char(LEFT_PARENTHESIS)),
            skip_ws_and_comments(into(preceded(
                tag(PATTERN),
                skip_ws_and_comments(delimited(
                    char('"'),
                    take_until_and_not("\"", "\"\""),
                    char('"'),
                )),
            ))),
            skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
        ),
        |p| SubtypeElement::PatternConstraint(p),
    )(input)
}

fn user_defined_constraint<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    map(
        opt_delimited::<char, UserDefinedConstraint, char, Error<&str>, _, _, _>(
            skip_ws_and_comments(char(LEFT_PARENTHESIS)),
            skip_ws_and_comments(into(preceded(
                tag(CONSTRAINED_BY),
                skip_ws_and_comments(delimited(
                    char(LEFT_BRACE),
                    take_until_unbalanced("{", "}"),
                    char(RIGHT_BRACE),
                )),
            ))),
            skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
        ),
        |c| SubtypeElement::UserDefinedConstraint(c),
    )(input)
}

/// Parses a PermittedAlphabet constraint.
/// ### Reference in X680
/// >* _51.7.1 The "PermittedAlphabet" notation shall be: `PermittedAlphabet ::= FROM Constraint`_
/// >* _51.7.2 A "PermittedAlphabet" specifies all values which can be constructed using a sub-alphabet of the parent string. This notation can only be applied to restricted character string types._
/// >* _51.7.3 The "Constraint" shall use the "SubtypeConstraint" alternative of "ConstraintSpec". Each "SubtypeElements" within that "SubtypeConstraint" shall be one of the four alternatives "SingleValue", "ContainedSubtype", "ValueRange", and "SizeConstraint". The sub-alphabet includes precisely those characters which appear in one or more of the values of the parent string type which are allowed by the "Constraint"._
/// >* _51.7.4 If "Constraint" is extensible, then the set of values selected by the permitted alphabet constraint is extensible. The set of values in the root are those permitted by the root of "Constraint", and the extension additions are those values permitted by the root together with the extension-additions of "Constraint", excluding those values already in the root._
fn permitted_alphabet_constraint<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    opt_delimited::<char, SubtypeElement, char, Error<&str>, _, _, _>(
        skip_ws_and_comments(char(LEFT_PARENTHESIS)),
        skip_ws_and_comments(map(
            preceded(
                tag(FROM),
                in_parentheses(alt((
                    map(set_operation, |v| ElementOrSetOperation::SetOperation(v)),
                    map(subtype_element, |v| ElementOrSetOperation::Element(v)),
                ))),
            ),
            |i| SubtypeElement::PermittedAlphabet(Box::new(i)),
        )),
        skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
    )(input)
}

fn single_type_constraint<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    opt_delimited::<char, SubtypeElement, char, Error<&str>, _, _, _>(
        skip_ws_and_comments(char(LEFT_PARENTHESIS)),
        skip_ws_and_comments(into(preceded(
            tag(WITH_COMPONENTS),
            in_braces(pair(
                opt(skip_ws_and_comments(terminated(
                    value(ExtensionMarker(), tag(ELLIPSIS)),
                    skip_ws_and_comments(char(COMMA)),
                ))),
                many1(terminated(
                    subset_member,
                    opt(skip_ws_and_comments(char(COMMA))),
                )),
            )),
        ))),
        skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
    )(input)
}

fn multiple_type_constraints<'a>(input: &'a str) -> IResult<&'a str, SubtypeElement> {
    opt_delimited::<char, SubtypeElement, char, Error<&str>, _, _, _>(
        skip_ws_and_comments(char(LEFT_PARENTHESIS)),
        skip_ws_and_comments(preceded(
            tag(WITH_COMPONENT),
            skip_ws_and_comments(alt((
                map(single_type_constraint, |s| {
                    if let SubtypeElement::SingleTypeConstraint(c) = s {
                        SubtypeElement::MultipleTypeConstraints(c)
                    } else {
                        s
                    }
                }),
                multiple_type_constraints,
            ))),
        )),
        skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
    )(input)
}

fn subset_member<'a>(
    input: &'a str,
) -> IResult<&'a str, (&str, Option<Vec<Constraint>>, Option<ComponentPresence>)> {
    skip_ws_and_comments(tuple((
        identifier,
        opt(skip_ws_and_comments(constraint)),
        opt(skip_ws_and_comments(alt((
            value(ComponentPresence::Present, tag(PRESENT)),
            value(ComponentPresence::Absent, tag(ABSENT)),
        )))),
    )))(input)
}

fn table_constraint<'a>(input: &'a str) -> IResult<&'a str, TableConstraint> {
    opt_delimited::<char, TableConstraint, char, Error<&str>, _, _, _>(
        skip_ws_and_comments(char(LEFT_PARENTHESIS)),
        skip_ws_and_comments(into(pair(
            object_set,
            opt(in_braces(separated_list1(
                skip_ws_and_comments(char(COMMA)),
                relational_constraint,
            ))),
        ))),
        skip_ws_and_comments(char(RIGHT_PARENTHESIS)),
    )(input)
}

fn relational_constraint<'a>(input: &'a str) -> IResult<&'a str, RelationalConstraint> {
    into(skip_ws_and_comments(preceded(
        char(AT),
        pair(many0_count(char(DOT)), identifier),
    )))(input)
}

#[cfg(test)]
mod tests {
    use crate::intermediate::{constraints::*, information_object::*, types::*, *};

    use crate::parser::constraint::*;

    #[test]
    fn parses_value_constraint() {
        assert_eq!(
            constraint("(5)").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::SingleValue {
                    value: ASN1Value::Integer(5),
                    extensible: false
                }),
                extensible: false
            })]
        );
        assert_eq!(
            constraint("(5..9)").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                    min: Some(ASN1Value::Integer(5)),
                    max: Some(ASN1Value::Integer(9)),
                    extensible: false
                }),
                extensible: false
            })]
        );
        assert_eq!(
            constraint("(-5..9)").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                    min: Some(ASN1Value::Integer(-5)),
                    max: Some(ASN1Value::Integer(9)),
                    extensible: false
                }),
                extensible: false
            })]
        );
        assert_eq!(
            constraint("(-9..-4,...)").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                    min: Some(ASN1Value::Integer(-9)),
                    max: Some(ASN1Value::Integer(-4)),
                    extensible: true
                }),
                extensible: false
            })]
        );
    }

    #[test]
    fn handles_added_extension_values() {
        assert_eq!(
            constraint("(1..32767,..., 8388607)").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                    min: Some(ASN1Value::Integer(1)),
                    max: Some(ASN1Value::Integer(32767)),
                    extensible: true
                }),
                extensible: false
            })]
        )
    }

    #[test]
    fn handles_redundant_parentheses() {
        assert_eq!(
            constraint("((5..9))").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                    min: Some(ASN1Value::Integer(5)),
                    max: Some(ASN1Value::Integer(9)),
                    extensible: false
                }),
                extensible: false
            })]
        );
    }

    #[test]
    fn parses_value_constraint_with_inserted_comment() {
        assert_eq!(
            constraint("(-9..-4, -- Very annoying! -- ...)").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                    min: Some(ASN1Value::Integer(-9)),
                    max: Some(ASN1Value::Integer(-4)),
                    extensible: true
                }),
                extensible: false
            })]
        );
        assert_eq!(
            constraint("(-9-- Very annoying! --..-4,  ...)").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                    min: Some(ASN1Value::Integer(-9)),
                    max: Some(ASN1Value::Integer(-4)),
                    extensible: true
                }),
                extensible: false
            })]
        );
    }

    #[test]
    fn parses_size_constraint() {
        assert_eq!(
            constraint("(SIZE(3..16, ...))").unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::SizeConstraint(Box::new(
                    ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                        min: Some(ASN1Value::Integer(3)),
                        max: Some(ASN1Value::Integer(16)),
                        extensible: true
                    })
                ))),
                extensible: false
            })]
        )
    }

    #[test]
    fn parses_composite_constraint() {
        assert_eq!(
            constraint(r#"(ALL EXCEPT 1)"#).unwrap().1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::SetOperation(SetOperation {
                    base: SubtypeElement::SingleValue {
                        value: ASN1Value::All,
                        extensible: false
                    },
                    operator: SetOperator::Except,
                    operant: Box::new(ElementOrSetOperation::Element(
                        SubtypeElement::SingleValue {
                            value: ASN1Value::Integer(1),
                            extensible: false
                        }
                    ))
                }),
                extensible: false
            })]
        )
    }

    #[test]
    fn parses_complex_set() {
        assert_eq!(
            constraint(
                r#"((WITH COMPONENT (WITH COMPONENTS {..., containerId (ALL EXCEPT 1)})) |
          (WITH COMPONENT (WITH COMPONENTS {..., containerId (ALL EXCEPT 2)})))"#
            )
            .unwrap()
            .1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::SetOperation(SetOperation {
                    base: SubtypeElement::MultipleTypeConstraints(InnerTypeConstraint {
                        is_partial: true,
                        constraints: vec![ConstrainedComponent {
                            identifier: "containerId".into(),
                            constraints: vec![Constraint::SubtypeConstraint(ElementSet {
                                set: ElementOrSetOperation::SetOperation(SetOperation {
                                    base: SubtypeElement::SingleValue {
                                        value: ASN1Value::All,
                                        extensible: false
                                    },
                                    operator: SetOperator::Except,
                                    operant: Box::new(ElementOrSetOperation::Element(
                                        SubtypeElement::SingleValue {
                                            value: ASN1Value::Integer(1),
                                            extensible: false
                                        }
                                    ))
                                }),
                                extensible: false
                            })],
                            presence: ComponentPresence::Unspecified
                        }]
                    }),
                    operator: SetOperator::Union,
                    operant: Box::new(ElementOrSetOperation::Element(
                        SubtypeElement::MultipleTypeConstraints(InnerTypeConstraint {
                            is_partial: true,
                            constraints: vec![ConstrainedComponent {
                                identifier: "containerId".into(),
                                constraints: vec![Constraint::SubtypeConstraint(ElementSet {
                                    set: ElementOrSetOperation::SetOperation(SetOperation {
                                        base: SubtypeElement::SingleValue {
                                            value: ASN1Value::All,
                                            extensible: false
                                        },
                                        operator: SetOperator::Except,
                                        operant: Box::new(ElementOrSetOperation::Element(
                                            SubtypeElement::SingleValue {
                                                value: ASN1Value::Integer(2),
                                                extensible: false
                                            }
                                        ))
                                    }),
                                    extensible: false
                                })],
                                presence: ComponentPresence::Unspecified
                            }]
                        })
                    ))
                }),
                extensible: false
            })]
        )
    }

    #[test]
    fn parses_full_component_constraint() {
        assert_eq!(
            constraint(
                "(WITH COMPONENTS
                  {ordering ABSENT ,
                  sales (0..5) PRESENT,
                  e-cash-return ABSENT } )"
            )
            .unwrap()
            .1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::SingleTypeConstraint(
                    InnerTypeConstraint {
                        is_partial: false,
                        constraints: vec![
                            ConstrainedComponent {
                                identifier: "ordering".into(),
                                constraints: vec![],
                                presence: ComponentPresence::Absent
                            },
                            ConstrainedComponent {
                                identifier: "sales".into(),
                                constraints: vec![Constraint::SubtypeConstraint(ElementSet {
                                    set: ElementOrSetOperation::Element(
                                        SubtypeElement::ValueRange {
                                            min: Some(ASN1Value::Integer(0)),
                                            max: Some(ASN1Value::Integer(5)),
                                            extensible: false
                                        }
                                    ),
                                    extensible: false
                                })],
                                presence: ComponentPresence::Present
                            },
                            ConstrainedComponent {
                                identifier: "e-cash-return".into(),
                                constraints: vec![],
                                presence: ComponentPresence::Absent
                            }
                        ]
                    }
                )),
                extensible: false
            })]
        );
    }

    #[test]
    fn parses_partial_component_constraint() {
        assert_eq!(
            constraint(
                "( WITH COMPONENTS
                      {... ,
                      ordering ABSENT,
                      sales (0..5) } )"
            )
            .unwrap()
            .1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::SingleTypeConstraint(
                    InnerTypeConstraint {
                        is_partial: true,
                        constraints: vec![
                            ConstrainedComponent {
                                identifier: "ordering".into(),
                                constraints: vec![],
                                presence: ComponentPresence::Absent
                            },
                            ConstrainedComponent {
                                identifier: "sales".into(),
                                constraints: vec![Constraint::SubtypeConstraint(ElementSet {
                                    set: ElementOrSetOperation::Element(
                                        SubtypeElement::ValueRange {
                                            min: Some(ASN1Value::Integer(0)),
                                            max: Some(ASN1Value::Integer(5)),
                                            extensible: false
                                        }
                                    ),
                                    extensible: false
                                })],
                                presence: ComponentPresence::Unspecified
                            }
                        ]
                    }
                )),
                extensible: false
            })]
        );
    }

    #[test]
    fn parses_composite_array_constraint() {
        assert_eq!(
            constraint(
                "((WITH COMPONENT (WITH COMPONENTS {..., eventDeltaTime PRESENT})) |
                    (WITH COMPONENT (WITH COMPONENTS {..., eventDeltaTime ABSENT})))
                "
            )
            .unwrap()
            .1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::SetOperation(SetOperation {
                    base: SubtypeElement::MultipleTypeConstraints(InnerTypeConstraint {
                        is_partial: true,
                        constraints: vec![ConstrainedComponent {
                            identifier: "eventDeltaTime".into(),
                            constraints: vec![],
                            presence: ComponentPresence::Present
                        }]
                    }),
                    operator: SetOperator::Union,
                    operant: Box::new(ElementOrSetOperation::Element(
                        SubtypeElement::MultipleTypeConstraints(InnerTypeConstraint {
                            is_partial: true,
                            constraints: vec![ConstrainedComponent {
                                identifier: "eventDeltaTime".into(),
                                constraints: vec![],
                                presence: ComponentPresence::Absent
                            }]
                        })
                    ))
                }),
                extensible: false
            })]
        );
    }

    #[test]
    fn parses_composite_component_constraint() {
        assert_eq!(
            constraint(
                "((WITH COMPONENTS {..., laneId PRESENT, connectionId ABSENT }) |
                    (WITH COMPONENTS {..., laneId ABSENT, connectionId PRESENT }))
                "
            )
            .unwrap()
            .1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::SetOperation(SetOperation {
                    base: SubtypeElement::SingleTypeConstraint(InnerTypeConstraint {
                        is_partial: true,
                        constraints: vec![
                            ConstrainedComponent {
                                identifier: "laneId".into(),
                                constraints: vec![],
                                presence: ComponentPresence::Present
                            },
                            ConstrainedComponent {
                                identifier: "connectionId".into(),
                                constraints: vec![],
                                presence: ComponentPresence::Absent
                            }
                        ]
                    }),
                    operator: SetOperator::Union,
                    operant: Box::new(ElementOrSetOperation::Element(
                        SubtypeElement::SingleTypeConstraint(InnerTypeConstraint {
                            is_partial: true,
                            constraints: vec![
                                ConstrainedComponent {
                                    identifier: "laneId".into(),
                                    constraints: vec![],
                                    presence: ComponentPresence::Absent
                                },
                                ConstrainedComponent {
                                    identifier: "connectionId".into(),
                                    constraints: vec![],
                                    presence: ComponentPresence::Present
                                }
                            ]
                        })
                    ))
                }),
                extensible: false
            })]
        );
    }

    #[test]
    fn parses_composite_range_constraint() {
        assert_eq!(
            constraint(
                "(0..3|5..8|10)
                "
            )
            .unwrap()
            .1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::SetOperation(SetOperation {
                    base: SubtypeElement::ValueRange {
                        min: Some(ASN1Value::Integer(0)),
                        max: Some(ASN1Value::Integer(3)),
                        extensible: false
                    },
                    operator: SetOperator::Union,
                    operant: Box::new(ElementOrSetOperation::SetOperation(SetOperation {
                        base: SubtypeElement::ValueRange {
                            min: Some(ASN1Value::Integer(5)),
                            max: Some(ASN1Value::Integer(8)),
                            extensible: false
                        },
                        operator: SetOperator::Union,
                        operant: Box::new(ElementOrSetOperation::Element(
                            SubtypeElement::SingleValue {
                                value: ASN1Value::Integer(10),
                                extensible: false
                            }
                        ))
                    }))
                }),
                extensible: false
            })]
        );
    }

    #[test]
    fn parses_composite_range_constraint_with_elsewhere_declared_values() {
        assert_eq!(
            constraint(
                "(unknown   | passengerCar..tram
                  | agricultural)"
            )
            .unwrap()
            .1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::SetOperation(SetOperation {
                    base: SubtypeElement::SingleValue {
                        value: ASN1Value::ElsewhereDeclaredValue("unknown".to_string()),
                        extensible: false
                    },
                    operator: SetOperator::Union,
                    operant: Box::new(ElementOrSetOperation::SetOperation(SetOperation {
                        base: SubtypeElement::ValueRange {
                            min: Some(ASN1Value::ElsewhereDeclaredValue(
                                "passengerCar".to_string()
                            )),
                            max: Some(ASN1Value::ElsewhereDeclaredValue("tram".to_string())),
                            extensible: false
                        },
                        operator: SetOperator::Union,
                        operant: Box::new(ElementOrSetOperation::Element(
                            SubtypeElement::SingleValue {
                                value: ASN1Value::ElsewhereDeclaredValue(
                                    "agricultural".to_string()
                                ),
                                extensible: false
                            }
                        ))
                    }))
                }),
                extensible: false
            })]
        );
    }

    #[test]
    fn parses_table_constraint() {
        assert_eq!(
            constraint(
                "({
                  My-ops |
                  {
                    &id 5,
                    &Type INTEGER (1..6)
                  } |
                  {ConnectionManeuverAssist-addGrpC  IDENTIFIED BY addGrpC},
                  ...
                })"
            )
            .unwrap()
            .1,
            vec![Constraint::TableConstraint(TableConstraint {
                object_set: ObjectSet {
                    values: vec![
                        ObjectSetValue::Reference("My-ops".into()),
                        ObjectSetValue::Inline(InformationObjectFields::DefaultSyntax(vec![
                            InformationObjectField::FixedValueField(FixedValueField {
                                identifier: "&id".into(),
                                value: ASN1Value::Integer(5)
                            }),
                            InformationObjectField::TypeField(TypeField {
                                identifier: "&Type".into(),
                                r#type: ASN1Type::Integer(Integer {
                                    constraints: vec![Constraint::SubtypeConstraint(ElementSet {
                                        set: ElementOrSetOperation::Element(
                                            SubtypeElement::ValueRange {
                                                min: Some(ASN1Value::Integer(1)),
                                                max: Some(ASN1Value::Integer(6)),
                                                extensible: false
                                            }
                                        ),
                                        extensible: false
                                    })],
                                    distinguished_values: None
                                })
                            })
                        ])),
                        ObjectSetValue::Inline(InformationObjectFields::CustomSyntax(vec![
                            SyntaxApplication::TypeReference(ASN1Type::ElsewhereDeclaredType(
                                DeclarationElsewhere {
                                    identifier: "ConnectionManeuverAssist-addGrpC".into(),
                                    constraints: vec![]
                                }
                            )),
                            SyntaxApplication::Literal("IDENTIFIED".into()),
                            SyntaxApplication::Literal("BY".into()),
                            SyntaxApplication::ValueReference(ASN1Value::ElsewhereDeclaredValue(
                                "addGrpC".into()
                            ))
                        ]))
                    ],
                    extensible: Some(3)
                },
                linked_fields: vec![]
            })]
        );
    }

    #[test]
    fn parses_character_value_range() {
        assert_eq!(
            value_range(r#""a".."z""#).unwrap().1,
            SubtypeElement::ValueRange {
                min: Some(ASN1Value::String("a".to_owned())),
                max: Some(ASN1Value::String("z".to_owned())),
                extensible: false
            }
        )
    }

    #[test]
    fn parses_permitted_alphabet_constraint() {
        assert_eq!(
            permitted_alphabet_constraint(r#"(FROM ("a".."z" | "A".."Z" | "0".."9" | ".-"))"#)
                .unwrap()
                .1,
            SubtypeElement::PermittedAlphabet(Box::new(ElementOrSetOperation::SetOperation(
                SetOperation {
                    base: SubtypeElement::ValueRange {
                        min: Some(ASN1Value::String("a".to_owned())),
                        max: Some(ASN1Value::String("z".to_owned())),
                        extensible: false
                    },
                    operator: SetOperator::Union,
                    operant: Box::new(ElementOrSetOperation::SetOperation(SetOperation {
                        base: SubtypeElement::ValueRange {
                            min: Some(ASN1Value::String("A".to_owned())),
                            max: Some(ASN1Value::String("Z".to_owned())),
                            extensible: false
                        },
                        operator: SetOperator::Union,
                        operant: Box::new(ElementOrSetOperation::SetOperation(SetOperation {
                            base: SubtypeElement::ValueRange {
                                min: Some(ASN1Value::String("0".to_owned())),
                                max: Some(ASN1Value::String("9".to_owned())),
                                extensible: false
                            },
                            operator: SetOperator::Union,
                            operant: Box::new(ElementOrSetOperation::Element(
                                SubtypeElement::SingleValue {
                                    value: ASN1Value::String(".-".to_owned()),
                                    extensible: false
                                }
                            ))
                        }))
                    }))
                }
            )))
        )
    }

    #[test]
    fn parses_serial_constraints() {
        assert_eq!(
            constraint(r#"(FROM ("a".."z" | "A".."Z" | "0".."9" | ".-")) (SIZE (1..255))"#)
                .unwrap()
                .1,
            vec![
                Constraint::SubtypeConstraint(ElementSet {
                    set: ElementOrSetOperation::Element(SubtypeElement::PermittedAlphabet(
                        Box::new(ElementOrSetOperation::SetOperation(SetOperation {
                            base: SubtypeElement::ValueRange {
                                min: Some(ASN1Value::String("a".to_owned())),
                                max: Some(ASN1Value::String("z".to_owned())),
                                extensible: false
                            },
                            operator: SetOperator::Union,
                            operant: Box::new(ElementOrSetOperation::SetOperation(SetOperation {
                                base: SubtypeElement::ValueRange {
                                    min: Some(ASN1Value::String("A".to_owned())),
                                    max: Some(ASN1Value::String("Z".to_owned())),
                                    extensible: false
                                },
                                operator: SetOperator::Union,
                                operant: Box::new(ElementOrSetOperation::SetOperation(
                                    SetOperation {
                                        base: SubtypeElement::ValueRange {
                                            min: Some(ASN1Value::String("0".to_owned())),
                                            max: Some(ASN1Value::String("9".to_owned())),
                                            extensible: false
                                        },
                                        operator: SetOperator::Union,
                                        operant: Box::new(ElementOrSetOperation::Element(
                                            SubtypeElement::SingleValue {
                                                value: ASN1Value::String(".-".to_owned()),
                                                extensible: false
                                            }
                                        ))
                                    }
                                ))
                            }))
                        }))
                    )),
                    extensible: false
                }),
                Constraint::SubtypeConstraint(ElementSet {
                    set: ElementOrSetOperation::Element(SubtypeElement::SizeConstraint(Box::new(
                        ElementOrSetOperation::Element(SubtypeElement::ValueRange {
                            min: Some(ASN1Value::Integer(1)),
                            max: Some(ASN1Value::Integer(255)),
                            extensible: false
                        })
                    ))),
                    extensible: false
                })
            ]
        )
    }

    #[test]
    fn parses_real_constraint() {
        assert_eq!(
            constraint(
                r#"(WITH COMPONENTS {
                mantissa (-16777215..16777215),
                base (2),
                exponent (-125..128) } )"#
            )
            .unwrap()
            .1,
            vec![Constraint::SubtypeConstraint(ElementSet {
                set: ElementOrSetOperation::Element(SubtypeElement::SingleTypeConstraint(
                    InnerTypeConstraint {
                        is_partial: false,
                        constraints: vec![
                            ConstrainedComponent {
                                identifier: "mantissa".into(),
                                constraints: vec![Constraint::SubtypeConstraint(ElementSet {
                                    set: ElementOrSetOperation::Element(
                                        SubtypeElement::ValueRange {
                                            min: Some(ASN1Value::Integer(-16777215)),
                                            max: Some(ASN1Value::Integer(16777215)),
                                            extensible: false
                                        }
                                    ),
                                    extensible: false
                                })],
                                presence: ComponentPresence::Unspecified
                            },
                            ConstrainedComponent {
                                identifier: "base".into(),
                                constraints: vec![Constraint::SubtypeConstraint(ElementSet {
                                    set: ElementOrSetOperation::Element(
                                        SubtypeElement::SingleValue {
                                            value: ASN1Value::Integer(2),
                                            extensible: false
                                        }
                                    ),
                                    extensible: false
                                })],
                                presence: ComponentPresence::Unspecified
                            },
                            ConstrainedComponent {
                                identifier: "exponent".into(),
                                constraints: vec![Constraint::SubtypeConstraint(ElementSet {
                                    set: ElementOrSetOperation::Element(
                                        SubtypeElement::ValueRange {
                                            min: Some(ASN1Value::Integer(-125)),
                                            max: Some(ASN1Value::Integer(128)),
                                            extensible: false
                                        }
                                    ),
                                    extensible: false
                                })],
                                presence: ComponentPresence::Unspecified
                            }
                        ]
                    }
                )),
                extensible: false
            })]
        )
    }

    #[test]
    fn parses_pattern_constraint() {
        assert_eq!(constraint(
            r#"(PATTERN "[a-zA-Z]#(1,8)(-[a-zA-Z0-9]#(1,8))*")"#
        ).unwrap().1, 
        vec![
            Constraint::SubtypeConstraint(
                ElementSet { 
                    set: ElementOrSetOperation::Element(
                        SubtypeElement::PatternConstraint(
                            PatternConstraint {
                                pattern: "[a-zA-Z]#(1,8)(-[a-zA-Z0-9]#(1,8))*".into()
                            }
                            )
                        ), 
                        extensible: false 
                    }
                )
            ]
        )
    }

    #[test]
    fn parses_user_defined_constraint() {
        assert_eq!(
            constraint(
                r#"(CONSTRAINED BY {/* XML representation of the XSD pattern "\d\d\d\d-\d\d-\d\dT\d\d:\d\d:\d\d[-,+]\d\d:\d\d" */})"#
            ).unwrap().1, 
            vec![
                Constraint::SubtypeConstraint(
                    ElementSet { 
                        set: ElementOrSetOperation::Element(
                            SubtypeElement::UserDefinedConstraint(
                                UserDefinedConstraint { 
                                    definition: "/* XML representation of the XSD pattern \"\\d\\d\\d\\d-\\d\\d-\\d\\dT\\d\\d:\\d\\d:\\d\\d[-,+]\\d\\d:\\d\\d\" */".into()
                                }
                                )
                            ), 
                            extensible: false 
                        }
                    )
                ]
            )
    }
}
