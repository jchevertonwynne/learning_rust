use std::marker::PhantomData;

fn main() {
    let finder = Pathfinder::<Manhattan>::new(Coord::new(0.0, 0.0), Coord::new(4.0, 3.0));
    println!("manhattan dist = {}", finder.find_path());

    let finder = Pathfinder::<Euclidian>::new(Coord::new(0.0, 0.0), Coord::new(4.0, 3.0));
    println!("euclidian dist = {}", finder.find_path());
}

struct Pathfinder<D> {
    a: Coord,
    b: Coord,
    _pd: PhantomData<D>,
}

impl<D> Pathfinder<D> {
    fn new(a: Coord, b: Coord) -> Self {
        Self {
            a,
            b,
            _pd: PhantomData,
        }
    }
}

impl<D> Pathfinder<D>
where
    D: Distance,
{
    fn find_path(&self) -> f64 {
        D::distance(self.a, self.b)
    }
}

trait Distance {
    fn distance(a: Coord, b: Coord) -> f64;
}

#[derive(Debug, Clone, Copy)]
struct Coord {
    x: f64,
    y: f64,
}

impl Coord {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

struct Manhattan;

impl Distance for Manhattan {
    fn distance(a: Coord, b: Coord) -> f64 {
        (a.x - b.x).abs() + (a.y - b.y).abs()
    }
}

struct Euclidian;

impl Distance for Euclidian {
    fn distance(a: Coord, b: Coord) -> f64 {
        let x = a.x - b.x;
        let y = a.y - b.y;
        (x.powi(2) + y.powi(2)).sqrt()
    }
}
