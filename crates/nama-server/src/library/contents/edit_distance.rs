//! How far apart two words are, for correcting a search's typos.

/// The Levenshtein distance between `a` and `b`: how many characters must be inserted, deleted
/// or replaced to turn one into the other.
pub(super) fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    // The distances from what of `a` is read so far to each start of `b`, the empty one first.
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (read, a_char) in a.chars().enumerate() {
        let mut left = read + 1;
        let mut row = vec![left];
        let aboves = previous.iter().skip(1);
        for ((&b_char, &diagonal), &above) in b.iter().zip(&previous).zip(aboves) {
            left = (diagonal + usize::from(a_char != b_char))
                .min(above + 1)
                .min(left + 1);
            row.push(left);
        }
        previous = row;
    }
    previous.last().copied().unwrap_or_default()
}

#[cfg(test)]
#[path = "edit_distance_tests.rs"]
mod tests;
