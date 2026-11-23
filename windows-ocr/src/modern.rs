//! Shared modern Pokédex assets are also loaded by the Android application.
use crate::{hint_card, species_data, HintCard, Language, Species};
use std::collections::HashMap;
use std::sync::OnceLock;

pub const MAX_SPECIES: u16 = 1025;
pub const TYPE_COUNT: usize = 18;

pub fn species(modern: bool) -> &'static [Species] {
    static LEGACY: OnceLock<Vec<Species>> = OnceLock::new();
    static MODERN: OnceLock<Vec<Species>> = OnceLock::new();
    if !modern {
        return LEGACY.get_or_init(species_data);
    }
    MODERN.get_or_init(|| {
        let legacy = species(false);
        rows(include_str!("../data/modern_species.tsv"))
            .map(|row| {
                let id: u16 = row[0].parse().expect("National Pokédex number");
                let names = legacy.get(usize::from(id - 1)).map_or_else(
                    || [row[2].into(), row[3].into(), row[4].into()],
                    |entry| entry.names.clone(),
                );
                Species {
                    id,
                    names,
                    types: parse_types(row[1]),
                    modern: true,
                }
            })
            .collect()
    })
}

/// Manual entry can use any supported language and the full National Pokédex.
pub fn manual_match(text: &str) -> crate::MatchResult {
    use crate::MatchResult;
    let mut found = None;
    for language in [Language::Zh, Language::En, Language::Ja] {
        match crate::lookup_query(text, language, species(true)) {
            MatchResult::Found(id) => {
                if found.is_some_and(|previous| previous != id) {
                    return MatchResult::Ambiguous;
                }
                found = Some(id);
            }
            MatchResult::Unknown => {}
            problem => return problem,
        }
    }
    found.map_or(MatchResult::Unknown, MatchResult::Found)
}

pub fn manual_species(id: u16, modern: bool) -> Option<&'static Species> {
    let index = usize::from(id.checked_sub(1)?);
    species(modern)
        .get(index)
        .or_else(|| species(true).get(index))
}

#[derive(Debug)]
pub struct Form {
    id: u16,
    key: &'static str,
    names: [String; 3],
    types: Vec<usize>,
}
impl Form {
    pub fn name(&self, language: Language) -> &str {
        &self.names[language.index()]
    }

    pub fn menu_label(&self, language: Language) -> String {
        let types = self
            .types
            .iter()
            .map(|&ty| crate::type_name(language, ty))
            .collect::<Vec<_>>();
        format!("{} · {}", self.name(language), types.join(" / "))
    }
}

