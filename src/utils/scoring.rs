pub fn score(skill_overlap: i32, tag_match: i32, budget_fit: i32) -> i32 {
    3 * skill_overlap + 2 * tag_match + 1 * budget_fit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score() {
        assert_eq!(score(2, 1, 0), 3*2 + 2*1 + 1*0);
    }
}
