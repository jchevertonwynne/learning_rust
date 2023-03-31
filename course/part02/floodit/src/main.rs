use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use enum_iterator::Sequence;
use rand::{distributions::Standard, prelude::Distribution};

use Colour::*;

fn main() -> anyhow::Result<()> {
    let mut game = Game::new();
    println!("{}", game);

    loop {
        println!("{}", prompt_string());
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf)?;

        let Ok(chosen) = buf.trim().parse() else {
            continue;
        };

        game.apply(chosen);
        println!("{}", game);
        if game.won() {
            break;
        }
    }

    println!("you won in {} turns!", game.turns);

    Ok(())
}

fn prompt_string() -> String {
    let colours: Vec<_> = enum_iterator::all::<Colour>()
        .map(|c| format!("{c:?}"))
        .collect();

    format!(
        "please enter {} or {}",
        colours[..colours.len() - 1].join(", "),
        colours[colours.len() - 1]
    )
}

struct Game{
    board: [[Colour; 10]; 10],
    turns: usize,
}

impl Game {
    fn new() -> Game {
        Game {
            board: std::array::from_fn(|_| std::array::from_fn(|_| rand::random())),
            turns: 0,
        }
    }

    // takes a colour and applies it to the board
    fn apply(&mut self, new_colour: Colour) {
        todo!("please fill me in!")
    }

    // returns true if all tiles are the same colour
    fn won(&self) -> bool {
        todo!("please fill me in!")
    }
}

impl Display for Game {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "turns: {}", self.turns)?;
        for row in self.board.iter() {
            for cell in row.iter() {
                write!(f, "{cell}")?;
            }
            writeln!(f)?;
        }

        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, Sequence)]
enum Colour {
    Red,
    Green,
    Blue,
    Purple,
    White,
    Yellow,
}

impl Debug for Colour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Red => write!(f, "red"),
            Self::Green => write!(f, "green"),
            Self::Blue => write!(f, "blue"),
            Self::Purple => write!(f, "purple"),
            Self::White => write!(f, "white"),
            Self::Yellow => write!(f, "yellow"),
        }
    }
}

struct InvalidColourError;

impl FromStr for Colour {
    type Err = InvalidColourError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let colour = match s {
            "red" => Red,
            "green" => Green,
            "blue" => Blue,
            "purple" => Purple,
            "white" => White,
            "yellow" => Yellow,
            _ => return Err(InvalidColourError),
        };
        Ok(colour)
    }
}

impl Distribution<Colour> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Colour {
        match rng.gen_range(0..=6) {
            0 => Red,
            1 => Green,
            2 => Blue,
            3 => Purple,
            4 => White,
            _ => Yellow,
        }
    }
}

impl Display for Colour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut style = ansi_term::Style::new();
        style.background = Some(match self {
            Red => ansi_term::Colour::Red,
            Green => ansi_term::Colour::Green,
            Blue => ansi_term::Colour::Blue,
            Purple => ansi_term::Colour::Purple,
            White => ansi_term::Colour::White,
            Yellow => ansi_term::Colour::Yellow,
        });
        write!(f, "{}", style.paint("  "))
    }
}
