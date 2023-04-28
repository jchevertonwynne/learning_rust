use std::collections::{HashMap, HashSet};
use std::str::from_utf8;

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct BagItem<'a> {
    bag_color: &'a str,
    count: i32,
}

impl<'a> BagItem<'a> {
    pub fn new(line: &'a str) -> Self {
        let (count_str, bag_color_str) = line.split_once(" ").unwrap();
        let bag_count = count_str.parse().unwrap();
        let bag_color = bag_color_str
            .trim_end_matches("s")
            .trim_end_matches(" bag");
        return BagItem{bag_color: bag_color, count: bag_count};
    }
}

#[derive(Clone, Debug)]
pub struct BagRule<'a> {
    bag_color: &'a str,
    contains: Option<Vec<BagItem<'a>>>,
}

impl<'a> BagRule<'a> {
    fn new(line: &'a str) -> Self {
        let (bag_color, bag_contents_string) = line.split_once(" bags contain ").unwrap();
   
        // Return if there is no other bags inside
        if bag_contents_string == "no other bags." {
            return BagRule {bag_color, contains: None};
        }
        // Split the contents each content type, seperated by commas
        let bag_items = bag_contents_string
            .trim_end_matches(".")
            .split(", ")
            .map(|item_string| BagItem::new(item_string))
            .collect();

        return BagRule{bag_color,contains: Some(bag_items)};
    }
}

pub fn parse_input() -> Vec<BagRule<'static>> {
    let bytes = include_bytes!("data/input.txt");
    let data = from_utf8(bytes).unwrap();

    data.lines()
        .map(|line| BagRule::new(line))
        .collect::<Vec<_>>()
}

pub fn generate_lookups<'a>(rules: &'a Vec<BagRule<'a>>) -> (HashMap<&'a str, HashSet<&'a str>>, HashMap<&'a str, &'a Vec<BagItem<'a>>>) {
    let mut lookup_is_inside = HashMap::new();
    let mut lookup_contains = HashMap::new();

    rules.iter().for_each(|rule| {
        if let Some(value) = &rule.contains {
            value.iter().for_each(|x| {
                lookup_is_inside.entry(x.bag_color)
                    .or_insert_with(|| {HashSet::new()})
                    .insert(rule.bag_color);
            });
            lookup_contains.insert(rule.bag_color, value);
        }
    });

    (lookup_is_inside, lookup_contains)
}

// Calculate which bag colors contain a bag of the provided color.
pub fn bag_colors_contain<'a>(lookup: &HashMap<&'a str, HashSet<&'a str>>, color: &str) -> HashSet<&'a str>{
    lookup.get(&color).map_or(HashSet::new(), |contained_by|{
        let mut result = HashSet::new();
        result.extend(contained_by);
        contained_by.iter().fold(result, |mut c, contained_by_color| {
            c.extend(bag_colors_contain(lookup, contained_by_color.clone()));
            c
        })
    })
}

// Count the number bags inside a single bag of the provided color.
// If the inside bag contains other bags, then count how many bags are inside that (recursively)
pub fn count_bags_inside<'a>(lookup: &HashMap<&'a str, &Vec<BagItem<'a>>>, color: &'a str) -> i32 {
    const EMPTY_BAG_COUNT: i32 = 0;

    lookup.get(&color).map_or(EMPTY_BAG_COUNT, |contents|{
        contents.iter().map(|content_type| {
            let bags_inside = count_bags_inside(lookup, content_type.bag_color.clone());
            content_type.count * (1 + bags_inside) // 1 for the bag itself
        }).sum()
    })
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