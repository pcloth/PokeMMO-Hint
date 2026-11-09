pub mod commandline;
use std::fmt;
pub mod bitmap;
pub mod engines;
pub mod history;
pub mod model;
pub mod regions;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Language {
    #[default]
    Zh,
    En,
    Ja,
}
impl Language {
    pub const ALL: [Language; 3] = [Self::Zh, Self::En, Self::Ja];
    pub const fn index(self) -> usize {
        match self {
            Self::Zh => 0,
            Self::En => 1,
            Self::Ja => 2,
        }
    }
    pub const fn code(self) -> &'static str {
        match self {
            Self::Zh => "zh",
            Self::En => "en",
            Self::Ja => "ja",
        }
    }
    /// Name written in that language, for the language selector itself.
    pub const fn native_name(self) -> &'static str {
        match self {
            Self::Zh => "简体中文",
            Self::En => "English",
            Self::Ja => "日本語",
        }
    }
    /// Language tag passed to Windows.Media.Ocr.
    pub const fn windows_tag(self) -> &'static str {
        match self {
            Self::Zh => "zh-Hans",
            Self::En => "en-US",
            Self::Ja => "ja-JP",
        }
    }
    pub fn parse(code: &str) -> Option<Self> {
        match code.trim().to_ascii_lowercase().as_str() {
            "zh" | "zh-cn" | "zh-hans" | "cn" => Some(Self::Zh),
            "en" | "en-us" | "en-gb" => Some(Self::En),
            "ja" | "ja-jp" | "jp" => Some(Self::Ja),
            _ => None,
        }
    }
}

pub const TYPE_KEYS: [&str; 17] = [
    "normal", "fighting", "flying", "poison", "ground", "rock", "bug", "ghost", "steel", "fire",
    "water", "grass", "electric", "psychic", "ice", "dragon", "dark",
];
pub const TYPE_NAMES_ZH: [&str; 17] = [
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
pub const TYPE_NAMES_EN: [&str; 17] = [
    "Normal",
    "Fighting",
    "Flying",
    "Poison",
    "Ground",
    "Rock",
    "Bug",
    "Ghost",
    "Steel",
    "Fire",
    "Water",
    "Grass",
    "Electric",
    "Psychic",
    "Ice",
    "Dragon",
    "Dark",
];
pub const TYPE_NAMES_JA: [&str; 17] = [
    "ノーマル",
    "かくとう",
    "ひこう",
    "どく",
    "じめん",
    "いわ",
    "むし",
    "ゴースト",
    "はがね",
    "ほのお",
    "みず",
    "くさ",
    "でんき",
    "エスパー",
    "こおり",
    "ドラゴン",
    "あく",
];
pub fn type_names(lang: Language) -> &'static [&'static str; 17] {
    match lang {
        Language::Zh => &TYPE_NAMES_ZH,
        Language::En => &TYPE_NAMES_EN,
        Language::Ja => &TYPE_NAMES_JA,
    }
}
pub fn type_name(lang: Language, type_id: usize) -> &'static str {
    type_names(lang)[type_id]
}

#[derive(Debug, Clone)]
pub struct Species {
    pub id: u16,
    /// Invariant: Chinese, English and Japanese names are always present.
    pub names: [String; 3],
    pub types: Vec<usize>,
}
impl Species {
    pub fn name(&self, lang: Language) -> &str {
        &self.names[lang.index()]
    }
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
    pub neutral: Vec<TypeEffect>,
    pub resistances: Vec<TypeEffect>,
    pub immunities: Vec<TypeEffect>,
}

pub fn hint_card(species: &Species, lang: Language) -> HintCard {
    let mut card = HintCard {
        id: species.id,
        name: species.name(lang).to_owned(),
        types: species.types.clone(),
        weaknesses: vec![],
        neutral: vec![],
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
        } else {
            card.neutral.push(effect);
        }
    }
    card.weaknesses
        .sort_by(|a, b| b.multiplier.total_cmp(&a.multiplier));
    card.resistances
        .sort_by(|a, b| a.multiplier.total_cmp(&b.multiplier));
    card
}

