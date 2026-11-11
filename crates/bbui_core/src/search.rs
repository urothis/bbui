//! Deterministic fuzzy matching for menu commands.

use std::collections::HashMap;

#[derive(Default, Debug)]
pub struct MenuSearch {
    recency: HashMap<String, u64>,
    clock: u64,
}

impl MenuSearch {
    /// Remember a command selection in this menu's own recency context.
    pub fn pick(&mut self, id: impl Into<String>) {
        self.clock = self.clock.saturating_add(1);
        self.recency.insert(id.into(), self.clock);
    }

    /// Forget this menu context's recent selections. Recreating a `MenuSearch`
    /// also starts with empty recency.
    pub fn clear_history(&mut self) {
        self.recency.clear();
    }

    /// Rank by match quality, then by recent selection, then by command ID.
    /// The final ID tie-break makes results stable regardless of input order.
    pub fn rank<'a>(
        &self,
        query: &str,
        items: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Vec<&'a str> {
        let mut scored: Vec<_> = items
            .into_iter()
            .filter_map(|(id, label)| {
                score(query, label).map(|match_score| {
                    (id, match_score, self.recency.get(id).copied().unwrap_or(0))
                })
            })
            .collect();
        scored.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| b.2.cmp(&a.2))
                .then_with(|| a.0.cmp(b.0))
        });
        scored.into_iter().map(|(id, _, _)| id).collect()
    }
}

/// Score a case-insensitive fuzzy query against a label or breadcrumb.
///
/// Query terms are matched against individual label words, so a typo in one
/// term does not prevent the remaining terms from matching. A single compact
/// query can also match a subsequence across word initials (for example `ac`
/// against `Add Cube`).
pub fn score(query: &str, label: &str) -> Option<i32> {
    let query_words = words(query);
    if query_words.is_empty() {
        return Some(0);
    }
    let label_words = words(label);
    if label_words.is_empty() {
        return None;
    }

    let token_score = query_words
        .iter()
        .map(|query_word| {
            label_words
                .iter()
                .filter_map(|label_word| score_word(query_word, label_word))
                .max()
        })
        .try_fold(0, |sum, score| score.map(|score| sum + score));

    let compact_query: Vec<char> = query_words.iter().flatten().copied().collect();
    let label_chars: Vec<char> = label_words.iter().flatten().copied().collect();
    let compact_score = score_subsequence(&compact_query, &label_chars);
    match (token_score, compact_score) {
        (Some(tokens), Some(compact)) => Some(tokens.max(compact)),
        (Some(tokens), None) => Some(tokens),
        (None, Some(compact)) if query_words.len() == 1 => Some(compact),
        _ => None,
    }
}

fn words(value: &str) -> Vec<Vec<char>> {
    let mut words = Vec::new();
    let mut current = Vec::new();
    for character in value.to_lowercase().chars() {
        if character.is_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn score_word(query: &[char], word: &[char]) -> Option<i32> {
    if query.is_empty() || word.is_empty() {
        return None;
    }
    if let Some(start) = word.windows(query.len()).position(|window| window == query) {
        let extra = word.len().saturating_sub(query.len());
        return Some(80 + query.len() as i32 * 10 - start as i32 * 3 - extra as i32);
    }
    if let Some(score) = score_subsequence(query, word) {
        return Some(score);
    }

    let distance = edit_distance(query, word);
    let shortest = query.len().min(word.len());
    let maximum_distance = match shortest {
        3..=5 => 1,
        6.. => 2,
        _ => return None,
    };
    (distance <= maximum_distance).then_some(
        24 + query.len() as i32 * 4
            - distance as i32 * 10
            - query.len().abs_diff(word.len()) as i32 * 2,
    )
}

fn score_subsequence(query: &[char], label: &[char]) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let mut at = 0;
    let mut score = 16;
    let mut previous = None;
    for wanted in query {
        let position = (at..label.len()).find(|&index| label[index] == *wanted)?;
        score += 8;
        if position == 0 {
            score += 8;
        }
        if previous == Some(position.saturating_sub(1)) {
            score += 3;
        }
        score -= (position - at) as i32 * 2;
        previous = Some(position);
        at = position + 1;
    }
    Some(score - label.len() as i32 / 4)
}

fn edit_distance(a: &[char], b: &[char]) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, left) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, right) in b.iter().enumerate() {
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + usize::from(left != right));
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_word_initials_and_uses_recency_for_ties() {
        assert!(score("ad cu", "Add Cube").is_some());
        assert!(score("add cubr", "Add Cube").is_some());
        assert!(score("ad cubr", "Object Add Cube").is_some());
        assert!(score("ob ad cu", "Object Add Cube").is_some());
        assert!(score("ad.cu", "Add Cube").is_some());
        assert!(score("cubx", "Cube").is_some());
        assert!(score("cxyz", "Cube").is_none());
        let mut s = MenuSearch::default();
        s.pick("second");
        assert_eq!(
            s.rank("cube", [("first", "Cube"), ("second", "Cube")]),
            vec!["second", "first"]
        );
        assert_eq!(s.rank("cubx", [("cube", "Cube")]), vec!["cube"]);
    }

    #[test]
    fn nested_ranking_is_deterministic_even_when_input_order_changes() {
        let candidates = [
            ("z-command", "Object Add Cube"),
            ("a-command", "Object Add Cube"),
        ];
        let expected = vec!["a-command", "z-command"];
        let search = MenuSearch::default();
        assert_eq!(search.rank("ob ad cu", candidates), expected);
        assert_eq!(
            search.rank("ob ad cu", candidates.into_iter().rev()),
            expected
        );
    }

    #[test]
    fn recent_picks_are_isolated_per_menu_context_and_can_be_cleared() {
        let candidates = [
            ("a", "Same command"),
            ("b", "Same command"),
            ("c", "Same command"),
        ];
        let mut first_menu = MenuSearch::default();
        let mut second_menu = MenuSearch::default();
        first_menu.pick("c");
        second_menu.pick("b");

        assert_eq!(first_menu.rank("same", candidates), vec!["c", "a", "b"]);
        assert_eq!(second_menu.rank("same", candidates), vec!["b", "a", "c"]);
        first_menu.clear_history();
        assert_eq!(first_menu.rank("same", candidates), vec!["a", "b", "c"]);
        assert_eq!(second_menu.rank("same", candidates), vec!["b", "a", "c"]);
    }

    #[test]
    fn ranking_is_invariant_under_every_candidate_permutation() {
        let candidates = [
            ("cube", "Add Cube"),
            ("cone", "Add Cone"),
            ("collection", "Add Collection"),
            ("duplicate", "Duplicate Object"),
        ];
        let mut search = MenuSearch::default();
        search.pick("cone");
        search.pick("cube");
        let expected = search.rank("ad c", candidates);

        fn permute(
            at: usize,
            values: &mut [(&str, &str); 4],
            search: &MenuSearch,
            expected: &[&str],
        ) {
            if at == values.len() {
                assert_eq!(search.rank("ad c", values.iter().copied()), expected);
                return;
            }
            for next in at..values.len() {
                values.swap(at, next);
                permute(at + 1, values, search, expected);
                values.swap(at, next);
            }
        }

        permute(0, &mut candidates.clone(), &search, &expected);
    }
}
