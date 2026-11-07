use std::fmt;
pub mod bitmap;
pub mod regions;

pub const TYPE_KEYS: [&str; 17] = [
    "normal", "fighting", "flying", "poison", "ground", "rock", "bug", "ghost", "steel", "fire",
    "water", "grass", "electric", "psychic", "ice", "dragon", "dark",
];
pub const TYPE_NAMES: [&str; 17] = [
    "普通",
    "格斗",
    "飞行",
    "毒",
    "地面",
    "岩石",
    "虫",
    "幽灵",
    "钢",
    "火",
    "水",
    "草",
    "电",
    "超能力",
    "冰",
    "龙",
    "恶",
];

#[derive(Debug, Clone)]
pub struct Species {
    pub id: u16,
    pub name: String,
    pub types: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct TypeEffect {
    pub type_id: usize,
    pub multiplier: f64,
}

#[derive(Debug, Clone)]
pub struct HintCard {
    pub id: u16,
    pub name: String,
    pub types: Vec<usize>,
    pub weaknesses: Vec<TypeEffect>,
    pub resistances: Vec<TypeEffect>,
    pub immunities: Vec<TypeEffect>,
}

pub fn hint_card(species: &Species) -> HintCard {
    let mut card = HintCard {
        id: species.id,
        name: species.name.clone(),
        types: species.types.clone(),
        weaknesses: vec![],
        resistances: vec![],
        immunities: vec![],
    };
    for type_id in 0..17 {
        let multiplier = incoming_multiplier(type_id, species);
        let effect = TypeEffect {
            type_id,
            multiplier,
        };
        if multiplier > 1.0 {
            card.weaknesses.push(effect);
        } else if multiplier == 0.0 {
            card.immunities.push(effect);
        } else if multiplier < 1.0 {
            card.resistances.push(effect);
        }
    }
    card.weaknesses
        .sort_by(|a, b| b.multiplier.total_cmp(&a.multiplier));
    card.resistances
        .sort_by(|a, b| a.multiplier.total_cmp(&b.multiplier));
    card
}

pub fn species_data() -> Vec<Species> {
    include_str!("../data/species.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 4, "invalid bundled species row");
            Species {
                id: fields[0].parse().expect("species id"),
                name: fields[1].to_owned(),
                types: fields[2..]
                    .iter()
                    .filter(|key| !key.is_empty())
                    .map(|key| TYPE_KEYS.iter().position(|t| t == key).expect("type key"))
                    .collect(),
            }
        })
        .collect()
}

// Generation 2-5, attacker rows. Steel still resists Ghost and Dark; no Fairy.
pub fn effectiveness(attacker: usize, defender: usize) -> f64 {
    const STRONG: [&[usize]; 17] = [
        &[],
        &[0, 5, 8, 14, 16],
        &[1, 6, 11],
        &[11],
        &[3, 5, 8, 9, 12],
        &[2, 6, 9, 14],
        &[11, 13, 16],
        &[7, 13],
        &[5, 14],
        &[6, 8, 11, 14],
        &[4, 5, 9],
        &[4, 5, 10],
        &[2, 10],
        &[1, 3],
        &[2, 4, 11, 15],
        &[15],
        &[7, 13],
    ];
    const RESIST: [&[usize]; 17] = [
        &[5, 8],
        &[2, 3, 6, 13],
        &[5, 8, 12],
        &[3, 4, 5, 7],
        &[6, 11],
        &[1, 4, 8],
        &[1, 2, 3, 7, 8, 9],
        &[8, 16],
        &[8, 9, 10, 12],
        &[5, 9, 10, 15],
        &[10, 11, 15],
        &[2, 3, 6, 8, 9, 11, 15],
        &[11, 12, 15],
        &[8, 13],
        &[8, 9, 10, 14],
        &[8],
        &[1, 8, 16],
    ];
    const IMMUNE: [&[usize]; 17] = [
        &[7],
        &[7],
        &[],
        &[8],
        &[2],
        &[],
        &[],
        &[0],
        &[],
        &[],
        &[],
        &[],
        &[4],
        &[16],
        &[],
        &[],
        &[],
    ];
    assert!(attacker < 17 && defender < 17);
    if IMMUNE[attacker].contains(&defender) {
        0.0
    } else if STRONG[attacker].contains(&defender) {
        2.0
    } else if RESIST[attacker].contains(&defender) {
        0.5
    } else {
        1.0
    }
}

pub fn incoming_multiplier(attack: usize, species: &Species) -> f64 {
    species
        .types
        .iter()
        .map(|&defense| effectiveness(attack, defense))
        .product()
}