pub fn species_data() -> Vec<Species> {
    let names = species_names();
    include_str!("../data/species.tsv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 4, "invalid bundled species row");
            let id: u16 = fields[0].parse().expect("species id");
            let chinese = fields[1].to_owned();
            let (english, japanese) = names
                .get(usize::from(id))
                .and_then(|names| names.clone())
                .unwrap_or_else(|| (chinese.clone(), chinese.clone()));
            Species {
                id,
                names: [chinese, english, japanese],
                types: fields[2..]
                    .iter()
                    .filter(|key| !key.is_empty())
                    .map(|key| TYPE_KEYS.iter().position(|t| t == key).expect("type key"))
                    .collect(),
            }
        })
        .collect()
}
fn species_names() -> Vec<Option<(String, String)>> {
    let mut names = vec![None; 650];
    for line in include_str!("../data/species_names.tsv").lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 3, "invalid bundled species name row");
        let id: usize = fields[0].parse().expect("species name id");
        assert!(id < names.len(), "species name id out of range");
        assert!(names[id].is_none(), "duplicate species name row");
        names[id] = Some((fields[1].to_owned(), fields[2].to_owned()));
    }
    names
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

pub fn hint(species: &Species, lang: Language) -> String {
    let names = type_names(lang);
    let mut groups: Vec<Vec<&str>> = vec![vec![]; 6];
    for (attack, name) in names.iter().enumerate() {
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
            Some(5)
        };
        if let Some(i) = group {
            groups[i].push(name);
        }
    }
    let (weak_label, neutral_label, resist_label, immune_label, none) = match lang {
        Language::Zh => ("弱点", "普通伤害（1倍）", "抗性", "免疫", "无"),
        Language::En => ("Weaknesses", "Neutral (1x)", "Resists", "Immunities", "none"),
        Language::Ja => ("弱点", "等倍", "耐性", "無効", "なし"),
    };
    let (strong4, strong2, resist2, resist4) = match lang {
        Language::En => ("4x: ", "2x: ", "1/2x: ", "1/4x: "),
        Language::Zh | Language::Ja => ("4倍：", "2倍：", "1/2：", "1/4："),
    };
    let (and_sep, group_sep) = match lang {
        Language::En => (", ", "; "),
        Language::Zh | Language::Ja => ("、", "；"),
    };
    let mut lines = vec![format!(
        "#{:03} {} · {}",
        species.id,
        species.name(lang),
        species
            .types
            .iter()
            .map(|&t| names[t])
            .collect::<Vec<_>>()
            .join(" / ")
    )];
    let mut weak = Vec::new();
    for (i, prefix) in [(0, strong4), (1, strong2)] {
        if !groups[i].is_empty() {
            weak.push(format!("{}{}", prefix, groups[i].join(and_sep)));
        }
    }
    lines.push(format!(
        "{}  {}",
        weak_label,
        if weak.is_empty() {
            none.to_owned()
        } else {
            weak.join(group_sep)
        }
    ));
    lines.push(format!(
        "{}  {}",
        neutral_label,
        if groups[5].is_empty() {
            none.to_owned()
        } else {
            groups[5].join(and_sep)
        }
    ));
    let mut resist = Vec::new();
    for (i, prefix) in [(2, resist2), (3, resist4)] {
        if !groups[i].is_empty() {
            resist.push(format!("{}{}", prefix, groups[i].join(and_sep)));
        }
    }
    lines.push(format!(
        "{}  {}",
        resist_label,
        if resist.is_empty() {
            none.to_owned()
        } else {
            resist.join(group_sep)
        }
    ));
    lines.push(format!(
        "{}  {}",
        immune_label,
        if groups[4].is_empty() {
            none.to_owned()
        } else {
            groups[4].join(and_sep)
        }
    ));
    lines.join("\r\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameSuggestion {
    pub id: u16,
    pub observed: String,
}

pub fn correction_key(text: &str, lang: Language) -> Option<String> {
    let text = normalized(text);
    if text.contains("no") || text.contains('#') {
        return None;
    }
    let before_level = text.split("lv").next().unwrap_or("");
    let name: String = match lang {
        Language::Zh => before_level
            .chars()
            .filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c))
            .collect(),
        Language::En => before_level
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect(),
        Language::Ja => before_level.chars().filter(|c| is_kana(*c)).collect(),
    };
    let length = name.chars().count();
    let range = match lang {
        Language::Zh => 3..=6,
        Language::En => 4..=20,
        Language::Ja => 3..=12,
    };
    range.contains(&length).then_some(name)
}
fn is_kana(c: char) -> bool {
    ('\u{3040}'..='\u{30ff}').contains(&c)
}

