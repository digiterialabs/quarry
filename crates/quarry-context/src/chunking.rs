pub fn chunk_text(content: &str, max_chars: usize, overlap_chars: usize) -> Vec<String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = trimmed.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0usize;

    while start < chars.len() {
        let mut end = (start + max_chars).min(chars.len());
        if end < chars.len() {
            let mut cursor = end;
            while cursor > start + 64 {
                if chars[cursor - 1].is_whitespace() {
                    end = cursor;
                    break;
                }
                cursor -= 1;
            }
        }

        let chunk = chars[start..end]
            .iter()
            .collect::<String>()
            .trim()
            .to_string();
        if !chunk.is_empty() {
            chunks.push(chunk);
        }

        if end >= chars.len() {
            break;
        }

        let step_back = overlap_chars.min(end.saturating_sub(start + 1));
        start = end.saturating_sub(step_back);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::chunk_text;

    #[test]
    fn chunking_is_deterministic() {
        let text = "a ".repeat(800);
        let first = chunk_text(&text, 200, 40);
        let second = chunk_text(&text, 200, 40);
        assert_eq!(first, second);
        assert!(first.len() > 2);
    }

    #[test]
    fn empty_text_returns_no_chunks() {
        assert!(chunk_text("   ", 200, 40).is_empty());
    }
}
