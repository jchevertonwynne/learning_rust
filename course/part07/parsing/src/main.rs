#![allow(dead_code)]

use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{all_consuming, map},
    multi::separated_list0,
    sequence::{delimited, tuple},
    IResult,
};
use std::fmt::{Debug, Formatter};

fn main() -> anyhow::Result<()> {
    let lines = parse_lines(SENSORS)?.1;

    println!("{lines:#?}");

    for packet_str in PACKETS.lines() {
        let packet = parse_packet_line(packet_str)?.1;
        println!("{packet:?}");
    }

    Ok(())
}

const SENSORS: &str = "Sensor at x=2, y=18: closest beacon is at x=-2, y=15
Sensor at x=9, y=16: closest beacon is at x=10, y=16
Sensor at x=13, y=2: closest beacon is at x=15, y=3";

#[derive(Debug)]
struct Line {
    sensor: Coord,
    beacon: Coord,
}

#[derive(Debug)]
struct Coord {
    x: i8,
    y: i8,
}

fn parse_lines(input: &str) -> IResult<&str, Vec<Line>> {
    all_consuming(separated_list0(tag("\n"), parse_line))(input)
}

fn parse_line(line: &str) -> IResult<&str, Line> {
    map(
        tuple((
            tag("Sensor at x="),
            nom::character::complete::i8,
            tag(", y="),
            nom::character::complete::i8,
            tag(": closest beacon is at x="),
            nom::character::complete::i8,
            tag(", y="),
            nom::character::complete::i8,
        )),
        |(_, sx, _, sy, _, bx, _, by)| Line {
            sensor: Coord { x: sx, y: sy },
            beacon: Coord { x: bx, y: by },
        },
    )(line)
}

const PACKETS: &str = "[]
[[]]
[[[]]]
[1,1,3,1,1]
[1,1,5,1,1]
[[1],[2,3,4]]
[1,[2,[3,[4,[5,6,0]]]],8,9]
[1,[2,[3,[4,[5,6,7]]]],8,9]
[[1],4]
[[2]]
[3]
[[4,4],4,4]
[[4,4],4,4,4]
[[6]]
[7,7,7]
[7,7,7,7]
[[8,7,6]]
[9]";

enum Item {
    Value(u8),
    Packet(Packet),
}

struct Packet(Vec<Item>);

fn parse_item(input: &str) -> IResult<&str, Item> {
    alt((
        map(nom::character::complete::u8, Item::Value),
        map(_parse_packet, Item::Packet),
    ))(input)
}

fn parse_packet_line(input: &str) -> IResult<&str, Packet> {
    all_consuming(_parse_packet)(input)
}

fn _parse_packet(input: &str) -> IResult<&str, Packet> {
    map(
        delimited(tag("["), separated_list0(tag(","), parse_item), tag("]")),
        Packet,
    )(input)
}

impl Debug for Item {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Item::Value(v) => write!(f, "{v}"),
            Item::Packet(p) => write!(f, "{p:?}"),
        }
    }
}

impl Debug for Packet {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;
        if !self.0.is_empty() {
            match self.0.split_last() {
                None => write!(f, "{:?}", self.0[0])?,
                Some((last, rest)) => {
                    for r in rest {
                        write!(f, "{r:?},")?;
                    }
                    write!(f, "{last:?}")?;
                }
            }
        }
        write!(f, "]")
    }
}
