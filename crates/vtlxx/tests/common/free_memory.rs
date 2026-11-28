use std::path::Path;

// ld65's segment list includes BSS, unlike the generated binary's length.
pub fn assert_free(map: &Path, first: u32, last: u32) {
    let map = std::fs::read_to_string(map).unwrap();
    let segments = map.split("Segment list:").nth(1).expect("segment list");
    let segments = segments.split("Exports list").next().unwrap();
    let mut seen = false;
    for line in segments.lines() {
        let columns: Vec<_> = line.split_whitespace().collect();
        if columns.len() < 4 {
            continue;
        }
        let (Ok(start), Ok(end)) = (
            u32::from_str_radix(columns[1], 16),
            u32::from_str_radix(columns[2], 16),
        ) else {
            continue;
        };
        seen = true;
        assert!(end < first || start > last, "occupied test RAM: {line}");
    }
    assert!(seen, "no segments found in link map");
}
