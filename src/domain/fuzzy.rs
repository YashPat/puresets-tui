//! Forgiving name search: each word of a query matches when its letters
//! appear in order and close together, so "bnch" finds Bench Press.

/// How well a name matched a query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Letters skipped inside the matched stretches; fewer is closer.
    pub gaps: usize,
    /// Indices of the name's matched characters, ascending.
    pub positions: Vec<usize>,
}

/// Matches every whitespace-separated word of `query` against `name`,
/// ignoring case. An empty query matches everything with no positions.
pub fn find(name: &str, query: &str) -> Option<Match> {
    let name: Vec<char> = name.chars().collect();
    let mut gaps = 0;
    let mut positions = Vec::new();
    for term in query.split_whitespace() {
        let term: Vec<char> = term.chars().collect();
        let found = closest(&name, &term)?;
        gaps += span(&found) - term.len();
        positions.extend(found);
    }
    positions.sort_unstable();
    positions.dedup();
    Some(Match { gaps, positions })
}

/// The tightest run of `name` holding `term`'s letters in order, as the
/// matched indices. A term may skip no more letters than it has, so stray
/// letters far apart don't count as a match.
fn closest(name: &[char], term: &[char]) -> Option<Vec<usize>> {
    let first = term.first()?;
    let mut best: Option<Vec<usize>> = None;
    for start in (0..name.len()).filter(|&index| same(name[index], *first)) {
        let Some(found) = forward(name, term, start) else {
            // No later start can fit the term either.
            break;
        };
        if best.as_deref().is_none_or(|best| span(&found) < span(best)) {
            best = Some(found);
        }
    }
    best.filter(|found| span(found) - term.len() <= term.len())
}

/// How many letters of the name a match covers, first to last.
fn span(found: &[usize]) -> usize {
    match (found.first(), found.last()) {
        (Some(first), Some(last)) => last + 1 - first,
        _ => 0,
    }
}

/// Greedy in-order match of `term` in `name` from `start`.
fn forward(name: &[char], term: &[char], start: usize) -> Option<Vec<usize>> {
    let mut found = Vec::with_capacity(term.len());
    let mut index = start;
    for &letter in term {
        index += name.get(index..)?.iter().position(|&c| same(c, letter))?;
        found.push(index);
        index += 1;
    }
    Some(found)
}

fn same(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gaps(name: &str, query: &str) -> Option<usize> {
        find(name, query).map(|found| found.gaps)
    }

    #[test]
    fn words_match_in_order_ignoring_case() {
        assert_eq!(gaps("Incline Bench Press", "BENCH inc"), Some(0));
        assert_eq!(gaps("Bench Press", "bnch"), Some(1));
        assert_eq!(gaps("Romanian Deadlift", "dlft"), Some(1));
        assert_eq!(gaps("Bench Press", "hcneb"), None, "letters out of order");
        assert_eq!(gaps("Bench Press", ""), Some(0));
    }

    #[test]
    fn letters_spread_far_apart_do_not_match() {
        assert_eq!(gaps("Close-Grip Bench Press", "inc"), None);
        assert_eq!(gaps("Bench Press", "bs"), None);
    }

    #[test]
    fn positions_are_the_tightest_run() {
        let found = find("Bench Press", "es").unwrap();
        assert_eq!(found.positions, [8, 9], "the 'es' in Press, not B-e…s");
        let found = find("Incline Bench Press", "bench inc").unwrap();
        assert_eq!(found.positions, [0, 1, 2, 8, 9, 10, 11, 12]);
    }
}
