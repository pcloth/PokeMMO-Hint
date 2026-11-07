/// Fourfold bilinear magnification with a white border. Preserve antialiased strokes
/// instead of dropping every pixel below a fixed RGB threshold.
pub fn ocr_bitmap(pixels: &[u8], width: i32, height: i32) -> Vec<u8> {
    make_bitmap(pixels, width, height, false)
}

/// Alternative thresholded pass, used only when the grayscale read has no match.
pub fn binary_bitmap(pixels: &[u8], width: i32, height: i32) -> Vec<u8> {
    make_bitmap(pixels, width, height, true)
}

fn make_bitmap(pixels: &[u8], width: i32, height: i32, binary: bool) -> Vec<u8> {
    assert!(width > 0 && height > 0 && width <= 2000 && height <= 1000);
    assert_eq!(pixels.len(), width as usize * height as usize * 4);
    const SCALE: usize = 4;
    const BORDER: usize = 12;
    let (iw, ih) = (width as usize, height as usize);
    let gray: Vec<u8> = pixels
        .chunks_exact(4)
        .map(|p| {
            ((77 * u32::from(p[2]) + 150 * u32::from(p[1]) + 29 * u32::from(p[0])) / 256) as u8
        })
        .collect();
    // Most pixels along the crop's border belong to the background.
    let mut border = Vec::new();
    for y in 0..ih {
        for x in 0..iw {
            if x == 0 || y == 0 || x == iw - 1 || y == ih - 1 {
                border.push(gray[y * iw + x]);
            }
        }
    }
    border.sort_unstable();
    let invert = border[border.len() / 2] < 128;
    let gray: Vec<u8> = gray
        .into_iter()
        .map(|g| if invert { 255 - g } else { g })
        .collect();
    let threshold = otsu(&gray);
    let (w, h) = (iw * SCALE + BORDER * 2, ih * SCALE + BORDER * 2);
    let size = 54 + w * h * 4;
    let mut out = vec![255u8; size];
    out[..54].fill(0);
    out[..2].copy_from_slice(b"BM");
    out[2..6].copy_from_slice(&(size as u32).to_le_bytes());
    out[10..14].copy_from_slice(&54u32.to_le_bytes());
    out[14..18].copy_from_slice(&40u32.to_le_bytes());
    out[18..22].copy_from_slice(&(w as i32).to_le_bytes());
    out[22..26].copy_from_slice(&(-(h as i32)).to_le_bytes());
    out[26..28].copy_from_slice(&1u16.to_le_bytes());
    out[28..30].copy_from_slice(&32u16.to_le_bytes());
    out[34..38].copy_from_slice(&((w * h * 4) as u32).to_le_bytes());
    for y in 0..ih * SCALE {
        for x in 0..iw * SCALE {
            let sx = ((x as f64 + 0.5) / SCALE as f64 - 0.5).clamp(0.0, (iw - 1) as f64);
            let sy = ((y as f64 + 0.5) / SCALE as f64 - 0.5).clamp(0.0, (ih - 1) as f64);
            let (x0, y0) = (sx.floor() as usize, sy.floor() as usize);
            let (x1, y1) = ((x0 + 1).min(iw - 1), (y0 + 1).min(ih - 1));
            let (tx, ty) = (sx - x0 as f64, sy - y0 as f64);
            let top = gray[y0 * iw + x0] as f64 * (1.0 - tx) + gray[y0 * iw + x1] as f64 * tx;
            let bottom = gray[y1 * iw + x0] as f64 * (1.0 - tx) + gray[y1 * iw + x1] as f64 * tx;
            let mut value = (top * (1.0 - ty) + bottom * ty).round() as u8;
            if binary {
                value = if value <= threshold { 0 } else { 255 };
            }
            let dest = 54 + ((y + BORDER) * w + x + BORDER) * 4;
            out[dest..dest + 3].fill(value);
            out[dest + 3] = 255;
        }
    }
    out
}

fn otsu(values: &[u8]) -> u8 {
    let mut hist = [0usize; 256];
    for &v in values {
        hist[v as usize] += 1;
    }
    let total = values.len() as f64;
    let sum: f64 = hist
        .iter()
        .enumerate()
        .map(|(v, &n)| v as f64 * n as f64)
        .sum();
    let (mut count, mut partial, mut best, mut threshold) = (0.0, 0.0, -1.0, 128);
    for (value, &n) in hist.iter().enumerate() {
        count += n as f64;
        partial += value as f64 * n as f64;
        if count == 0.0 || count == total {
            continue;
        }
        let delta = partial / count - (sum - partial) / (total - count);
        let variance = count * (total - count) * delta * delta;
        if variance > best {
            best = variance;
            threshold = value as u8;
        }
    }
    threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pixel(bmp: &[u8], x: usize, y: usize) -> u8 {
        let w = i32::from_le_bytes(bmp[18..22].try_into().unwrap()) as usize;
        bmp[54 + (y * w + x) * 4]
    }
    #[test]
    fn keeps_white_padding_and_antialiased_strokes() {
        let mut p = vec![30u8; 5 * 5 * 4];
        for i in 0..5 * 5 {
            p[i * 4 + 3] = 255;
        }
        p[(2 * 5 + 2) * 4..(2 * 5 + 2) * 4 + 3].fill(255);
        let b = ocr_bitmap(&p, 5, 5);
        assert_eq!(&b[..2], b"BM");
        assert_eq!(b.len(), 54 + 44 * 44 * 4);
        assert_eq!(i32::from_le_bytes(b[22..26].try_into().unwrap()), -44);
        assert_eq!(pixel(&b, 0, 0), 255);
        assert!(pixel(&b, 22, 22) < 100);
        assert!(b[54..].chunks_exact(4).any(|p| p[0] > 0 && p[0] < 255));
    }
    #[test]
    fn dark_text_on_light_background_stays_dark() {
        let mut p = vec![245u8; 5 * 5 * 4];
        p[(2 * 5 + 2) * 4..(2 * 5 + 2) * 4 + 3].fill(0);
        let b = ocr_bitmap(&p, 5, 5);
        assert!(pixel(&b, 22, 22) < 100);
        assert!(pixel(&b, 12, 12) > 200);
        let binary = binary_bitmap(&p, 5, 5);
        assert!(binary[54..]
            .chunks_exact(4)
            .all(|p| p[0] == 0 || p[0] == 255));
    }
}
