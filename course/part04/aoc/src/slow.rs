use std::{collections::{HashMap, HashSet}};

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct BagItem {
    bag_color: String,
    count: i32,
}

impl BagItem {
    pub fn new(line: String) -> Self {
        let line = line.replace(" bags", "");
        let line = line.replace(" bag", "");

        let space_separator = String::from(" ");
        let parts = line.split(&space_separator)
            .map(|part| part.to_string())
            .collect::<Vec<_>>();

        let bag_count = parts.get(0).unwrap().parse().unwrap();
        let bag_color_first = parts.get(1).unwrap().clone();
        let bag_color_second = parts.get(2).unwrap().clone();
        let bag_color = format!("{bag_color_first} {bag_color_second}");
        BagItem{bag_color, count: bag_count}
    }
}

#[derive(Clone, Debug)]
pub struct BagRule {
    bag_color: String,
    contains: Option<Vec<BagItem>>,
}

#[allow(clippy::cmp_owned)]
impl BagRule {
    fn new(line: String) -> Self {
        // Split the line into the bag color and the contents
        let first_split_separator = String::from(" bags contain ");
        let parts: Vec<String> = line
            .split(first_split_separator.as_str())
            .map(|part| part.to_string())
            .collect();

        let bag_color = parts.get(0).unwrap().clone();
        let bag_contents_string = parts.get(1).unwrap().clone();

        // Remove dot from the end
        let dot_matcher = String::from(".");
        let bag_contents_string = bag_contents_string.trim_end_matches(&dot_matcher).to_string();

        // Return if there is no other bags inside
        if bag_contents_string == String::from("no other bags") {
            return BagRule {bag_color, contains: None};
        }

        // Split the contents each content type, separated by commas
        let comma_separator = String::from(", ");
        let bag_items = bag_contents_string
            .split(&comma_separator)
            .map(|content| content.to_string())
            .collect::<Vec<_>>();

        let bag_items = bag_items.iter()
            .map(|item_string| BagItem::new(item_string.to_string()))
            .collect();

        BagRule{bag_color,contains: Some(bag_items)}
    }
}

pub fn parse_input(data: &str) -> Vec<BagRule> {
    data.to_string().lines()
        .map(|line| BagRule::new(line.to_string()))
        .collect::<Vec<_>>()
}

pub fn generate_lookups(rules: Vec<BagRule>) -> (HashMap<String, HashSet<String>>, HashMap<String, Vec<BagItem>>) {
    let mut lookup_is_inside = HashMap::new();
    let mut lookup_contains = HashMap::new();

    rules.iter().for_each(|rule| {
        let r = rule.clone();
        if let Some(value) = r.contains {
            value.iter().for_each(|x| {
                lookup_is_inside.entry(x.bag_color.clone())
                    .or_insert_with(|| {HashSet::new()})
                    .insert(rule.bag_color.clone());
            });
            lookup_contains.insert(r.bag_color, value);
        }
    });

    (lookup_is_inside, lookup_contains)
}

// Calculate which bag colors contain a bag of the provided color.
pub fn bag_colors_contain(lookup: &HashMap<String, HashSet<String>>, color: String) -> HashSet<String>{
    lookup.get(&color).map_or(HashSet::new(), |contained_by|{
        contained_by.iter().fold(contained_by.clone(), |mut c, contained_by_color| {
            c.extend(bag_colors_contain(lookup, contained_by_color.clone()));
            c
        })
    })
}

// Count the number bags inside a single bag of the provided color.
// If the inside bag contains other bags, then count how many bags are inside that (recursively)
pub fn count_bags_inside(lookup: &HashMap<String, Vec<BagItem>>, color: String) -> i32 {
    const EMPTY_BAG_COUNT: i32 = 0;

    lookup.get(&color).map_or(EMPTY_BAG_COUNT, |contents|{
        contents.iter().map(|content_type| {
            let bags_inside = count_bags_inside(lookup, content_type.bag_color.clone());
            content_type.count * (1 + bags_inside) // 1 for the bag itself
        }).sum()
    })
}

pub fn run_calc() -> (usize, i32) {
    let data = include_str!("../src/data/input.txt");
    let rules = parse_input(data);
    let (lookup_inside, lookup_contains) = generate_lookups(rules);
    const SHINY_GOLD_BAG: &str = "shiny gold";
    let colors = bag_colors_contain(&lookup_inside,  SHINY_GOLD_BAG.to_string());
    let bags_inside = count_bags_inside(&lookup_contains, "shiny gold".to_string());
    (colors.len(), bags_inside)
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