// A single unique substitution is a suggestion, never an automatic match.
pub fn suggest_ocr(text: &str, lang: Language, data: &[Species]) -> Option<NameSuggestion> {
    if match_ocr(text, lang, data) != MatchResult::Unknown {
        return None;
    }
    let key = correction_key(text, lang)?;
    let chars: Vec<_> = key.chars().collect();
    let mut candidates = data.iter().filter(|s| {
        let name: Vec<_> = normalized(s.name(lang)).chars().collect();
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

/// Confirmed misreadings are scoped to one game language, so a key remembered
/// for a Chinese read never resolves an English or Japanese read.
pub fn confirmed_key(lang: Language, text: &str) -> String {
    format!("{}:{text}", lang.code())
}

pub fn resolve_reads(
    reads: &[String],
    lang: Language,
    data: &[Species],
    confirmed: &std::collections::HashMap<String, u16>,
) -> ReadDecision {
    let mut found = Vec::new();
    let mut suggestions = Vec::new();
    for raw in reads {
        let mut result = match_ocr(raw, lang, data);
        let mut corrected = false;
        if result == MatchResult::Unknown {
            if let Some(id) = correction_key(raw, lang)
                .map(|key| confirmed_key(lang, &key))
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
                if let Some(s) = suggest_ocr(raw, lang, data) {
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
                char::from_u32(c as u32 - 0xfee0).unwrap_or(c)
            } else if matches!(c, '\u{2018}' | '\u{2019}' | '\u{2032}') {
                '\''
            } else {
                c
            }
        })
        .flat_map(char::to_lowercase)
        .collect()
}

// Deliberately require an exact name. Guessing a similar name can give wrong immunities.
pub fn match_ocr(text: &str, lang: Language, data: &[Species]) -> MatchResult {
    let compact = compact(text);
    let ids = match explicit_numbers(&normalized(text)) {
        Ok(ids) => ids,
        Err(()) => return MatchResult::Unknown,
    };
    let aliases: Vec<_> = data.iter().map(|s| (s, name_aliases(s, lang))).collect();
    let mut found: Vec<_> = aliases
        .iter()
        .filter(|(_, names)| names.iter().any(|n| contains_name(&compact, n)))
        .map(|(s, _)| *s)
        .collect();
    // Prefer full species names over contained shorter names (e.g. リザード / リザードン).
    found.retain(|s| {
        !aliases.iter().any(|(other, names)| {
            other.id != s.id
                && names
                    .iter()
                    .any(|name| name.contains(&compact_name(s, lang)) && contains_name(&compact, name))
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
        // A misread suffix must not turn a shorter name into a longer species.
        !tail
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
            || tail.starts_with("lv")
    })
}

fn compact_name(species: &Species, lang: Language) -> String {
    compact(species.name(lang))
}
fn name_aliases(species: &Species, lang: Language) -> Vec<String> {
    let mut names = vec![compact_name(species, lang)];
    let extra: &[&str] = match (lang, species.id) {
        (Language::Zh, 233) => &["多边兽2", "多边兽ii", "多边兽Ⅱ"],
        (Language::Zh, 474) => &["多边兽z", "多边兽Ｚ"],
        // OCR often drops a gender sign or renders it as F/M.
        (Language::En, 29) => &["Nidoran♀", "NidoranF", "Nidoran female"],
        (Language::En, 32) => &["Nidoran♂", "NidoranM", "Nidoran male"],
        (Language::En, 83) => &["Farfetchd", "Farfetch'd"],
        (Language::En, 122) => &["Mr Mime"],
        (Language::En, 439) => &["Mime Jr", "MimeJr"],
        (Language::En, 250) => &["Ho Oh"],
        (Language::En, 474) => &["Porygon Z"],
        _ => &[],
    };
    for alias in extra {
        let alias = compact(alias);
        if !names.contains(&alias) {
            names.push(alias);
        }
    }
    names
}

pub fn lookup_query(text: &str, lang: Language, data: &[Species]) -> MatchResult {
    if let Ok(id) = text.trim().trim_start_matches('#').parse::<u16>() {
        if data.iter().any(|s| s.id == id) {
            return MatchResult::Found(id);
        }
        return MatchResult::Unknown;
    }
    match_ocr(text, lang, data)
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
        self.validate_for(width, height, Language::Zh)
    }
    pub fn validate_for(self, width: i32, height: i32, lang: Language) -> Result<Self, String> {
        if self.x < 0
            || self.y < 0
            || self.width < 8
            || self.height < 8
            || self.width > 2000
            || self.height > 1000
            || self.x as i64 + self.width as i64 > width as i64
            || self.y as i64 + self.height as i64 > height as i64
        {
            let message = match lang {
                Language::Zh => "选择区域必须在屏幕范围内，且至少 8×8 像素",
                Language::En => "Select an area inside the screen that is at least 8×8 pixels.",
                Language::Ja => "画面内で 8×8 ピクセル以上の範囲を選んでください。",
            };
            return Err(message.to_owned());
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
    fn manual_species(id: u16, zh: &str, en: &str, ja: &str) -> Species {
        Species {
            id,
            names: [zh.to_owned(), en.to_owned(), ja.to_owned()],
            types: vec![11],
        }
    }
    #[test]
    fn all_species_have_three_names_and_valid_types() {
        let data = species_data();
        assert_eq!(data.len(), 649);
        for id in 1..=649 {
            assert_eq!(data.iter().filter(|s| s.id == id).count(), 1);
        }
        assert!(data
            .iter()
            .all(|s| (1..=2).contains(&s.types.len()) && s.types.iter().all(|&t| t < 17)));
        for lang in Language::ALL {
            let mut names: Vec<_> = data.iter().map(|s| s.name(lang)).collect();
            assert!(
                names
                    .iter()
                    .all(|name| !name.is_empty() && !name.chars().any(char::is_control)),
                "{lang:?}"
            );
            names.sort_unstable();
            names.dedup();
            assert_eq!(names.len(), 649, "{lang:?} names must be unique");
        }
        assert_eq!(species(25).name(Language::Zh), "皮卡丘");
        assert_eq!(species(25).name(Language::En), "Pikachu");
        assert_eq!(species(25).name(Language::Ja), "ピカチュウ");
    }
    #[test]
    fn language_codes_round_trip() {
        for lang in Language::ALL {
            assert_eq!(Language::parse(lang.code()), Some(lang));
            assert!(Language::parse("xx").is_none());
        }
        assert_eq!(Language::parse("ja-JP"), Some(Language::Ja));
        assert_eq!(Language::parse("zh-Hans"), Some(Language::Zh));
        assert_eq!(Language::parse("en-US"), Some(Language::En));
    }
    #[test]
    fn known_screenshot_enemies() {
        let data = species_data();
        assert_eq!(
            match_ocr("雷 电 斑 马 Lv. 29 ♀", Language::Zh, &data),
            MatchResult::Found(523)
        );
        assert_eq!(
            match_ocr("奇诺栗鼠Lv.43", Language::Zh, &data),
            MatchResult::Found(573)
        );
        assert_eq!(
            match_ocr("Minipete Lv.30", Language::Zh, &data),
            MatchResult::Unknown
        );
        assert_eq!(
            match_ocr("Lv.29", Language::Zh, &data),
            MatchResult::Unknown
        );
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
            assert_eq!(
                match_ocr(text, Language::Zh, &data),
                MatchResult::Found(573),
                "{text}"
            );
        }
        assert_eq!(
            match_ocr("No.573 雷电斑马", Language::Zh, &data),
            MatchResult::Conflict
        );
        assert_eq!(
            match_ocr("No.573 No.523", Language::Zh, &data),
            MatchResult::Ambiguous
        );
        assert_eq!(
            match_ocr("Lv.573", Language::Zh, &data),
            MatchResult::Unknown
        );
        assert_eq!(
            match_ocr("573", Language::Zh, &data),
            MatchResult::Unknown
        );
        assert_eq!(
            match_ocr("No.999 奇诺栗鼠", Language::Zh, &data),
            MatchResult::Unknown
        );
        assert_eq!(match_ocr("No.0", Language::Zh, &data), MatchResult::Unknown);
        assert_eq!(
            match_ocr("No.57B", Language::Zh, &data),
            MatchResult::Unknown
        );
    }
    #[test]
    fn english_names_match_common_ocr_text() {
        let data = species_data();
        for (text, id) in [
            ("Pikachu Lv.29", 25),
            ("Charizard", 6),
            ("Mr. Mime Lv.20", 122),
            ("Mime Jr. Lv.15", 439),
            ("Farfetch'd", 83),
            ("Farfetchd", 83),
            ("Farfetch’d", 83),
            ("Ho-Oh", 250),
            ("Ho Oh", 250),
            ("Porygon-Z", 474),
            ("Porygon Z", 474),
            ("Nidoran♀", 29),
            ("Nidoran F", 29),
            ("Nidoran♂", 32),
            ("Nidoran M", 32),
            ("Mewtwo Lv.70", 150),
            ("Mew Lv.70", 151),
        ] {
            assert_eq!(
                match_ocr(text, Language::En, &data),
                MatchResult::Found(id),
                "{text}"
            );
        }
        assert_eq!(
            match_ocr("No.573", Language::En, &data),
            MatchResult::Found(573)
        );
        assert_eq!(
            match_ocr("Lv.29", Language::En, &data),
            MatchResult::Unknown
        );
        assert_eq!(
            match_ocr("573", Language::En, &data),
            MatchResult::Unknown
        );
        assert_eq!(
            match_ocr("Pikachu 小火龙", Language::En, &data),
            MatchResult::Found(25)
        );
    }
    #[test]
    fn japanese_names_match_common_ocr_text() {
        let data = species_data();
        for (text, id) in [
            ("ピカチュウ Lv.29", 25),
            ("リザードン", 6),
            ("リザード", 5),
            ("ミュウツー Lv.70", 150),
            ("ミュウ Lv.70", 151),
            ("ニドラン♀", 29),
            ("ニドラン♂", 32),
            ("フシギダネ", 1),
        ] {
            assert_eq!(
                match_ocr(text, Language::Ja, &data),
                MatchResult::Found(id),
                "{text}"
            );
        }
        assert_eq!(
            match_ocr("No.573", Language::Ja, &data),
            MatchResult::Found(573)
        );
        assert_eq!(
            match_ocr("Lv.29", Language::Ja, &data),
            MatchResult::Unknown
        );
        assert_eq!(
            match_ocr("573", Language::Ja, &data),
            MatchResult::Unknown
        );
    }
    #[test]
    fn matching_rejects_multiple_and_misread_names() {
        let data = species_data();
        assert_eq!(
            match_ocr("皮卡丘 小火龙", Language::Zh, &data),
            MatchResult::Ambiguous
        );
        assert_eq!(
            match_ocr("雷电班马", Language::Zh, &data),
            MatchResult::Unknown
        );
        assert_eq!(
            lookup_query("573", Language::Zh, &data),
            MatchResult::Found(573)
        );
        assert_eq!(
            match_ocr("多边兽2 Lv.20", Language::Zh, &data),
            MatchResult::Found(233)
        );
        assert_eq!(
            match_ocr("多边兽3 Lv.20", Language::Zh, &data),
            MatchResult::Unknown
        );
    }
    #[test]
    fn dual_types_and_immunities() {
        assert_eq!(incoming_multiplier(5, &species(6)), 4.0); // Rock -> Charizard
        assert_eq!(incoming_multiplier(4, &species(6)), 0.0); // Ground -> Charizard
        assert_eq!(incoming_multiplier(11, &species(6)), 0.25); // Grass -> Charizard
        assert_eq!(incoming_multiplier(1, &species(573)), 2.0);
        assert_eq!(incoming_multiplier(7, &species(573)), 0.0);
        assert_eq!(incoming_multiplier(4, &species(523)), 2.0);
        assert!(hint(&species(523), Language::Zh).contains("2倍：地面"));
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
        let error = Roi {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        }
        .validate_for(1920, 1080, Language::En)
        .unwrap_err();
        assert!(error.contains("8×8"));
    }
    #[test]
    fn near_name_is_only_a_candidate_until_confirmed() {
        let data = species_data();
        let mut aliases = std::collections::HashMap::new();
        let reads = vec!["君主尼".to_owned()];
        let decision = resolve_reads(&reads, Language::Zh, &data, &aliases);
        assert_eq!(decision.result, MatchResult::Unknown);
        assert_eq!(
            decision.suggestion.unwrap(),
            NameSuggestion {
                id: 497,
                observed: "君主尼".to_owned()
            }
        );
        aliases.insert(confirmed_key(Language::Zh, "君主尼"), 497);
        let decision = resolve_reads(&reads, Language::Zh, &data, &aliases);
        assert_eq!(decision.result, MatchResult::Found(497));
        assert!(decision.corrected);
        assert!(suggest_ocr("No.49B", Language::Zh, &data).is_none());
        assert!(suggest_ocr("蛇", Language::Zh, &data).is_none());
        let ambiguous = vec![
            manual_species(1, "君主蛇", "Serperior", "ジャローダ"),
            manual_species(2, "君主猴", "Serperiorm", "ジャローダー"),
        ];
        assert!(suggest_ocr("君主尼", Language::Zh, &ambiguous).is_none());
    }
    #[test]
    fn suggestions_and_confirmations_are_scoped_to_the_game_language() {
        let data = species_data();
        let suggestion = suggest_ocr("Pikachi", Language::En, &data).unwrap();
        assert_eq!(suggestion.id, 25);
        assert_eq!(suggestion.observed, "pikachi");
        let mut aliases = std::collections::HashMap::new();
        aliases.insert(confirmed_key(Language::En, "pikachi"), 25);
        let decision = resolve_reads(&["Pikachi".to_owned()], Language::En, &data, &aliases);
        assert_eq!(decision.result, MatchResult::Found(25));
        assert!(decision.corrected);
        let other = resolve_reads(&["Pikachi".to_owned()], Language::Ja, &data, &aliases);
        assert_eq!(other.result, MatchResult::Unknown);
        let japanese = suggest_ocr("ピカチュゥ", Language::Ja, &data).unwrap();
        assert_eq!(japanese.id, 25);
    }
    #[test]
    fn alternative_read_can_rescue_but_cannot_override_conflicts() {
        let data = species_data();
        let aliases = std::collections::HashMap::new();
        assert_eq!(
            resolve_reads(
                &["君主尼".to_owned(), "君主蛇".to_owned()],
                Language::Zh,
                &data,
                &aliases
            )
            .result,
            MatchResult::Found(497)
        );
        assert_eq!(
            resolve_reads(
                &["君主蛇".to_owned(), "皮卡丘".to_owned()],
                Language::Zh,
                &data,
                &aliases
            )
            .result,
            MatchResult::Conflict
        );
        assert_eq!(
            resolve_reads(
                &["No.573 雷电斑马".to_owned(), "奇诺栗鼠".to_owned()],
                Language::Zh,
                &data,
                &aliases
            )
            .result,
            MatchResult::Conflict
        );
    }
    #[test]
    fn neutral_damage_matches_serperior_and_all_types_are_present_once() {
        let card = hint_card(&species(497), Language::Zh);
        assert_eq!(
            card.neutral
                .iter()
                .map(|effect| TYPE_NAMES_ZH[effect.type_id])
                .collect::<Vec<_>>(),
            vec!["普通", "格斗", "岩石", "幽灵", "钢", "超能力", "龙", "恶"]
        );
        assert!(hint(&species(497), Language::Zh)
            .contains("普通伤害（1倍）  普通、格斗、岩石、幽灵、钢、超能力、龙、恶"));
        for pokemon in species_data() {
            let card = hint_card(&pokemon, Language::Zh);
            let mut seen = [false; 17];
            for group in [
                &card.weaknesses,
                &card.neutral,
                &card.resistances,
                &card.immunities,
            ] {
                for effect in group {
                    assert!(!seen[effect.type_id], "{}", pokemon.name(Language::Zh));
                    seen[effect.type_id] = true;
                }
            }
            assert!(
                seen.into_iter().all(|seen| seen),
                "{}",
                pokemon.name(Language::Zh)
            );
            assert!(card.neutral.iter().all(|effect| effect.multiplier == 1.0));
        }
    }
    #[test]
    fn card_separates_type_effects_and_omits_old_caption() {
        let card = hint_card(&species(6), Language::Zh);
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
        assert!(!hint(&species(6), Language::Zh).contains("基础形态属性"));
    }
    #[test]
    fn type_names_and_hints_are_localized() {
        for lang in Language::ALL {
            let names = type_names(lang);
            let mut unique = names.to_vec();
            unique.sort_unstable();
            unique.dedup();
            assert_eq!(unique.len(), 17, "{lang:?}");
            assert!(names.iter().all(|name| !name.is_empty()));
        }
        let english = hint(&species(6), Language::En);
        assert!(english.contains("#006 Charizard · Fire / Flying"));
        assert!(english.contains("Weaknesses  4x: Rock"));
        assert!(english.contains("Immunities  Ground"));
        let japanese = hint(&species(1), Language::Ja);
        assert!(japanese.contains("#001 フシギダネ · くさ / どく"));
        assert!(japanese.contains("弱点"));
        assert!(japanese.contains("耐性"));
        assert_eq!(type_name(Language::En, 9), "Fire");
        assert_eq!(type_name(Language::Ja, 9), "ほのお");
    }
}
