//! Recognition history shown in the main window: newest first, capped at
//! [`MAX_HISTORY`], and coalesced per region so continuous polling does not
//! flood the list. Only the matched species, region name, and local clock are
//! stored; raw OCR text can contain player names, so it is never kept.

pub const MAX_HISTORY: usize = 20;
const STORE_VERSION: &str = "pokemon-recognition-history-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: u16,
    pub region: String,
    pub clock: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct History {
    entries: Vec<HistoryEntry>,
}
impl History {
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&HistoryEntry> {
        self.entries.get(index)
    }
    /// Records a stable sighting as the newest entry and reports whether the
    /// list changed. A region that keeps reading the same species is ignored;
    /// a different species in that region adds a new entry. Entries beyond the
    /// cap are dropped from the oldest end.
    pub fn record(&mut self, id: u16, region: &str, clock: &str) -> bool {
        if !(1..=649).contains(&id) {
            return false;
        }
        let clock = sanitize_clock(clock);
        if clock.is_empty() {
            return false;
        }
        let region = sanitize_region(region);
        // Only the newest sighting of this region matters: alternating regions
        // must not append the same species every polling round.
        if self
            .entries
            .iter()
            .find(|entry| entry.region == region)
            .is_some_and(|entry| entry.id == id)
        {
            return false;
        }
        self.entries.insert(
            0,
            HistoryEntry {
                id,
                region,
                clock,
            },
        );
        self.entries.truncate(MAX_HISTORY);
        true
    }
    pub fn encode(&self) -> String {
        let mut text = format!("{STORE_VERSION}\n");
        for entry in &self.entries {
            text.push_str(&format!(
                "entry={},{},{}\n",
                entry.id, entry.clock, entry.region
            ));
        }
        text
    }
    /// Reads a stored list. History is auxiliary, so an unknown version or a
    /// damaged row only drops that data instead of failing startup.
    pub fn parse(text: &str) -> Self {
        let mut lines = text.lines();
        if lines.next() != Some(STORE_VERSION) {
            return Self::default();
        }
        let mut entries = Vec::new();
        for line in lines {
            let Some(value) = line.strip_prefix("entry=") else {
                continue;
            };
            let mut fields = value.splitn(3, ',');
            let (Some(id), Some(clock), Some(region)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let Ok(id) = id.parse::<u16>() else {
                continue;
            };
            if !(1..=649).contains(&id) {
                continue;
            }
            let clock = sanitize_clock(clock);
            if clock.is_empty() {
                continue;
            }
            entries.push(HistoryEntry {
                id,
                region: sanitize_region(region),
                clock,
            });
            if entries.len() == MAX_HISTORY {
                break;
            }
        }
        Self { entries }
    }
}

fn sanitize_region(region: &str) -> String {
    region
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(32)
        .collect::<String>()
        .trim()
        .to_owned()
}
fn sanitize_clock(clock: &str) -> String {
    let clock: String = clock
        .trim()
        .chars()
        .filter(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | ' '))
        .take(20)
        .collect();
    if clock.chars().any(|c| c.is_ascii_digit()) {
        clock.trim().to_owned()
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ids(history: &History) -> Vec<u16> {
        history.entries().iter().map(|entry| entry.id).collect()
    }
    #[test]
    fn newest_entry_is_first_and_the_list_is_capped() {
        let mut history = History::default();
        for id in 1..=25 {
            assert!(history.record(id, "选区 1", "11-09 10:00"));
        }
        assert_eq!(history.len(), MAX_HISTORY);
        assert_eq!(ids(&history)[0], 25);
        assert_eq!(ids(&history)[MAX_HISTORY - 1], 6);
    }
    #[test]
    fn continuous_polling_and_alternating_regions_do_not_flood_the_list() {
        let mut history = History::default();
        assert!(history.record(6, "左", "11-09 10:00"));
        assert!(!history.record(6, "左", "11-09 10:01"));
        assert!(history.record(6, "右", "11-09 10:02"));
        // The newest 左 entry is still Charizard, so the alternating doubles
        // round adds nothing.
        assert!(!history.record(6, "左", "11-09 10:03"));
        assert!(history.record(25, "左", "11-09 10:04"));
        assert!(history.record(6, "左", "11-09 10:05"));
        assert_eq!(ids(&history), vec![6, 25, 6, 6]);
    }
    #[test]
    fn store_round_trips_commas_and_control_characters() {
        let mut history = History::default();
        assert!(history.record(523, "群怪,左侧=1", "11-09 10:48"));
        assert!(history.record(6, "图鉴 No.编号", "11-09 10:49"));
        assert!(history.record(150, "坏\n名称", "11-09 10:50"));
        let restored = History::parse(&history.encode());
        assert_eq!(restored, history);
        assert_eq!(restored.get(0).unwrap().region, "坏 名称");
    }
    #[test]
    fn unknown_version_and_damaged_rows_are_ignored() {
        assert!(History::parse("").is_empty());
        assert!(History::parse("v0\nentry=6,11-09 10:00,x\n").is_empty());
        let text = concat!(
            "pokemon-recognition-history-v1\n",
            "entry=6,11-09 10:00,选区 1\n",
            "entry=0,11-09 10:00,越界\n",
            "entry=650,11-09 10:00,越界\n",
            "entry=abc,11-09 10:00,坏编号\n",
            "entry=25,11-09 10:01\n",
            "entry=150,   ,空时间\n",
            "garbage\n",
        );
        let history = History::parse(text);
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().id, 6);
    }
    #[test]
    fn a_stored_list_longer_than_the_cap_is_truncated() {
        let text = format!(
            "{STORE_VERSION}\n{}",
            "entry=6,11-09 10:00,选区 1\n".repeat(MAX_HISTORY + 5)
        );
        let history = History::parse(&text);
        assert_eq!(history.len(), MAX_HISTORY);
    }
    #[test]
    fn invalid_species_or_clock_is_rejected() {
        let mut history = History::default();
        assert!(!history.record(0, "选区 1", "11-09 10:00"));
        assert!(!history.record(650, "选区 1", "11-09 10:00"));
        assert!(!history.record(6, "选区 1", "  "));
        assert!(history.is_empty());
    }
}