pub fn forms(id: u16) -> impl Iterator<Item = &'static Form> {
    static FORMS: OnceLock<Vec<Form>> = OnceLock::new();
    FORMS
        .get_or_init(|| {
            rows(include_str!("../data/modern_forms.tsv"))
                .map(|row| Form {
                    id: row[0].parse().expect("form species number"),
                    key: row[1],
                    types: parse_types(row[2]),
                    names: [row[3].into(), row[4].into(), row[5].into()],
                })
                .collect()
        })
        .iter()
        .filter(move |form| form.id == id && !form.key.ends_with("-gmax"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTab {
    Legacy,
    Modern,
    Tera,
    Dynamax,
}
impl CardTab {
    pub const ALL: [Self; 4] = [Self::Legacy, Self::Modern, Self::Tera, Self::Dynamax];

    pub fn available(self, id: u16) -> bool {
        match self {
            Self::Legacy => manual_species(id, false).is_some_and(|species| !species.modern),
            Self::Dynamax => dynamax_forms(id).is_some(),
            Self::Modern | Self::Tera => true,
        }
    }
}

fn dynamax_forms(id: u16) -> Option<&'static str> {
    static ROSTER: OnceLock<HashMap<u16, &str>> = OnceLock::new();
    ROSTER
        .get_or_init(|| {
            rows(include_str!("../data/dynamax.tsv"))
                .map(|row| {
                    let id = row[0].parse().expect("Dynamax species number");
                    (id, row.get(1).copied().unwrap_or_default())
                })
                .collect()
        })
        .get(&id)
        .copied()
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct BattleTyping {
    pub tab: Option<CardTab>,
    pub form: Option<usize>,
    pub tera: Option<usize>,
    pub gigantamax: bool,
}
impl BattleTyping {
    pub fn active_tab(self, id: u16, default_modern: bool) -> CardTab {
        self.tab.filter(|tab| tab.available(id)).unwrap_or_else(|| {
            if default_modern || !CardTab::Legacy.available(id) {
                CardTab::Modern
            } else {
                CardTab::Legacy
            }
        })
    }

    pub fn selected_form(self, id: u16) -> Option<&'static Form> {
        self.form.and_then(|index| forms(id).nth(index))
    }

    pub fn fixed_tera(self, id: u16) -> Option<usize> {
        match id {
            1017 => Some(
                self.selected_form(id)
                    .and_then(|form| form.types.get(1).copied())
                    .unwrap_or(11),
            ),
            1024 => Some(TYPE_COUNT),
            _ => None,
        }
    }

    pub fn active_tera(self, id: u16) -> Option<usize> {
        self.tera
            .map(|selected| self.fixed_tera(id).unwrap_or(selected))
    }

    pub fn can_gigantamax(self, id: u16) -> bool {
        // Pokémon species represented by the -gmax rows in PokéAPI pokemon.csv.
        const SPECIES: [u16; 32] = [
            3, 6, 9, 12, 25, 52, 68, 94, 99, 131, 133, 143, 569, 809, 812, 815, 818, 823, 826, 834,
            839, 841, 842, 844, 849, 851, 858, 861, 869, 879, 884, 892,
        ];
        SPECIES.contains(&id)
            && self
                .selected_form(id)
                .filter(|form| Self::dynamax_form(form))
                .is_none_or(|form| {
                    matches!(form.key, "urshifu-rapid-strike" | "toxtricity-low-key")
                })
    }

    pub fn dynamax_form(form: &Form) -> bool {
        dynamax_forms(form.id).is_some_and(|keys| keys.split(',').any(|key| key == form.key))
    }

    pub fn card(self, base: &HintCard, language: Language) -> HintCard {
        let tab = self.active_tab(base.id, base.modern);
        let use_modern = tab != CardTab::Legacy;
        if self == Self::default() {
            return base.clone();
        }
        let Some(species) = manual_species(base.id, use_modern) else {
            return base.clone();
        };
        match tab {
            CardTab::Legacy => hint_card(species, language),
            CardTab::Modern => card(species, self.selected_form(base.id), None, language),
            CardTab::Tera => card(
                species,
                self.selected_form(base.id),
                self.active_tera(base.id),
                language,
            ),
            CardTab::Dynamax => {
                let form = self
                    .selected_form(base.id)
                    .filter(|form| Self::dynamax_form(form));
                let mut result = card(species, form, None, language);
                let gigantamax = self.gigantamax && self.can_gigantamax(base.id);
                result.name.push_str(" · ");
                result.name.push_str(match (language, gigantamax) {
                    (Language::Zh, false) => "极巨化",
                    (Language::Zh, true) => "超极巨化",
                    (Language::En, false) => "Dynamax",
                    (Language::En, true) => "Gigantamax",
                    (Language::Ja, false) => "ダイマックス",
                    (Language::Ja, true) => "キョダイマックス",
                });
                result
            }
        }
    }
}

/// A regular Tera type replaces the defensive typing. Stellar keeps the
/// selected form's defensive typing for the 18 ordinary attack types.
pub fn card(
    species: &Species,
    form: Option<&Form>,
    tera: Option<usize>,
    language: Language,
) -> HintCard {
    let mut effective = species.clone();
    if let Some(form) = form.filter(|form| form.id == species.id) {
        effective.types.clone_from(&form.types);
    }
    if let Some(tera) = tera.filter(|&value| value < TYPE_COUNT) {
        effective.types = vec![tera];
    }
    let mut result = hint_card(&effective, language);
    if let Some(form) = form.filter(|form| form.id == species.id) {
        result.name.push_str(" · ");
        result.name.push_str(form.name(language));
    }
    if let Some(tera) = tera.filter(|&tera| tera <= TYPE_COUNT) {
        result.name.push_str(" · ");
        result.name.push_str(match language {
            Language::Zh => "太晶",
            Language::En => "Tera ",
            Language::Ja => "テラスタル ",
        });
        result.name.push_str(if tera == TYPE_COUNT {
            match language {
                Language::Zh => "星晶",
                Language::En => "Stellar",
                Language::Ja => "ステラ",
            }
        } else {
            crate::type_name(language, tera)
        });
    }
    result
}

pub fn effectiveness(attack: usize, defender: usize) -> f64 {
    static CHART: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    let chart = CHART.get_or_init(|| {
        rows(include_str!("../data/modern_chart.tsv"))
            .map(|row| {
                row.iter()
                    .map(|value| value.parse().expect("modern damage multiplier"))
                    .collect()
            })
            .collect()
    });
    f64::from(chart[attack][defender]) / 100.0
}

fn rows(data: &str) -> impl Iterator<Item = Vec<&str>> {
    data.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| line.split('\t').collect())
}

