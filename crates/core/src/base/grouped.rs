pub fn by_key<T, K: PartialEq>(
    items: impl IntoIterator<Item = T>,
    key: impl Fn(&T) -> K,
) -> Vec<(K, Vec<T>)> {
    let mut groups: Vec<(K, Vec<T>)> = Vec::new();
    for item in items {
        let k = key(&item);
        match groups.iter_mut().find(|(known, _)| *known == k) {
            Some((_, held)) => held.push(item),
            None => groups.push((k, vec![item])),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_come_in_the_order_met_and_keep_their_items_in_order() {
        let grouped = by_key(["bee", "ant", "bat", "cow", "asp"], |word| {
            word.chars().next()
        });
        assert_eq!(
            grouped,
            [
                (Some('b'), vec!["bee", "bat"]),
                (Some('a'), vec!["ant", "asp"]),
                (Some('c'), vec!["cow"]),
            ]
        );
        assert!(by_key(Vec::<u8>::new(), |n| *n).is_empty());
    }
}
