use std::num::ParseIntError;

fn main() -> anyhow::Result<()> {
    let filename = std::env::args()
        .skip(1)
        .next()
        .unwrap_or_else(|| "yolo".to_string());

    let int = int_from_file(&filename)?;
    println!("int is {int}");
    Ok(())
}

fn int_from_file(filename: &str) -> Result<u64, ReadError> {
    let contents = std::fs::read_to_string(filename)?;
    // let my_int = contents.parse()?;

    let my_int =match contents.parse::<u64>() {
        Ok(i) => i,
        Err(err) => return Err(err.into()),
    };

    Ok(my_int)
}

#[derive(thiserror::Error, Debug)]
enum ReadError {
    #[error("failed to parse integer: {0}")]
    ParseError(#[from] ParseIntError),
    #[error("failed to read file: {0}")]
    IoError(#[from] std::io::Error),
}
