use std::collections::HashMap;
use std::hash::Hash;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Remove { first: usize, last: usize },
    Insert { first: usize, last: usize },
    Move { from: usize, to: usize },
}

/// Past this, a new list is quicker to lay out than the moves.
pub const MOST_STEPS: usize = 64;

/// `None` past `MOST_STEPS`. Keys are expected once in each list.
pub fn steps<K: Eq + Hash + Clone>(before: &[K], after: &[K]) -> Option<Vec<Step>> {
    let wanted: HashMap<&K, usize> = after.iter().enumerate().map(|(at, k)| (k, at)).collect();
    let mut now: Vec<K> = before.to_vec();
    let mut steps = Vec::new();
    removals(&mut now, &|key| wanted.contains_key(key), &mut steps);
    let moved = fewest_moves(&now, after);
    if steps.len() + moved.len() > MOST_STEPS {
        return None;
    }
    for &(from, to) in &moved {
        let key = now.remove(from);
        now.insert(to, key);
        steps.push(Step::Move { from, to });
    }
    insertions(&mut now, after, &mut steps);
    (steps.len() <= MOST_STEPS).then_some(steps)
}

fn removals<K>(now: &mut Vec<K>, keep: &dyn Fn(&K) -> bool, steps: &mut Vec<Step>) {
    let mut at = now.len();
    while at > 0 {
        at -= 1;
        if keep(&now[at]) {
            continue;
        }
        let last = at;
        while at > 0 && !keep(&now[at - 1]) {
            at -= 1;
        }
        now.drain(at..=last);
        steps.push(Step::Remove { first: at, last });
    }
}

fn insertions<K: Eq + Hash + Clone>(now: &mut Vec<K>, after: &[K], steps: &mut Vec<Step>) {
    let mut at = 0;
    while at < after.len() {
        if now.get(at) == Some(&after[at]) {
            at += 1;
            continue;
        }
        let first = at;
        while at < after.len() && now.get(at) != Some(&after[at]) {
            now.insert(at, after[at].clone());
            at += 1;
        }
        steps.push(Step::Insert {
            first,
            last: at - 1,
        });
    }
}

fn fewest_moves<K: Eq + Hash + Clone>(now: &[K], after: &[K]) -> Vec<(usize, usize)> {
    let present: std::collections::HashSet<&K> = now.iter().collect();
    let order: Vec<&K> = after.iter().filter(|k| present.contains(k)).collect();
    let down = pass(now, &order);
    let reversed_now: Vec<K> = now.iter().rev().cloned().collect();
    let reversed_order: Vec<&K> = order.iter().rev().copied().collect();
    let up = pass(&reversed_now, &reversed_order);
    if up.len() < down.len() {
        let last = now.len() - 1;
        up.into_iter()
            .map(|(from, to)| (last - from, last - to))
            .collect()
    } else {
        down
    }
}

fn pass<K: Eq + Clone>(now: &[K], order: &[&K]) -> Vec<(usize, usize)> {
    let mut working = now.to_vec();
    let mut moves = Vec::new();
    for (at, key) in order.iter().enumerate() {
        if &&working[at] == key {
            continue;
        }
        let Some(from) = working.iter().position(|k| &k == key) else {
            continue;
        };
        let moving = working.remove(from);
        working.insert(at, moving);
        moves.push((from, at));
    }
    moves
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(before: &[u32], steps: &[Step], after: &[u32]) -> Vec<u32> {
        let mut now = before.to_vec();
        for step in steps {
            match *step {
                Step::Remove { first, last } => drop(now.drain(first..=last)),
                Step::Insert { first, last } => {
                    now.splice(first..first, after[first..=last].iter().copied());
                }
                Step::Move { from, to } => {
                    let key = now.remove(from);
                    now.insert(to, key);
                }
            }
        }
        now
    }

    #[test]
    fn one_mod_moved_either_way_is_one_move() {
        let before = [1, 2, 3, 4, 5, 6];
        assert_eq!(
            steps(&before, &[1, 5, 2, 3, 4, 6]),
            Some(vec![Step::Move { from: 4, to: 1 }])
        );
        assert_eq!(
            steps(&before, &[1, 3, 4, 5, 2, 6]),
            Some(vec![Step::Move { from: 1, to: 4 }])
        );
    }

    #[test]
    fn rows_that_leave_or_come_go_in_stretches() {
        let steps = steps(&[1, 2, 3, 4, 5], &[1, 9, 8, 5]).unwrap();
        assert_eq!(
            steps,
            [
                Step::Remove { first: 1, last: 3 },
                Step::Insert { first: 1, last: 2 },
            ]
        );
        assert_eq!(super::steps::<u32>(&[], &[]), Some(vec![]));
    }

    #[test]
    fn too_many_steps_show_the_list_anew() {
        let before: Vec<u32> = (0..500).collect();
        let after: Vec<u32> = (0..500).rev().collect();
        assert_eq!(steps(&before, &after), None);
    }

    #[test]
    fn any_two_lists_are_reached_by_the_steps() {
        let mut seed = 0x5eed_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for _ in 0..2_000 {
            let mut before: Vec<u32> = (0..(next() % 12) as u32).collect();
            before.retain(|_| next() % 4 != 0);
            let mut after: Vec<u32> = before.clone();
            after.retain(|_| next() % 4 != 0);
            for extra in 100..100 + (next() % 4) as u32 {
                let at = (next() as usize) % (after.len() + 1);
                after.insert(at, extra);
            }
            for _ in 0..(next() % 3) {
                if after.len() > 1 {
                    let from = (next() as usize) % after.len();
                    let key = after.remove(from);
                    let to = (next() as usize) % (after.len() + 1);
                    after.insert(to, key);
                }
            }
            if let Some(steps) = steps(&before, &after) {
                assert_eq!(
                    applied(&before, &steps, &after),
                    after,
                    "{before:?} -> {after:?}"
                );
            }
        }
    }
}
