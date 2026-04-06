fn fnv1a64(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn line_tag(line: &str) -> String {
    format!("{:04X}", fnv1a64(line) & 0xFFFF)
}

pub fn annotate_text(text: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(index, line)| format!("{}#{}| {}", index + 1, line_tag(line), line))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotates_lines_with_stable_tags() {
        let annotated = annotate_text("alpha\nbeta");
        assert!(annotated.contains("1#"));
        assert!(annotated.contains("2#"));
        assert!(annotated.contains("| alpha"));
        assert_eq!(line_tag("alpha"), line_tag("alpha"));
    }
}
