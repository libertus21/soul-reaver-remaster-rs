//! Per-model actor block: family signature, 8x3 state trios and the 0x20-byte
//! behaviour-entry array. Read-only, counts and small ids only.
use crate::object::{self, ObjectError};
const ACTOR_SIGNATURE: u32 = 0xACE0_0065;
const PLAYER_SIGNATURE: u32 = 0x00BB_0630;
const STRIDE: usize = 0x20;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorKind { Monster, Player }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryArrayKind { Clean, Variant, None }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BehaviourEntry { pub id: u8 }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorBlock {
    pub kind: ActorKind,
    pub offset: usize,
    pub length: usize,
    pub trios: [[u8; 3]; 8],
    pub entries: Vec<BehaviourEntry>,
    pub entry_array: EntryArrayKind,
}
fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    bytes.get(at..at.checked_add(4)?).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn read_trios(block: &[u8]) -> Result<[[u8; 3]; 8], ObjectError> {
    let bytes = block.get(0x18..0x30).ok_or(ObjectError::HeaderTruncated)?;
    let mut trios = [[0u8; 3]; 8];
    for (row, trio) in trios.iter_mut().enumerate() {
        trio.copy_from_slice(&bytes[row * 3..row * 3 + 3]);
    }
    Ok(trios)
}
fn pointer_targets(block: &[u8], offset: usize, data_len: usize) -> Vec<usize> {
    let mut targets = Vec::new();
    for at in (0x30..).step_by(4).take(32) {
        let Some(value) = read_u32(block, at) else { break };
        let value = value as usize;
        if value >= offset && value < data_len {
            targets.push(value);
        }
    }
    targets.sort_unstable();
    targets.dedup();
    targets
}
fn run_ids(data: &[u8], start: usize, count: usize, sentinel: bool) -> Option<Vec<u8>> {
    let end = start.checked_add(count * STRIDE)?;
    if end > data.len() || data[start..start + 2] == [0x00, 0x10] {
        return None;
    }
    let mut ids = Vec::with_capacity(count);
    let mut good = 0;
    for index in 0..count {
        let at = start + index * STRIDE;
        let id = data[at + 0x1B];
        let ok = (1..=0x7F).contains(&id) && (!sentinel || data[at + 0x1C..at + 0x1E] == [0xFF, 0xFF]);
        if ok {
            good += 1;
        }
        ids.push(id);
    }
    (good >= (count - 1).max(1)).then_some(ids)
}
fn detect_entries(data: &[u8], targets: &[usize]) -> (EntryArrayKind, Vec<BehaviourEntry>) {
    let mut best: Option<(usize, usize, Vec<u8>)> = None;
    for &start in targets {
        let Some(end) = targets.iter().copied().find(|&target| target > start) else { continue };
        let distance = end - start;
        if distance % STRIDE != 0 {
            continue;
        }
        let count = distance / STRIDE;
        if !(1..=24).contains(&count) {
            continue;
        }
        let Some(ids) = run_ids(data, start, count, true) else { continue };
        if best.as_ref().is_none_or(|(known, at, _)| count > *known || (count == *known && start < *at)) {
            best = Some((count, start, ids));
        }
    }
    if let Some((_, _, ids)) = best {
        return (EntryArrayKind::Clean, ids.into_iter().map(|id| BehaviourEntry { id }).collect());
    }
    let variant = targets.iter().any(|&start| {
        let Some(end) = targets.iter().copied().find(|&target| target > start) else { return false };
        let distance = end - start;
        distance % STRIDE == 0 && (1..=24).contains(&(distance / STRIDE)) && run_ids(data, start, distance / STRIDE, false).is_some()
    });
    (if variant { EntryArrayKind::Variant } else { EntryArrayKind::None }, Vec::new())
}
/// `Ok(None)` for a non-actor or a zero/out-of-range pointer; a truncated
/// actor-signature block is a loud error.
pub fn parse_actor_block(bytes: &[u8]) -> Result<Option<ActorBlock>, ObjectError> {
    let Some((data, _)) = object::object_view(bytes) else { return Ok(None) };
    let Some(offset) = read_u32(data, 0x1C).map(|value| value as usize) else { return Ok(None) };
    if offset == 0 || offset >= data.len() {
        return Ok(None);
    }
    let kind = match read_u32(data, offset) {
        Some(ACTOR_SIGNATURE) => ActorKind::Monster,
        Some(PLAYER_SIGNATURE) => ActorKind::Player,
        _ => return Ok(None),
    };
    let block = &data[offset..];
    let trios = read_trios(block)?;
    let (entry_array, entries) = detect_entries(data, &pointer_targets(block, offset, data.len()));
    Ok(Some(ActorBlock { kind, offset, length: data.len() - offset, trios, entries, entry_array }))
}
pub fn ids_text(entries: &[BehaviourEntry]) -> String {
    if entries.is_empty() {
        return "-".to_string();
    }
    entries.iter().map(|entry| format!("{:02x}", entry.id)).collect::<Vec<_>>().join(",")
}
pub fn trios_text(trios: &[[u8; 3]; 8]) -> String {
    trios.iter().map(|row| format!("{},{},{}", row[0], row[1], row[2])).collect::<Vec<_>>().join("/")
}
pub fn entry_kind(kind: EntryArrayKind) -> &'static str {
    match kind { EntryArrayKind::Clean => "clean", EntryArrayKind::Variant => "variant", EntryArrayKind::None => "none" }
}
pub fn census_line(name: &str, index: Option<usize>, block: &ActorBlock) -> String {
    let index = index.map_or_else(|| "-".to_string(), |index| index.to_string());
    format!("actor {name} index={index} block={} entries={} ids={} trios={}", entry_kind(block.entry_array), block.entries.len(), ids_text(&block.entries), trios_text(&block.trios))
}
pub fn census_summary(actors: usize, entries: usize, distinct_ids: usize) -> String {
    format!("actors={actors} entries={entries} distinct_ids={distinct_ids}")
}
pub fn object_lines(block: Option<&ActorBlock>) -> Vec<String> {
    let Some(block) = block else { return vec!["  actor block: none".to_string()] };
    let trios = (0..8).map(|row| format!("row{row}={},{},{}", block.trios[row][0], block.trios[row][1], block.trios[row][2])).collect::<Vec<_>>().join(" ");
    vec![
        format!("  actor block: offset={} length={} entries={} ids={}", block.offset, block.length, block.entries.len(), ids_text(&block.entries)),
        format!("  actor trios: {trios}"),
        format!("  actor entry array: {}", entry_kind(block.entry_array)),
    ]
}#[cfg(test)]
pub(crate) fn synthetic_actor(name: &str, ids: &[u8], trios: [[u8; 3]; 8]) -> Vec<u8> {
    let mut bytes = crate::object::synthetic_model();
    let base = crate::level::data_base(1);
    bytes.resize(base + 0x400, 0);
    let data = &mut bytes[base..];
    data[0x4C..0x58].fill(0);
    data[0x4C..0x4C + name.len()].copy_from_slice(name.as_bytes());
    let block = 0x100usize;
    data[0x1C..0x20].copy_from_slice(&(block as u32).to_le_bytes());
    data[block..block + 4].copy_from_slice(&ACTOR_SIGNATURE.to_le_bytes());
    for (row, trio) in trios.iter().enumerate() {
        data[block + 0x18 + row * 3..block + 0x18 + row * 3 + 3].copy_from_slice(trio);
    }
    let array = block + 0x80;
    let end = array + ids.len() * STRIDE;
    data[block + 0x30..block + 0x34].copy_from_slice(&(array as u32).to_le_bytes());
    data[block + 0x34..block + 0x38].copy_from_slice(&(end as u32).to_le_bytes());
    for (index, &id) in ids.iter().enumerate() {
        let at = array + index * STRIDE;
        data[at + 0x1B] = id;
        data[at + 0x1C..at + 0x20].fill(0xFF);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trios() -> [[u8; 3]; 8] {
        [[19, 21, 20], [26, 32, 18], [5, 12, 7], [14, 18, 28], [35, 2, 11], [20, 3, 2], [10, 2, 66], [18, 8, 1]]
    }

    #[test]
    fn parses_actor_block_ids_trios_and_kind() {
        let parsed = parse_actor_block(&synthetic_actor("skinner", &[0x25, 0x26, 0x27], trios())).expect("parse").expect("actor");
        assert_eq!(parsed.kind, ActorKind::Monster);
        assert_eq!(parsed.entry_array, EntryArrayKind::Clean);
        assert_eq!(parsed.trios, trios());
        assert_eq!(parsed.entries.iter().map(|entry| entry.id).collect::<Vec<_>>(), vec![0x25, 0x26, 0x27]);
        assert_eq!((parsed.offset, parsed.length), (0x100, 0x300));
    }

    #[test]
    fn non_actor_and_out_of_range_pointers_are_none() {
        assert_eq!(parse_actor_block(&crate::object::synthetic_model()), Ok(None));
        let mut bytes = synthetic_actor("skinner", &[5], trios());
        let base = crate::level::data_base(1);
        bytes[base + 0x100..base + 0x104].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        assert_eq!(parse_actor_block(&bytes), Ok(None));
        let mut bytes = synthetic_actor("skinner", &[5], trios());
        bytes[base + 0x1C..base + 0x20].copy_from_slice(&(0x9000u32).to_le_bytes());
        assert_eq!(parse_actor_block(&bytes), Ok(None));
    }

    #[test]
    fn rejects_a_0x10_stride_sub_table() {
        let mut bytes = crate::object::synthetic_model();
        let base = crate::level::data_base(1);
        bytes.resize(base + 0x400, 0);
        let data = &mut bytes[base..];
        let block = 0x100usize;
        data[0x1C..0x20].copy_from_slice(&(block as u32).to_le_bytes());
        data[block..block + 4].copy_from_slice(&ACTOR_SIGNATURE.to_le_bytes());
        data[block + 0x18..block + 0x30].fill(1);
        let array = block + 0x80;
        data[block + 0x30..block + 0x34].copy_from_slice(&(array as u32).to_le_bytes());
        data[block + 0x34..block + 0x38].copy_from_slice(&((array + 0x10) as u32).to_le_bytes());
        data[array + 0x1B] = 5;
        data[array + 0x1C..array + 0x20].fill(0xFF);
        let parsed = parse_actor_block(&bytes).expect("parse").expect("actor");
        assert_ne!(parsed.entry_array, EntryArrayKind::Clean);
        assert!(parsed.entries.is_empty());
    }

    #[test]
    fn records_without_the_sentinel_are_variant() {
        let mut bytes = synthetic_actor("kain", &[5, 1], trios());
        let base = crate::level::data_base(1);
        for index in 0..2 {
            let at = base + 0x180 + index * STRIDE + 0x1C;
            bytes[at..at + 4].fill(0);
        }
        let parsed = parse_actor_block(&bytes).expect("parse").expect("actor");
        assert_eq!(parsed.entry_array, EntryArrayKind::Variant);
        assert!(parsed.entries.is_empty());
        assert_eq!(ids_text(&parsed.entries), "-");
    }

    #[test]
    fn truncated_actor_block_is_a_loud_error() {
        let mut bytes = synthetic_actor("skinner", &[5], trios());
        let base = crate::level::data_base(1);
        let block = 0x3F0usize;
        bytes[base + 0x1C..base + 0x20].copy_from_slice(&(block as u32).to_le_bytes());
        bytes[base + block..base + block + 4].copy_from_slice(&ACTOR_SIGNATURE.to_le_bytes());
        assert_eq!(parse_actor_block(&bytes), Err(ObjectError::HeaderTruncated));
    }

    #[test]
    fn formats_the_census_and_object_info_lines() {
        let block = parse_actor_block(&synthetic_actor("skinner", &[0x25, 0x26], trios())).expect("parse").expect("actor");
        let line = census_line("skinner", Some(129), &block);
        assert_eq!(
            line,
            "actor skinner index=129 block=clean entries=2 ids=25,26 trios=19,21,20/26,32,18/5,12,7/14,18,28/35,2,11/20,3,2/10,2,66/18,8,1"
        );
        assert_eq!(census_summary(1, 2, 2), "actors=1 entries=2 distinct_ids=2");
        assert_eq!(object_lines(None), vec!["  actor block: none"]);
        let lines = object_lines(Some(&block));
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "  actor block: offset=256 length=768 entries=2 ids=25,26");
        assert!(lines[1].starts_with("  actor trios: row0=19,21,20 "));
        assert_eq!(lines[2], "  actor entry array: clean");
    }
}

