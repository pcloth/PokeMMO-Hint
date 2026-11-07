use crate::{HintCard, Roi, TYPE_NAMES};

pub const MAX_REGIONS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub roi: Roi,
    pub width: i32,
    pub height: i32,
    pub origin_x: i32,
    pub origin_y: i32,
}
impl Selection {
    pub fn desktop(self) -> Bounds {
        Bounds {
            x: self.origin_x,
            y: self.origin_y,
            width: self.width,
            height: self.height,
        }
    }
    pub fn screen_bounds(self, desktop: Bounds) -> Result<Bounds, String> {
        self.roi.validate(self.width, self.height)?;
        if self.desktop() != desktop {
            return Err("屏幕布局或分辨率改变，请重新框选该区域。".to_owned());
        }
        let x = self
            .origin_x
            .checked_add(self.roi.x)
            .ok_or("屏幕坐标超出范围")?;
        let y = self
            .origin_y
            .checked_add(self.roi.y)
            .ok_or("屏幕坐标超出范围")?;
        x.checked_add(self.roi.width).ok_or("屏幕坐标超出范围")?;
        y.checked_add(self.roi.height).ok_or("屏幕坐标超出范围")?;
        Ok(Bounds {
            x,
            y,
            width: self.roi.width,
            height: self.roi.height,
        })
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RegionSettings {
    pub selections: Vec<Selection>,
    pub names: Vec<String>,
    pub selected: usize,
    pub window: Option<(i32, i32)>,
}
impl RegionSettings {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut result = Self::default();
        let mut lines = text.lines();
        if lines.next() != Some("pokemon-screen-regions-v1") {
            return Err("选区不是屏幕坐标格式；旧版窗口选区需要重新框选。".to_owned());
        }
        for line in lines.filter(|l| !l.is_empty()) {
            let (key, value) = line.split_once('=').ok_or("选区配置格式错误")?;
            match key {
                "region" => result.selections.push(parse_selection(value)?),
                "name" => {
                    let (index, name) = value.split_once(',').ok_or("选区名称格式错误")?;
                    let index: usize = index.parse().map_err(|_| "选区序号错误")?;
                    if index >= MAX_REGIONS {
                        return Err("选区序号超出范围".to_owned());
                    }
                    result
                        .names
                        .resize(result.names.len().max(index + 1), String::new());
                    result.names[index] = validate_name(name)?;
                }
                "selected" => result.selected = value.parse().map_err(|_| "选区序号错误")?,
                "window" => {
                    let (x, y) = value.split_once(',').ok_or("窗口位置错误")?;
                    result.window = Some((
                        x.parse().map_err(|_| "窗口位置错误")?,
                        y.parse().map_err(|_| "窗口位置错误")?,
                    ));
                }
                _ => return Err(format!("未知配置字段：{key}")),
            }
        }
        if result.selections.len() > MAX_REGIONS {
            return Err(format!("最多支持 {MAX_REGIONS} 个选区"));
        }
        result.names.resize(result.selections.len(), String::new());
        result.selected = result
            .selected
            .min(result.selections.len().saturating_sub(1));
        Ok(result)
    }
    pub fn encode(&self) -> String {
        let mut text = format!("pokemon-screen-regions-v1\nselected={}\n", self.selected);
        if let Some((x, y)) = self.window {
            text.push_str(&format!("window={x},{y}\n"));
        }
        for (index, s) in self.selections.iter().enumerate() {
            text.push_str(&format!(
                "region={},{},{},{},{}\n",
                s.roi, s.width, s.height, s.origin_x, s.origin_y
            ));
            if let Some(name) = self.names.get(index).filter(|n| !n.is_empty()) {
                text.push_str(&format!("name={index},{name}\n"));
            }
        }
        text
    }
    pub fn name(&self, index: usize) -> String {
        self.names
            .get(index)
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("选区 {}", index + 1))
    }
    pub fn rename(&mut self, index: usize, name: &str) -> Result<(), String> {
        if index >= self.selections.len() {
            return Err("请先添加选区".to_owned());
        }
        let name = validate_name(name)?;
        self.names.resize(self.selections.len(), String::new());
        self.names[index] = name;
        Ok(())
    }
}
fn validate_name(name: &str) -> Result<String, String> {
    if name.chars().any(char::is_control) || name.trim().chars().count() > 32 {
        return Err("选区名称最多 32 个字，不能包含换行或控制字符".to_owned());
    }
    Ok(name.trim().to_owned())
}
fn parse_selection(text: &str) -> Result<Selection, String> {
    let values: Vec<i32> = text
        .split(',')
        .map(|s| s.parse().map_err(|_| "选区坐标错误".to_owned()))
        .collect::<Result<_, _>>()?;
    if values.len() != 8 {
        return Err("选区坐标数量错误".to_owned());
    }
    let roi = Roi {
        x: values[0],
        y: values[1],
        width: values[2],
        height: values[3],
    }
    .validate(values[4], values[5])?;
    let selection = Selection {
        roi,
        width: values[4],
        height: values[5],
        origin_x: values[6],
        origin_y: values[7],
    };
    selection.screen_bounds(selection.desktop())?;
    Ok(selection)
}

