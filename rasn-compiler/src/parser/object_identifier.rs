//! The `object_identifier` module contains parsers for
//! parsing ASN1 OBJECT IDENTIFIERs in a specification.
//! OBJECT IDENTIFIERs serve to uniquely and globally (really globally!)
//! identify a so-called _information object_.
use crate::intermediate::{
    ASN1Type, ObjectIdentifierArc, ObjectIdentifierValue, OBJECT_IDENTIFIER,
};

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::u128,
    combinator::{into, map, opt},
    multi::many1,
    sequence::{pair, preceded},
    IResult,
};

use super::{
    common::{identifier, in_braces, in_parentheses, skip_ws, skip_ws_and_comments},
    constraint::constraint,
};

/// Tries to parse an ASN1 OBJECT IDENTIFIER
/// As opposed to other ASN1 "types", an OBJECT IDENTIFIER is always a value.
/// In some places of an ASN1 spec, we do not encounter the `OBJECT IDENTIFIER` keyword
/// before an OBJECT IDENTIFIER value, such as in the header's module identifier.
///
/// *`input` - string slice to be matched against
///
/// `object_identifier` will try to match an OBJECT IDENTIFIER declaration in the `input` string.
/// If the match succeeds, the parser will consume the match and return the remaining string
/// and an `ObjectIdentifier` value representing the ASN1 declaration.
/// If the match fails, the parser will not consume the input and will return an error.
pub fn object_identifier_value<'a>(input: &'a str) -> IResult<&'a str, ObjectIdentifierValue> {
    into(skip_ws_and_comments(preceded(
        opt(tag(OBJECT_IDENTIFIER)),
        in_braces(many1(skip_ws(object_identifier_arc))),
    )))(input)
}

pub fn object_identifier<'a>(input: &'a str) -> IResult<&'a str, ASN1Type> {
    map(
        into(preceded(
            skip_ws_and_comments(tag(OBJECT_IDENTIFIER)),
            opt(skip_ws_and_comments(constraint)),
        )),
        |oid| ASN1Type::ObjectIdentifier(oid),
    )(input)
}

fn object_identifier_arc<'a>(input: &'a str) -> IResult<&'a str, ObjectIdentifierArc> {
    skip_ws(alt((
        numeric_id,
        into(pair(identifier, skip_ws(in_parentheses(u128)))),
        into(identifier),
    )))(input)
}

fn numeric_id<'a>(input: &'a str) -> IResult<&'a str, ObjectIdentifierArc> {
    map(u128, |i| i.into())(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ansi_x9_62() {
        assert_eq!(
            object_identifier_value(r#"{iso(1) member-body(2) us(840) 10045 modules(0) 2}"#)
                .unwrap()
                .1,
            ObjectIdentifierValue(vec![
                ObjectIdentifierArc {
                    name: Some("iso".into()),
                    number: Some(1)
                },
                ObjectIdentifierArc {
                    name: Some("member-body".into()),
                    number: Some(2)
                },
                ObjectIdentifierArc {
                    name: Some("us".into()),
                    number: Some(840)
                },
                ObjectIdentifierArc {
                    name: None,
                    number: Some(10045)
                },
                ObjectIdentifierArc {
                    name: Some("modules".into()),
                    number: Some(0)
                },
                ObjectIdentifierArc {
                    name: None,
                    number: Some(2)
                },
            ])
        )
    }
}