pub fn hint(species: &Species) -> String {
    let mut groups: Vec<Vec<&str>> = vec![vec![]; 5];
    for (attack, name) in TYPE_NAMES.iter().enumerate() {
        let value = incoming_multiplier(attack, species);
        let group = if value == 4.0 {
            Some(0)
        } else if value == 2.0 {
            Some(1)
        } else if value == 0.5 {
            Some(2)
        } else if value == 0.25 {
            Some(3)
        } else if value == 0.0 {
            Some(4)
        } else {
            None
        };
        if let Some(i) = group {
            groups[i].push(name);
        }
    }
    let mut lines = vec![format!(
        "#{:03} {} · {}",
        species.id,
        species.name,
        species
            .types
            .iter()
            .map(|&t| TYPE_NAMES[t])
            .collect::<Vec<_>>()
            .join(" / ")
    )];
    let mut weak = Vec::new();
    for (i, prefix) in [(0, "4倍："), (1, "2倍：")] {
        if !groups[i].is_empty() {
            weak.push(format!("{}{}", prefix, groups[i].join("、")));
        }
    }
    lines.push(format!(
        "弱点  {}",
        if weak.is_empty() {
            "无".to_owned()
        } else {
            weak.join("；")
        }
    ));
    let mut resist = Vec::new();
    for (i, prefix) in [(2, "1/2："), (3, "1/4：")] {
        if !groups[i].is_empty() {
            resist.push(format!("{}{}", prefix, groups[i].join("、")));
        }
    }
    lines.push(format!(
        "抗性  {}",
        if resist.is_empty() {
            "无".to_owned()
        } else {
            resist.join("；")
        }
    ));
    lines.push(format!(
        "免疫  {}",
        if groups[4].is_empty() {
            "无".to_owned()
        } else {
            groups[4].join("、")
        }
    ));
    lines.join("\r\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameSuggestion {
    pub id: u16,
    pub observed: String,
}

pub fn correction_key(text: &str) -> Option<String> {
    let text = normalized(text);
    if text.contains("no") || text.contains('#') {
        return None;
    }
    let name: String = text
        .split("lv")
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
        .collect();
    (3..=6).contains(&name.chars().count()).then_some(name)
}

// A single unique substitution is a suggestion, never an automatic match.
pub fn suggest_ocr(text: &str, data: &[Species]) -> Option<NameSuggestion> {
    if match_ocr(text, data) != MatchResult::Unknown {
        return None;
    }
    let key = correction_key(text)?;
    let chars: Vec<_> = key.chars().collect();
    let mut candidates = data.iter().filter(|s| {
        let name: Vec<_> = s.name.chars().collect();
        name.len() == chars.len() && name.iter().zip(&chars).filter(|(a, b)| a != b).count() == 1
    });
    let first = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(NameSuggestion {
        id: first.id,
        observed: key,
    })
}

pub struct ReadDecision {
    pub raw: String,
    pub result: MatchResult,
    pub corrected: bool,
    pub suggestion: Option<NameSuggestion>,
}

pub fn resolve_reads(
    reads: &[String],
    data: &[Species],
    confirmed: &std::collections::HashMap<String, u16>,
) -> ReadDecision {
    let mut found = Vec::new();
    let mut suggestions = Vec::new();
    for raw in reads {
        let mut result = match_ocr(raw, data);
        let mut corrected = false;
        if result == MatchResult::Unknown {
            if let Some(id) = correction_key(raw)
                .and_then(|key| confirmed.get(&key))
                .copied()
            {
                if data.iter().any(|s| s.id == id) {
                    result = MatchResult::Found(id);
                    corrected = true;
                }
            }
        }
        match result {
            MatchResult::Conflict | MatchResult::Ambiguous => {
                return ReadDecision {
                    raw: raw.clone(),
                    result,
                    corrected: false,
                    suggestion: None,
                }
            }
            MatchResult::Found(id) => found.push((id, raw, corrected)),
            MatchResult::Unknown => {
                if let Some(s) = suggest_ocr(raw, data) {
                    suggestions.push((raw, s));
                }
            }
        }
    }
    if let Some(&(id, raw, corrected)) = found.first() {
        let result = if found.iter().all(|f| f.0 == id) {
            MatchResult::Found(id)
        } else {
            MatchResult::Conflict
        };
        return ReadDecision {
            raw: raw.clone(),
            result,
            corrected,
            suggestion: None,
        };
    }
    let raw = reads.first().cloned().unwrap_or_default();
    if let Some((candidate_raw, suggestion)) = suggestions.first() {
        if suggestions.iter().any(|(_, s)| s.id != suggestion.id) {
            return ReadDecision {
                raw,
                result: MatchResult::Ambiguous,
                corrected: false,
                suggestion: None,
            };
        }
        return ReadDecision {
            raw: (*candidate_raw).clone(),
            result: MatchResult::Unknown,
            corrected: false,
            suggestion: Some(suggestion.clone()),
        };
    }
    ReadDecision {
        raw,
        result: MatchResult::Unknown,
        corrected: false,
        suggestion: None,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatchResult {
    Found(u16),
    Unknown,
    Ambiguous,
    Conflict,
}

fn compact(text: &str) -> String {
    normalized(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn normalized(text: &str) -> String {
    text.chars()
        .map(|c| {
            if ('\u{ff01}'..='\u{ff5e}').contains(&c) {
                char::from_u32(c as u32 - 0xfee0).unwrap()
            } else {
                c
            }
        })
        .flat_map(char::to_lowercase)
        .collect()
}

// Deliberately require an exact name. Guessing a similar name can give wrong immunities.
pub fn match_ocr(text: &str, data: &[Species]) -> MatchResult {
    let compact = compact(text);
    let ids = match explicit_numbers(&normalized(text)) {
        Ok(ids) => ids,
        Err(()) => return MatchResult::Unknown,
    };
    let aliases: Vec<_> = data.iter().map(|s| (s, name_aliases(s))).collect();
    let mut found: Vec<_> = aliases
        .iter()
        .filter(|(_, names)| names.iter().any(|n| contains_name(&compact, n)))
        .map(|(s, _)| *s)
        .collect();
    // Prefer full species names over contained shorter names (e.g. 多边兽2 / 多边兽).
    found.retain(|s| {
        !aliases.iter().any(|(other, names)| {
            other.id != s.id
                && names
                    .iter()
                    .any(|name| name.contains(&compact_name(s)) && contains_name(&compact, name))
        })
    });
    if ids.len() > 1 {
        return MatchResult::Ambiguous;
    }
    if let Some(&id) = ids.first() {
        if !data.iter().any(|s| s.id == id) {
            return MatchResult::Unknown;
        }
        if found.len() > 1 {
            return MatchResult::Ambiguous;
        }
        if found.first().is_some_and(|s| s.id != id) {
            return MatchResult::Conflict;
        }
        return MatchResult::Found(id);
    }
    match found.as_slice() {
        [one] => MatchResult::Found(one.id),
        [] => MatchResult::Unknown,
        _ => MatchResult::Ambiguous,
    }
}

fn explicit_numbers(text: &str) -> Result<Vec<u16>, ()> {
    let chars: Vec<_> = text.chars().collect();
    let mut ids = Vec::new();
    for i in 0..chars.len() {
        if i > 0 && chars[i - 1].is_ascii_alphanumeric() {
            continue;
        }
        let start = if chars[i] == '#' {
            let mut p = i + 1;
            while chars.get(p).is_some_and(|c| c.is_whitespace()) {
                p += 1;
            }
            Some(p)
        } else if chars[i] == 'n' && chars.get(i + 1) == Some(&'o') {
            let mut p = i + 2;
            while chars.get(p).is_some_and(|c| c.is_whitespace()) {
                p += 1;
            }
            if matches!(chars.get(p), Some('.' | ':')) {
                p += 1;
            }
            while chars.get(p).is_some_and(|c| c.is_whitespace()) {
                p += 1;
            }
            if chars.get(p).is_some_and(char::is_ascii_digit) {
                Some(p)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(start) = start {
            let segment: String = chars[start..]
                .iter()
                .take_while(|c| c.is_ascii_digit() || c.is_whitespace())
                .collect();
            let digits: String = segment.chars().filter(char::is_ascii_digit).collect();
            if digits.is_empty() {
                continue;
            }
            let tail: String = chars[start + segment.chars().count()..].iter().collect();
            if tail.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && !tail.starts_with("lv")
                && !tail.starts_with("no")
            {
                return Err(());
            }
            let id = digits.parse::<u16>().map_err(|_| ())?;
            if !(1..=649).contains(&id) {
                return Err(());
            }
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    Ok(ids)
}

fn contains_name(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(offset, _)| {
        let tail = &text[offset + name.len()..];
        // A misread suffix must not turn 多边兽3 into the shorter 多边兽.
        !tail
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
            || tail.starts_with("lv")
    })
}

fn compact_name(species: &Species) -> String {
    compact(&species.name)
}
fn name_aliases(species: &Species) -> Vec<String> {
    let mut names = vec![compact_name(species)];
    match species.id {
        233 => names.extend(["多边兽2", "多边兽ii", "多边兽Ⅱ"].map(compact)),
        474 => names.extend(["多边兽z", "多边兽Ｚ"].map(compact)),
        _ => {}
    }
    names
}

pub fn lookup_query(text: &str, data: &[Species]) -> MatchResult {
    if let Ok(id) = text.trim().trim_start_matches('#').parse::<u16>() {
        if data.iter().any(|s| s.id == id) {
            return MatchResult::Found(id);
        }
        return MatchResult::Unknown;
    }
    match_ocr(text, data)
}

#[derive(Default)]
pub struct Tracker {
    pending: Option<u16>,
    streak: u8,
}
impl Tracker {
    pub fn update(&mut self, result: &MatchResult) -> Option<u16> {
        if let MatchResult::Found(id) = result {
            if self.pending == Some(*id) {
                self.streak = self.streak.saturating_add(1);
            } else {
                self.pending = Some(*id);
                self.streak = 1;
            }
            if self.streak >= 2 {
                return Some(*id);
            }
        } else {
            self.pending = None;
            self.streak = 0;
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roi {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl Roi {
    pub fn validate(self, width: i32, height: i32) -> Result<Self, String> {
        if self.x < 0
            || self.y < 0
            || self.width < 8
            || self.height < 8
            || self.width > 2000
            || self.height > 1000
            || self.x as i64 + self.width as i64 > width as i64
            || self.y as i64 + self.height as i64 > height as i64
        {
            return Err("选择区域必须在屏幕范围内，且至少 8×8 像素".to_owned());
        }
        Ok(self)
    }
}
impl fmt::Display for Roi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{},{}", self.x, self.y, self.width, self.height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn species(id: u16) -> Species {
        species_data().into_iter().find(|s| s.id == id).unwrap()
    }
    #[test]
    fn all_species_are_valid_and_unique() {
        let data = species_data();
        assert_eq!(data.len(), 649);
        for id in 1..=649 {
            assert_eq!(data.iter().filter(|s| s.id == id).count(), 1);
        }
        assert!(data
            .iter()
            .all(|s| (1..=2).contains(&s.types.len()) && s.types.iter().all(|&t| t < 17)));
    }
    #[test]
    fn known_screenshot_enemies() {
        let data = species_data();
        assert_eq!(
            match_ocr("雷 电 斑 马 Lv. 29 ♀", &data),
            MatchResult::Found(523)
        );
        assert_eq!(match_ocr("奇诺栗鼠Lv.43", &data), MatchResult::Found(573));
        assert_eq!(match_ocr("Minipete Lv.30", &data), MatchResult::Unknown);
        assert_eq!(match_ocr("Lv.29", &data), MatchResult::Unknown);
    }
    #[test]
    fn explicit_dex_numbers_and_name_cross_checks() {
        let data = species_data();
        for text in [
            "No.573",
            "No. 573 奇诺栗鼠",
            "no:0573",
            "Ｎｏ．５７３",
            "#573",
            "No.573 Lv.29",
            "No. 5 73",
        ] {
            assert_eq!(match_ocr(text, &data), MatchResult::Found(573), "{text}");
        }
        assert_eq!(match_ocr("No.573 雷电斑马", &data), MatchResult::Conflict);
        assert_eq!(match_ocr("No.573 No.523", &data), MatchResult::Ambiguous);
        assert_eq!(match_ocr("Lv.573", &data), MatchResult::Unknown);
        assert_eq!(match_ocr("573", &data), MatchResult::Unknown);
        assert_eq!(match_ocr("No.999 奇诺栗鼠", &data), MatchResult::Unknown);
        assert_eq!(match_ocr("No.0", &data), MatchResult::Unknown);
        assert_eq!(match_ocr("No.57B", &data), MatchResult::Unknown);
    }
    #[test]
    fn matching_rejects_multiple_and_misread_names() {
        let data = species_data();
        assert_eq!(match_ocr("皮卡丘 小火龙", &data), MatchResult::Ambiguous);
        assert_eq!(match_ocr("雷电班马", &data), MatchResult::Unknown);
        assert_eq!(lookup_query("573", &data), MatchResult::Found(573));
        assert_eq!(match_ocr("多边兽2 Lv.20", &data), MatchResult::Found(233));
        assert_eq!(match_ocr("多边兽3 Lv.20", &data), MatchResult::Unknown);
    }
    #[test]
    fn dual_types_and_immunities() {
        assert_eq!(incoming_multiplier(5, &species(6)), 4.0); // Rock -> Charizard
        assert_eq!(incoming_multiplier(4, &species(6)), 0.0); // Ground -> Charizard
        assert_eq!(incoming_multiplier(11, &species(6)), 0.25); // Grass -> Charizard
        assert_eq!(incoming_multiplier(1, &species(573)), 2.0);
        assert_eq!(incoming_multiplier(7, &species(573)), 0.0);
        assert_eq!(incoming_multiplier(4, &species(523)), 2.0);
        assert!(hint(&species(523)).contains("2倍：地面"));
    }
    #[test]
    fn generation_five_steel_resists_ghost_and_dark() {
        assert_eq!(effectiveness(7, 8), 0.5);
        assert_eq!(effectiveness(16, 8), 0.5);
        assert!(TYPE_KEYS.iter().all(|s| *s != "fairy"));
    }
    #[test]
    fn changing_or_losing_enemy_clears_old_result_immediately() {
        let mut t = Tracker::default();
        assert_eq!(t.update(&MatchResult::Found(523)), None);
        assert_eq!(t.update(&MatchResult::Found(523)), Some(523));
        assert_eq!(t.update(&MatchResult::Found(573)), None);
        assert_eq!(t.update(&MatchResult::Found(573)), Some(573));
        assert_eq!(t.update(&MatchResult::Unknown), None);
        assert_eq!(t.update(&MatchResult::Found(573)), None);
    }
    #[test]
    fn invalid_crop_coordinates_are_rejected() {
        assert!(Roi {
            x: -1,
            y: 0,
            width: 100,
            height: 20
        }
        .validate(1920, 1080)
        .is_err());
        assert!(Roi {
            x: 1900,
            y: 0,
            width: 100,
            height: 20
        }
        .validate(1920, 1080)
        .is_err());
        assert!(Roi {
            x: 90,
            y: 50,
            width: 270,
            height: 36
        }
        .validate(1920, 1080)
        .is_ok());
    }
    #[test]
    fn near_name_is_only_a_candidate_until_confirmed() {
        let data = species_data();
        let mut aliases = std::collections::HashMap::new();
        let reads = vec!["君主尼".to_owned()];
        let decision = resolve_reads(&reads, &data, &aliases);
        assert_eq!(decision.result, MatchResult::Unknown);
        assert_eq!(
            decision.suggestion.unwrap(),
            NameSuggestion {
                id: 497,
                observed: "君主尼".to_owned()
            }
        );
        aliases.insert("君主尼".to_owned(), 497);
        let decision = resolve_reads(&reads, &data, &aliases);
        assert_eq!(decision.result, MatchResult::Found(497));
        assert!(decision.corrected);
        assert!(suggest_ocr("No.49B", &data).is_none());
        assert!(suggest_ocr("蛇", &data).is_none());
        let ambiguous = vec![
            Species {
                id: 1,
                name: "君主蛇".to_owned(),
                types: vec![11],
            },
            Species {
                id: 2,
                name: "君主猴".to_owned(),
                types: vec![11],
            },
        ];
        assert!(suggest_ocr("君主尼", &ambiguous).is_none());
    }
    #[test]
    fn alternative_read_can_rescue_but_cannot_override_conflicts() {
        let data = species_data();
        let aliases = std::collections::HashMap::new();
        assert_eq!(
            resolve_reads(&["君主尼".to_owned(), "君主蛇".to_owned()], &data, &aliases).result,
            MatchResult::Found(497)
        );
        assert_eq!(
            resolve_reads(&["君主蛇".to_owned(), "皮卡丘".to_owned()], &data, &aliases).result,
            MatchResult::Conflict
        );
        assert_eq!(
            resolve_reads(
                &["No.573 雷电斑马".to_owned(), "奇诺栗鼠".to_owned()],
                &data,
                &aliases
            )
            .result,
            MatchResult::Conflict
        );
    }
    #[test]
    fn card_separates_type_effects_and_omits_old_caption() {
        let card = hint_card(&species(6));
        assert_eq!(card.id, 6);
        assert!(card
            .weaknesses
            .iter()
            .any(|e| e.type_id == 5 && e.multiplier == 4.0));
        assert!(card
            .resistances
            .iter()
            .any(|e| e.type_id == 11 && e.multiplier == 0.25));
        assert!(card
            .immunities
            .iter()
            .any(|e| e.type_id == 4 && e.multiplier == 0.0));
        assert!(!hint(&species(6)).contains("基础形态属性"));
    }
}
