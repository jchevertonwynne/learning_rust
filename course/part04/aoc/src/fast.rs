


#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct BagItem<'a> {
    bag_color: &'a str,
    count: i32,
}

pub fn run_calc() -> (usize, i32) {
    todo!("Implement the fast version of the code and enable the fast run in the benchmarks to compare");
    
    // let data = include_str!("../src/data/input.txt");
    // let rules = parse_input(data);
    // let (lookup_inside, lookup_contains) = generate_lookups(rules);
    // const SHINY_GOLD_BAG: &str = "shiny gold";
    // let colors = bag_colors_contain(&lookup_inside,  SHINY_GOLD_BAG.to_string());
    // let bags_inside = count_bags_inside(&lookup_contains, "shiny gold".to_string());
    // return (colors.len(), bags_inside);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_calc() {
        let result = run_calc();
        assert_eq!(result, (222, 13264));
    }
}