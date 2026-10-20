//! Map DrawingML preset pattern fills (ECMA-376 Part 1 §20.1.8.47, `ST_PresetPatternVal`)
//! to repeating 8x8 SVG tiles.
//!
//! The tiles are this project's own drawings of what each preset name describes (line
//! direction, spacing, and weight, or a dot density); they are not copied from any
//! renderer and are not verified to be pixel-identical to PowerPoint.
use super::*;

/// Tile edge in pattern pixels; one pattern pixel is one slide unit (1/96 inch).
const TILE: usize = 8;

/// The 8x8 tile of a preset, row by row (bit 7 is the left column), or `None` when the
/// preset is not drawn.
fn tile(preset: &str) -> Option<[u8; TILE]> {
    // Diagonals: `up` rises to the right ("/"), `down` falls to the right ("\").
    let diagonal = |period: usize, width: usize, up: bool| {
        cells(|x, y| {
            let step = if up { x + y } else { x + TILE - 1 - y };
            (step + 1) % period < width
        })
    };
    let horizontal = |period: usize, width: usize| cells(|_, y| y % period < width);
    let vertical = |period: usize, width: usize| cells(|x, _| x % period < width);
    let grid = |period: usize| cells(|x, y| x.is_multiple_of(period) || y.is_multiple_of(period));
    let checker = |size: usize| cells(|x, y| (x / size + y / size).is_multiple_of(2));
    let percent = |value: usize| cells(|x, y| BAYER[y][x] * 100 < value * 64);
    Some(match preset {
        "pct5" => percent(5),
        "pct10" => percent(10),
        "pct20" => percent(20),
        "pct25" => percent(25),
        "pct30" => percent(30),
        "pct40" => percent(40),
        "pct50" => percent(50),
        "pct60" => percent(60),
        "pct70" => percent(70),
        "pct75" => percent(75),
        "pct80" => percent(80),
        "pct90" => percent(90),
        "horz" => horizontal(8, 1),
        "ltHorz" => horizontal(4, 1),
        "narHorz" => horizontal(2, 1),
        "dkHorz" => horizontal(4, 2),
        "vert" => vertical(8, 1),
        "ltVert" => vertical(4, 1),
        "narVert" => vertical(2, 1),
        "dkVert" => vertical(4, 2),
        "upDiag" => diagonal(8, 1, true),
        "ltUpDiag" => diagonal(4, 1, true),
        "dkUpDiag" => diagonal(4, 2, true),
        "wdUpDiag" => diagonal(8, 3, true),
        "dnDiag" => diagonal(8, 1, false),
        "ltDnDiag" => diagonal(4, 1, false),
        "dkDnDiag" => diagonal(4, 2, false),
        "wdDnDiag" => diagonal(8, 3, false),
        "cross" | "lgGrid" => grid(8),
        "smGrid" => grid(4),
        "diagCross" => {
            let (up, down) = (diagonal(8, 1, true), diagonal(8, 1, false));
            std::array::from_fn(|row| up[row] | down[row])
        }
        // A dense lattice of two-pixel diagonals in both directions.
        "trellis" => {
            let (up, down) = (diagonal(4, 2, true), diagonal(4, 2, false));
            std::array::from_fn(|row| up[row] | down[row])
        }
        "smCheck" => checker(2),
        "lgCheck" => checker(4),
        _ => return None,
    })
}
/// 8x8 ordered-dither thresholds (0..64); a cell is set when its threshold is below the
/// requested share of 64.
const BAYER: [[usize; TILE]; TILE] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];
fn cells(set: impl Fn(usize, usize) -> bool) -> [u8; TILE] {
    std::array::from_fn(|y| {
        (0..TILE)
            .filter(|&x| set(x, y))
            .fold(0u8, |row, x| row | (0x80 >> x))
    })
}
/// Add a pattern definition for `a:pattFill` to `svg` and return its paint reference.
pub(super) fn define(
    node: Node<'_, '_>,
    theme: &Theme,
    svg: &mut String,
    warnings: &mut Warnings,
) -> String {
    let preset = node.attribute("prst").unwrap_or("pct5");
    let rows = tile(preset).unwrap_or_else(|| {
        warn(
            warnings,
            format!("Unsupported pattern fill preset: {preset}; drawn as pct50"),
        );
        tile("pct50").expect("pct50 is a drawn preset")
    });
    warnings.push(
        Code::Approximation,
        "Pattern fills are drawn with this renderer's own 8x8 tiles, one tile pixel per 1/96 inch",
    );
    // ECMA-376 defaults: black foreground, white background.
    let foreground = child(node, A, "fgClr")
        .map(|n| color(n, "#000000", theme, warnings))
        .unwrap_or_else(|| "#000000".into());
    let background = child(node, A, "bgClr")
        .map(|n| color(n, "#FFFFFF", theme, warnings))
        .unwrap_or_else(|| "#FFFFFF".into());
    let mut path = String::new();
    for (y, row) in rows.iter().enumerate() {
        let mut x = 0;
        while x < TILE {
            if row & (0x80 >> x) == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < TILE && row & (0x80 >> x) != 0 {
                x += 1;
            }
            path.push_str(&format!("M{start} {y}h{}v1h-{}z", x - start, x - start));
        }
    }
    let id = format!("pattern-{}", svg.len());
    svg.push_str(&format!(
        "<defs><pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" width=\"{TILE}\" height=\"{TILE}\"><rect width=\"{TILE}\" height=\"{TILE}\" fill=\"{background}\"/><path d=\"{path}\" fill=\"{foreground}\"/></pattern></defs>"
    ));
    format!("url(#{id})")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn count(rows: [u8; TILE]) -> u32 {
        rows.iter().map(|r| r.count_ones()).sum()
    }
    #[test]
    fn diagonals_run_in_their_named_direction() {
        let up = tile("upDiag").unwrap();
        // "/" : the top row is set at the right edge, the bottom row at the left edge.
        assert_eq!(up[0], 0b0000_0001);
        assert_eq!(up[7], 0b1000_0000);
        let down = tile("dnDiag").unwrap();
        assert_eq!(down[0], 0b1000_0000);
        assert_eq!(down[7], 0b0000_0001);
        assert_eq!(count(tile("dkUpDiag").unwrap()), 32);
        assert_eq!(count(tile("ltUpDiag").unwrap()), 16);
    }
    #[test]
    fn percent_presets_set_their_share_of_cells() {
        for (preset, cells) in [("pct5", 4), ("pct25", 16), ("pct50", 32), ("pct90", 58)] {
            assert_eq!(count(tile(preset).unwrap()), cells, "{preset}");
        }
        assert_eq!(tile("smCheck").unwrap()[0], 0b1100_1100);
    }
    #[test]
    fn unknown_preset_is_reported_and_drawn_as_half_tone() {
        let source = format!(
            r#"<a:pattFill xmlns:a="{A}" prst="weave"><a:fgClr><a:srgbClr val="FF0000"/></a:fgClr></a:pattFill>"#
        );
        let doc = Document::parse(&source).unwrap();
        let mut warnings = Warnings::default();
        let mut svg = String::new();
        let paint = define(
            doc.root_element(),
            &Theme::default(),
            &mut svg,
            &mut warnings,
        );
        assert_eq!(paint, "url(#pattern-0)");
        assert!(svg.contains("fill=\"#FF0000\"") && svg.contains("fill=\"#FFFFFF\""));
        assert_eq!(
            warnings.as_slice(),
            [
                "Unsupported pattern fill preset: weave; drawn as pct50",
                "Pattern fills are drawn with this renderer's own 8x8 tiles, one tile pixel per 1/96 inch"
            ]
        );
    }
}
