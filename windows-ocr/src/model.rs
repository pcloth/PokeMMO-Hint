//! Inspect the LSTM character table, so a model's missing glyphs are visible to users.
use std::collections::BTreeSet;

pub fn lstm_characters(data: &[u8]) -> Result<BTreeSet<char>, String> {
    let count =
        u32::from_le_bytes(data.get(..4).ok_or("模型头不完整")?.try_into().unwrap()) as usize;
    if !(22..=64).contains(&count) || data.len() < 4 + count * 8 {
        return Err("模型目录不完整".to_owned());
    }
    let offsets: Vec<i64> = (0..count)
        .map(|i| i64::from_le_bytes(data[4 + i * 8..12 + i * 8].try_into().unwrap()))
        .collect();
    let start = offsets[21]; // TESSDATA_LSTM_UNICHARSET
    if start < (4 + count * 8) as i64 || start >= data.len() as i64 {
        return Err("没有可读的 LSTM 字表".to_owned());
    }
    let end = offsets
        .iter()
        .copied()
        .filter(|&v| v > start)
        .min()
        .unwrap_or(data.len() as i64);
    if end > data.len() as i64 {
        return Err("模型字表越界".to_owned());
    }
    let text = std::str::from_utf8(&data[start as usize..end as usize])
        .map_err(|_| "模型字表不是 UTF-8")?;
    let mut lines = text.lines();
    let expected: usize = lines
        .next()
        .ok_or("字表为空")?
        .parse()
        .map_err(|_| "字表计数错误")?;
    let rows: Vec<_> = lines.collect();
    if expected != rows.len() {
        return Err("模型字表行数不符".to_owned());
    }
    Ok(rows
        .iter()
        .filter_map(|line| line.split_whitespace().next())
        .flat_map(str::chars)
        .collect())
}
pub fn missing_characters(name: &str, characters: &BTreeSet<char>) -> String {
    name.chars()
        .filter(|c| !characters.contains(c))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_model_character_table_and_reports_unrepresentable_names() {
        let table = "3\n猫 1 Han\n斩 1 Han\n蛇 1 Han\n";
        let mut data = vec![0; 4 + 24 * 8];
        data[..4].copy_from_slice(&24u32.to_le_bytes());
        for i in 0..24 {
            data[4 + i * 8..12 + i * 8].copy_from_slice(&(-1i64).to_le_bytes());
        }
        let offset = data.len() as i64;
        data[4 + 21 * 8..12 + 21 * 8].copy_from_slice(&offset.to_le_bytes());
        data.extend_from_slice(table.as_bytes());
        let chars = lstm_characters(&data).unwrap();
        assert_eq!(missing_characters("猫鼬斩", &chars), "鼬");
        assert_eq!(missing_characters("蛇", &chars), "");
        assert!(lstm_characters(&data[..8]).is_err());
        data[4 + 21 * 8..12 + 21 * 8].copy_from_slice(&i64::MAX.to_le_bytes());
        assert!(lstm_characters(&data).is_err());
    }
}
