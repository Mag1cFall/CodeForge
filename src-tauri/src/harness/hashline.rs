const NIBBLE_STR: &[u8; 16] = b"ZPMQVRWSNKTXJBYH";
const XXH_PRIME32_1: u32 = 0x9E37_79B1;
const XXH_PRIME32_2: u32 = 0x85EB_CA77;
const XXH_PRIME32_3: u32 = 0xC2B2_AE3D;
const XXH_PRIME32_4: u32 = 0x27D4_EB2F;
const XXH_PRIME32_5: u32 = 0x1656_67B1;

fn xxh32_round(acc: u32, input: u32) -> u32 {
    let mixed = acc.wrapping_add(input.wrapping_mul(XXH_PRIME32_2));
    mixed.rotate_left(13).wrapping_mul(XXH_PRIME32_1)
}

fn xxh32_merge_round(acc: u32, value: u32) -> u32 {
    let merged = acc ^ xxh32_round(0, value);
    merged
        .wrapping_mul(XXH_PRIME32_1)
        .wrapping_add(XXH_PRIME32_4)
}

fn xxh32_avalanche(mut hash: u32) -> u32 {
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(XXH_PRIME32_2);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(XXH_PRIME32_3);
    hash ^= hash >> 16;
    hash
}

fn xxh32_with_seed(input: &str, seed: u32) -> u32 {
    let bytes = input.as_bytes();
    let mut index = 0usize;
    let mut hash;

    if bytes.len() >= 16 {
        let mut v1 = seed.wrapping_add(XXH_PRIME32_1).wrapping_add(XXH_PRIME32_2);
        let mut v2 = seed.wrapping_add(XXH_PRIME32_2);
        let mut v3 = seed;
        let mut v4 = seed.wrapping_sub(XXH_PRIME32_1);

        while index + 16 <= bytes.len() {
            let read_u32 = |offset: usize| {
                u32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ])
            };

            v1 = xxh32_round(v1, read_u32(index));
            v2 = xxh32_round(v2, read_u32(index + 4));
            v3 = xxh32_round(v3, read_u32(index + 8));
            v4 = xxh32_round(v4, read_u32(index + 12));
            index += 16;
        }

        hash = v1
            .rotate_left(1)
            .wrapping_add(v2.rotate_left(7))
            .wrapping_add(v3.rotate_left(12))
            .wrapping_add(v4.rotate_left(18));
        hash = xxh32_merge_round(hash, v1);
        hash = xxh32_merge_round(hash, v2);
        hash = xxh32_merge_round(hash, v3);
        hash = xxh32_merge_round(hash, v4);
    } else {
        hash = seed.wrapping_add(XXH_PRIME32_5);
    }

    hash = hash.wrapping_add(bytes.len() as u32);

    while index + 4 <= bytes.len() {
        let value = u32::from_le_bytes([
            bytes[index],
            bytes[index + 1],
            bytes[index + 2],
            bytes[index + 3],
        ]);
        hash = hash
            .wrapping_add(value.wrapping_mul(XXH_PRIME32_3))
            .rotate_left(17)
            .wrapping_mul(XXH_PRIME32_4);
        index += 4;
    }

    while index < bytes.len() {
        hash = hash
            .wrapping_add(u32::from(bytes[index]).wrapping_mul(XXH_PRIME32_5))
            .rotate_left(11)
            .wrapping_mul(XXH_PRIME32_1);
        index += 1;
    }

    xxh32_avalanche(hash)
}

fn normalize_line_content(line: &str) -> String {
    line.replace('\r', "").trim_end().to_string()
}

pub fn line_tag_with_number(line_number: usize, line: &str) -> String {
    let normalized = normalize_line_content(line);
    let seed = if normalized.chars().any(char::is_alphanumeric) {
        0u32
    } else {
        line_number as u32
    };
    let index = (xxh32_with_seed(&normalized, seed) % 256) as u8;
    let high = NIBBLE_STR[(index >> 4) as usize] as char;
    let low = NIBBLE_STR[(index & 0x0F) as usize] as char;
    format!("{high}{low}")
}

pub fn line_tag(line: &str) -> String {
    line_tag_with_number(1, line)
}

pub fn annotate_text(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let output = text
        .split('\n')
        .enumerate()
        .map(|(index, line)| {
            let line_number = index + 1;
            format!(
                "{}#{}|{}",
                line_number,
                line_tag_with_number(line_number, line),
                line
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    log_hashline_event(
        "annotate_text",
        serde_json::json!({
            "lineCount": text.split('\n').count(),
            "inputChars": text.chars().count(),
            "outputChars": output.chars().count(),
        }),
    );

    output
}

fn log_hashline_event(event: &str, payload: serde_json::Value) {
    eprintln!(
        "{}",
        serde_json::json!({
            "component": "harness.hashline",
            "event": event,
            "payload": payload,
        })
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotates_lines_with_stable_tags() {
        let annotated = annotate_text("alpha\nbeta");
        assert!(annotated.contains("1#"));
        assert!(annotated.contains("2#"));
        assert!(annotated.contains("|alpha"));
        assert_eq!(line_tag("alpha"), line_tag("alpha"));
    }

    #[test]
    fn keeps_trailing_empty_line_as_hashline_entry() {
        let annotated = annotate_text("x\ny\n");
        let lines = annotated.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 3);
        assert!(lines[2].starts_with("3#"));
        assert!(lines[2].ends_with('|'));
    }

    #[test]
    fn distinguishes_punctuation_lines_by_line_number() {
        let first = line_tag_with_number(1, "{}");
        let second = line_tag_with_number(2, "{}");
        assert_ne!(first, second);
    }
}