/// Bit mask: left=1, right=2, top=4, bottom=8; zero means move.
pub fn drag_roi(roi: Roi, dx: i32, dy: i32, edges: u8, width: i32, height: i32) -> Roi {
    if edges == 0 {
        return Roi {
            x: (roi.x + dx).clamp(0, width - roi.width),
            y: (roi.y + dy).clamp(0, height - roi.height),
            ..roi
        };
    }
    let (mut left, mut right, mut top, mut bottom) =
        (roi.x, roi.x + roi.width, roi.y, roi.y + roi.height);
    if edges & 1 != 0 {
        left = (left + dx).clamp((right - 2000).max(0), right - 8);
    }
    if edges & 2 != 0 {
        right = (right + dx).clamp(left + 8, (left + 2000).min(width));
    }
    if edges & 4 != 0 {
        top = (top + dy).clamp((bottom - 1000).max(0), bottom - 8);
    }
    if edges & 8 != 0 {
        bottom = (bottom + dy).clamp(top + 8, (top + 1000).min(height));
    }
    Roi {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}

pub fn first_weakness(card: &HintCard) -> String {
    match card.weaknesses.first() {
        Some(e) => format!("{}×{}", TYPE_NAMES[e.type_id], e.multiplier),
        None => "无弱点".to_owned(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl Bounds {
    pub fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
    pub fn contains(self, x: i32, y: i32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
    fn clamped(self, screen: Self) -> Self {
        Self {
            x: self.x.clamp(
                screen.x,
                (screen.x + screen.width - self.width).max(screen.x),
            ),
            y: self.y.clamp(
                screen.y,
                (screen.y + screen.height - self.height).max(screen.y),
            ),
            ..self
        }
    }
}
pub fn tag_position(region: Bounds, size: (i32, i32), screen: Bounds) -> Bounds {
    let (width, height) = size;
    let above = Bounds {
        x: region.x,
        y: region.y - height - 6,
        width,
        height,
    }
    .clamped(screen);
    if !above.overlaps(region) {
        return above;
    }
    // At the top edge, keep the label on screen without covering its own OCR crop.
    for (x, y) in [
        (region.x + region.width + 6, region.y),
        (region.x - width - 6, region.y),
        (region.x, region.y + region.height + 6),
    ] {
        let candidate = Bounds {
            x,
            y,
            width,
            height,
        }
        .clamped(screen);
        if !candidate.overlaps(region) {
            return candidate;
        }
    }
    above
}
pub fn detail_position(
    region: Bounds,
    size: (i32, i32),
    screen: Bounds,
    obstacles: &[Bounds],
) -> Bounds {
    let (width, height) = size;
    let candidates = [
        Bounds {
            x: region.x,
            y: region.y + region.height + 10,
            width,
            height,
        },
        Bounds {
            x: region.x + region.width + 10,
            y: region.y,
            width,
            height,
        },
        Bounds {
            x: region.x - width - 10,
            y: region.y,
            width,
            height,
        },
        Bounds {
            x: region.x,
            y: region.y - height - 42,
            width,
            height,
        },
    ];
    candidates
        .into_iter()
        .map(|r| r.clamped(screen))
        .min_by_key(|r| obstacles.iter().filter(|o| r.overlaps(**o)).count())
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remembers_multiple_regions_and_control_window() {
        let a = Selection {
            roi: Roi {
                x: 90,
                y: 50,
                width: 200,
                height: 30,
            },
            width: 1920,
            height: 1080,
            origin_x: 0,
            origin_y: 0,
        };
        let b = Selection {
            roi: Roi {
                x: 550,
                y: 55,
                width: 180,
                height: 30,
            },
            ..a
        };
        let config = RegionSettings {
            selections: vec![a, b],
            names: vec![String::new(), String::new()],
            selected: 1,
            window: Some((-800, 200)),
        };
        assert_eq!(RegionSettings::parse(&config.encode()).unwrap(), config);
        assert!(RegionSettings::parse("90,50,200,30,1920,1080").is_err());
        assert!(
            RegionSettings::parse("pokemmo-regions-v2\nregion=90,50,200,30,1920,1080\n").is_err()
        );
        assert!(RegionSettings::parse(
            "pokemon-screen-regions-v1\nregion=1900,0,200,30,1920,1080,0,0\n"
        )
        .is_err());
        assert!(RegionSettings::parse("pokemon-screen-regions-v1\nregion=broken\n").is_err());
        // An intentionally emptied list must round-trip, rather than reviving the legacy ROI.
        let empty = RegionSettings::default();
        assert_eq!(RegionSettings::parse(&empty.encode()).unwrap(), empty);
        let oversized = format!(
            "pokemon-screen-regions-v1\n{}",
            "region=90,50,200,30,1920,1080,0,0\n".repeat(MAX_REGIONS + 1)
        );
        assert!(RegionSettings::parse(&oversized).is_err());
    }
    #[test]
    fn custom_names_round_trip_and_unnamed_lists_have_defaults() {
        let mut settings = RegionSettings::parse(
            "pokemon-screen-regions-v1\nregion=90,50,200,30,1920,1080,0,0\nregion=550,55,180,30,1920,1080,0,0\n",
        )
        .unwrap();
        assert_eq!(settings.name(0), "选区 1");
        settings.rename(0, "  群怪,左侧=1  ").unwrap();
        settings.rename(1, "图鉴 No.编号").unwrap();
        let restored = RegionSettings::parse(&settings.encode()).unwrap();
        assert_eq!(restored, settings);
        assert_eq!(restored.name(0), "群怪,左侧=1");
        assert!(settings.rename(0, "坏\n名称").is_err());
        assert!(settings.rename(0, &"长".repeat(33)).is_err());
        assert!(settings.rename(2, "不存在").is_err());
        settings.rename(0, " ").unwrap();
        assert_eq!(settings.name(0), "选区 1");
    }
    #[test]
    fn screen_coordinates_support_negative_monitors_and_reject_changed_layout() {
        let desktop = Bounds {
            x: -1920,
            y: -200,
            width: 3840,
            height: 1280,
        };
        let selection = Selection {
            roi: Roi {
                x: 100,
                y: 50,
                width: 200,
                height: 30,
            },
            width: desktop.width,
            height: desktop.height,
            origin_x: desktop.x,
            origin_y: desktop.y,
        };
        assert_eq!(
            selection.screen_bounds(desktop).unwrap(),
            Bounds {
                x: -1820,
                y: -150,
                width: 200,
                height: 30
            }
        );
        let config = RegionSettings {
            selections: vec![selection],
            ..RegionSettings::default()
        };
        assert_eq!(
            RegionSettings::parse(&config.encode()).unwrap(),
            RegionSettings {
                names: vec![String::new()],
                ..config
            }
        );
        assert!(selection.screen_bounds(Bounds { x: 0, ..desktop }).is_err());
        assert!(selection
            .screen_bounds(Bounds {
                width: 1920,
                ..desktop
            })
            .is_err());
        assert!(RegionSettings::parse(
            "pokemon-screen-regions-v1\nregion=100,0,200,30,1920,1080,2147483647,0\n"
        )
        .is_err());
    }
    #[test]
    fn labels_are_above_each_region_and_support_negative_monitor_coordinates() {
        let screen = Bounds {
            x: -1920,
            y: 0,
            width: 3840,
            height: 1080,
        };
        let region = Bounds {
            x: -1600,
            y: 100,
            width: 200,
            height: 30,
        };
        let tag = tag_position(region, (90, 28), screen);
        assert!(tag.y + tag.height < region.y);
        assert!(!tag.overlaps(region));
        let top = Bounds { y: 0, ..region };
        assert!(!tag_position(top, (90, 28), screen).overlaps(top));
        let details = detail_position(region, (600, 280), screen, &[region]);
        assert!(!details.overlaps(region));
        assert!(details.x >= screen.x && details.x + details.width <= screen.x + screen.width);
    }
    #[test]
    fn moving_and_resizing_regions_stay_within_the_desktop() {
        let roi = Roi {
            x: 100,
            y: 80,
            width: 200,
            height: 30,
        };
        assert_eq!(
            drag_roi(roi, 50, 20, 0, 800, 600),
            Roi {
                x: 150,
                y: 100,
                ..roi
            }
        );
        assert_eq!(
            drag_roi(roi, -500, 900, 0, 800, 600),
            Roi {
                x: 0,
                y: 570,
                ..roi
            }
        );
        assert_eq!(
            drag_roi(roi, 50, 20, 10, 800, 600),
            Roi {
                width: 250,
                height: 50,
                ..roi
            }
        );
        assert_eq!(
            drag_roi(roi, 999, 999, 5, 800, 600),
            Roi {
                x: 292,
                y: 102,
                width: 8,
                height: 8
            }
        );
        assert!(drag_roi(roi, 999, 999, 10, 800, 600)
            .validate(800, 600)
            .is_ok());
        assert_eq!(drag_roi(roi, 0, 0, 0, 800, 600), roi);
    }
    #[test]
    fn compact_label_uses_only_the_first_weakness() {
        let species = crate::species_data();
        let card = crate::hint_card(species.iter().find(|s| s.id == 6).unwrap());
        assert_eq!(first_weakness(&card), "岩石×4");
        let mut empty = card;
        empty.weaknesses.clear();
        assert_eq!(first_weakness(&empty), "无弱点");
    }
    #[test]
    fn trackers_are_independent_and_continue_after_a_success() {
        use crate::{MatchResult, Tracker};
        let mut states = [Tracker::default(), Tracker::default()];
        states[0].update(&MatchResult::Found(6));
        states[1].update(&MatchResult::Found(523));
        assert_eq!(states[0].update(&MatchResult::Found(6)), Some(6));
        assert_eq!(states[1].update(&MatchResult::Unknown), None);
        assert_eq!(states[0].update(&MatchResult::Found(6)), Some(6));
        assert_eq!(states[0].update(&MatchResult::Found(573)), None);
        assert_eq!(states[0].update(&MatchResult::Found(573)), Some(573));
    }
}
