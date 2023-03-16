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
        .map(|c| {
            let s = format!("{c:?}");
            format!("'({}){}'", &s[..1], &s[1..])
        })
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
        let curr_colour = self.board[0][0];
        if curr_colour == new_colour {
            return;
        }
        self._apply(curr_colour, new_colour, 0, 0);
        self.turns += 1;
    }

    fn _apply(&mut self, curr_colour: Colour, new_colour: Colour, x: usize, y: usize) {
        self.board[y][x] = new_colour;
        if x >= 1 && self.board[y][x - 1] == curr_colour {
            self._apply(curr_colour, new_colour, x - 1, y);
        }
        if x + 1 < self.board[0].len() && self.board[y][x + 1] == curr_colour {
            self._apply(curr_colour, new_colour, x + 1, y);
        }
        if y >= 1 && self.board[y - 1][x] == curr_colour {
            self._apply(curr_colour, new_colour, x, y - 1);
        }
        if y + 1 < self.board.len() && self.board[y + 1][x] == curr_colour {
            self._apply(curr_colour, new_colour, x, y + 1);
        }
    }

    // returns true if all tiles are the same colour
    fn won(&self) -> bool {
        let owned = self.board[0][0];
        self.board
            .iter()
            .flat_map(|row| row.iter())
            .all(|cell| *cell == owned)
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
            "r" | "red" => Red,
            "g" | "green" => Green,
            "b" | "blue" => Blue,
            "p" | "purple" => Purple,
            "w" | "white" => White,
            "y" | "yellow" => Yellow,
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
