/// Compact content for token-efficient storage.
///
/// - Strips trailing whitespace from each line
/// - Collapses runs of 3+ blank lines to 2 (one visual blank line)
/// - Trims leading/trailing blank lines
/// - Preserves indentation inside fenced code blocks
pub fn compact_content(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut consecutive_blanks: usize = 0;

    for line in s.lines() {
        let trimmed = line.trim_end();

        if trimmed.is_empty() {
            consecutive_blanks += 1;
            if consecutive_blanks <= 2 {
                result.push('\n');
            }
        } else {
            consecutive_blanks = 0;
            result.push_str(trimmed);
            result.push('\n');
        }
    }

    // Trim leading and trailing blank lines
    let trimmed = result.trim_matches('\n');
    if trimmed.is_empty() {
        String::new()
    } else {
        let mut out = trimmed.to_string();
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_trailing_whitespace() {
        let input = "hello   \nworld  \n";
        assert_eq!(compact_content(input), "hello\nworld\n");
    }

    #[test]
    fn collapses_excessive_blank_lines() {
        let input = "a\n\n\n\n\nb";
        // 4 blank lines between a and b → collapsed to 2
        assert_eq!(compact_content(input), "a\n\n\nb\n");
    }

    #[test]
    fn preserves_single_blank_line() {
        let input = "a\n\nb\n";
        assert_eq!(compact_content(input), "a\n\nb\n");
    }

    #[test]
    fn preserves_double_blank_line() {
        let input = "a\n\n\nb\n";
        assert_eq!(compact_content(input), "a\n\n\nb\n");
    }

    #[test]
    fn trims_leading_and_trailing_blanks() {
        let input = "\n\n\nhello\n\n\n";
        assert_eq!(compact_content(input), "hello\n");
    }

    #[test]
    fn preserves_code_block_indentation() {
        let input = "text\n\n```python\ndef foo():\n    return 42\n```\n";
        assert_eq!(
            compact_content(input),
            "text\n\n```python\ndef foo():\n    return 42\n```\n"
        );
    }

    #[test]
    fn empty_input() {
        assert_eq!(compact_content(""), "");
        assert_eq!(compact_content("\n\n\n"), "");
    }

    #[test]
    fn preserves_list_indentation() {
        let input = "- item 1\n  - nested\n  - nested 2\n- item 2\n";
        assert_eq!(compact_content(input), input);
    }
}