fn parse_types(values: &str) -> Vec<usize> {
    values
        .split(',')
        .map(|value| value.parse().expect("modern type ID"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lookup_query, match_ocr, MatchResult};

    #[test]
    fn modern_catalog_covers_every_species_and_four_digit_ocr() {
        let catalog = species(true);
        assert_eq!(catalog.len(), usize::from(MAX_SPECIES));
        for (index, entry) in catalog.iter().enumerate() {
            assert_eq!(usize::from(entry.id), index + 1);
            assert!(entry.names.iter().all(|name| !name.is_empty()));
            assert!((1..=2).contains(&entry.types.len()));
            assert!(entry.types.iter().all(|&t| t < TYPE_COUNT));
        }
        for (name, lang) in [
            ("桃歹郎", Language::Zh),
            ("Pecharunt", Language::En),
            ("モモワロウ", Language::Ja),
        ] {
            assert_eq!(lookup_query(name, lang, catalog), MatchResult::Found(1025));
        }
        assert_eq!(
            match_ocr("No.1025 Lv.100", Language::Zh, catalog),
            MatchResult::Found(1025)
        );
        assert_eq!(
            match_ocr("No.1025 Pecharunt", Language::En, catalog),
            MatchResult::Found(1025)
        );
        assert_eq!(
            match_ocr("No.1025 Pikachu", Language::En, catalog),
            MatchResult::Conflict
        );
        assert_eq!(
            match_ocr("No.1025", Language::Zh, species(false)),
            MatchResult::Unknown
        );
    }

    #[test]
    fn manual_query_can_reach_modern_species_without_changing_legacy_typing() {
        for name in ["苍响", "Zacian", "ザシアン", "No.888", "888"] {
            assert_eq!(manual_match(name), MatchResult::Found(888), "{name}");
        }
        assert_eq!(manual_match("No.888 Pikachu"), MatchResult::Conflict);
        assert!(manual_species(888, false).unwrap().modern);
        assert!(!manual_species(39, false).unwrap().modern);
        assert!(manual_species(39, true).unwrap().modern);
    }

    #[test]
    fn cards_show_the_selected_rules_and_keep_tera_separate() {
        let base = hint_card(manual_species(39, false).unwrap(), Language::Zh);
        let mut typing = BattleTyping::default();
        assert_eq!(typing.active_tab(base.id, base.modern), CardTab::Legacy);
        assert_eq!(typing.card(&base, Language::Zh).types, [0]);

        typing.tab = Some(CardTab::Modern);
        let modern = typing.card(&base, Language::Zh);
        assert_eq!(modern.types, [0, 17]);
        assert_eq!(
            modern
                .weaknesses
                .iter()
                .map(|effect| effect.type_id)
                .collect::<Vec<_>>(),
            [3, 8]
        );

        typing.tab = Some(CardTab::Tera);
        typing.tera = Some(10);
        assert_eq!(typing.card(&base, Language::Zh).types, [10]);
        typing.tab = Some(CardTab::Modern);
        assert_eq!(typing.card(&base, Language::Zh).types, [0, 17]);

        assert_eq!(typing.active_tab(888, false), CardTab::Modern);
        assert!(!CardTab::Legacy.available(888));
        assert!(!CardTab::Dynamax.available(888));
        assert!(!CardTab::Dynamax.available(1025));
        assert!(!CardTab::Dynamax.available(19)); // Rattata is absent from Sword and Shield.
    }

    #[test]
    fn gigantamax_preserves_the_selected_sword_and_shield_form_types() {
        let base = hint_card(manual_species(892, false).unwrap(), Language::Zh);
        let mut typing = BattleTyping {
            tab: Some(CardTab::Dynamax),
            tera: Some(9),
            gigantamax: true,
            ..BattleTyping::default()
        };
        let single = typing.card(&base, Language::Zh);
        assert_eq!(single.types, [1, 16]);
        assert!(single.name.ends_with("超极巨化"));
        typing.form = forms(892).position(|form| form.key == "urshifu-rapid-strike");
        let rapid = typing.card(&base, Language::Zh);
        assert_eq!(rapid.types, [1, 10]);
        assert!(rapid.name.ends_with("超极巨化"));

        let base = hint_card(manual_species(6, false).unwrap(), Language::Zh);
        typing.form = forms(6).position(|form| form.key == "charizard-mega-x");
        typing.gigantamax = false;
        let regular = typing.card(&base, Language::Zh);
        assert_eq!(regular.types, [9, 2]);
        assert!(regular.name.ends_with(" · 极巨化"));
    }

    #[test]
    fn fixed_tera_type_follows_the_selected_mask() {
        let base = hint_card(manual_species(1017, true).unwrap(), Language::Zh);
        let mut typing = BattleTyping {
            tab: Some(CardTab::Tera),
            tera: Some(11),
            ..BattleTyping::default()
        };
        assert_eq!(typing.card(&base, Language::Zh).types, [11]);
        typing.form = forms(1017).position(|form| form.key == "ogerpon-wellspring-mask");
        assert_eq!(typing.card(&base, Language::Zh).types, [10]);
        assert_eq!(typing.active_tera(1024), Some(TYPE_COUNT));
    }

    #[test]
    fn form_and_tera_use_modern_defensive_typing() {
        let charizard = &species(true)[5];
        let mega = forms(6).find(|form| form.types == [9, 15]).unwrap();
        let changed = card(charizard, Some(mega), None, Language::Zh);
        assert_eq!(changed.types, [9, 15]);
        assert!(changed
            .weaknesses
            .iter()
            .all(|effect| effect.multiplier != 4.0));
        let stellar = card(charizard, Some(mega), Some(TYPE_COUNT), Language::Zh);
        assert_eq!(stellar.types, changed.types);
        let tera = card(&species(true)[247], None, Some(7), Language::Zh);
        assert_eq!(tera.types, [7]);
        assert_eq!(
            tera.weaknesses
                .iter()
                .map(|effect| effect.type_id)
                .collect::<Vec<_>>(),
            [7, 16]
        );
        assert_eq!(effectiveness(15, 17), 0.0); // Dragon vs Fairy
        assert_eq!(effectiveness(7, 8), 1.0); // Modern Steel vs Ghost
        assert_eq!(crate::effectiveness(7, 8), 0.5); // Legacy Steel vs Ghost
    }
}
