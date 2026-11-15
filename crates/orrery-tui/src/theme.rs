use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

pub const BG_BLACK: Color = Color::Rgb(0, 0, 0);
pub const PANEL_BG: Color = Color::Rgb(12, 8, 0);
pub const AMBER_PRIMARY: Color = Color::Rgb(255, 176, 0);
pub const AMBER_BRIGHT: Color = Color::Rgb(255, 216, 102);
pub const AMBER_DIM: Color = Color::Rgb(153, 99, 0);
pub const AMBER_DARK: Color = Color::Rgb(92, 58, 0);
pub const AMBER_ALERT: Color = Color::Rgb(255, 85, 0);

pub const BANNER: [&str; 9] = [
    r"            .       o            .                *           .",
    r"        _.-~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~-._",
    r"     .-~      ___   ____   ____   _____  ____  __   __            ~-.",
    r"   .~        / _ \ |  _ \ |  _ \ | ____||  _ \ \ \ / /    .-.        ~.",
    r"  @         | | | || |_) || |_) ||  _|  | |_) | \ V /    ( * )         )",
    r"   '.       | |_| ||  _ < |  _ < | |___ |  _ <   | |      '-'        .'",
    r"     '-.     \___/ |_| \_\|_| \_\|_____||_| \_\  |_|              .-'",
    r"        '-.____________________________________________________.-O",
    r"     .         *             .        o              .",
];

pub const COMPACT_HEADER: &str = " -(*)--o---@----O-   O R R E R Y  //  WORKFLOW ORCHESTRATOR";

pub fn banner_line(row: usize) -> Line<'static> {
    let Some(&text) = BANNER.get(row) else {
        return Line::default();
    };
    let row_1 = row + 1;
    let spans: Vec<Span<'static>> = text
        .char_indices()
        .map(|(col_0, ch)| {
            let col_1 = col_0 + 1;
            let fg = if row_1 == 1 || row_1 == 9 {
                AMBER_DARK
            } else if (3..=7).contains(&row_1) && (13..=54).contains(&col_1) {
                AMBER_PRIMARY
            } else if matches!(ch, '@' | 'O' | '*' | '(' | ')') {
                AMBER_BRIGHT
            } else {
                AMBER_DIM
            };
            let slice = &text[col_0..col_0 + ch.len_utf8()];
            Span::styled(slice, Style::default().fg(fg).bg(BG_BLACK))
        })
        .collect();
    Line::from(spans)
}

pub fn base_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(AMBER_PRIMARY).bg(BG_BLACK))
        .border_style(Style::default().fg(AMBER_DIM).bg(BG_BLACK))
}

pub fn focused_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(AMBER_PRIMARY).bg(PANEL_BG))
        .border_style(Style::default().fg(AMBER_PRIMARY).bg(PANEL_BG))
}

pub fn alert_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .style(Style::default().fg(AMBER_ALERT).bg(PANEL_BG))
        .border_style(Style::default().fg(AMBER_ALERT).bg(PANEL_BG))
}

pub fn muted() -> Style {
    Style::default().fg(AMBER_DIM).bg(BG_BLACK)
}

pub fn highlight() -> Style {
    Style::default()
        .fg(AMBER_BRIGHT)
        .bg(BG_BLACK)
        .add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span_at(row_1_based: usize, col_1_based: usize) -> Span<'static> {
        let line = banner_line(row_1_based - 1);
        line.spans[col_1_based - 1].clone()
    }

    #[test]
    fn palette_rgb_values() {
        assert_eq!(BG_BLACK, Color::Rgb(0, 0, 0));
        assert_eq!(PANEL_BG, Color::Rgb(12, 8, 0));
        assert_eq!(AMBER_PRIMARY, Color::Rgb(255, 176, 0));
        assert_eq!(AMBER_BRIGHT, Color::Rgb(255, 216, 102));
        assert_eq!(AMBER_DIM, Color::Rgb(153, 99, 0));
        assert_eq!(AMBER_DARK, Color::Rgb(92, 58, 0));
        assert_eq!(AMBER_ALERT, Color::Rgb(255, 85, 0));

        assert_eq!(muted().fg, Some(AMBER_DIM));
        assert_eq!(muted().bg, Some(BG_BLACK));
        assert_eq!(highlight().fg, Some(AMBER_BRIGHT));
        assert_eq!(highlight().bg, Some(BG_BLACK));
        assert!(highlight().add_modifier.contains(Modifier::BOLD));

        let _ = base_block();
        let _ = focused_block();
        let _ = alert_block();
        assert!(COMPACT_HEADER.contains("O R R E R Y"));
    }

    #[test]
    fn banner_shape() {
        assert_eq!(BANNER.len(), 9);
        for row in BANNER {
            assert!(row.is_ascii());
            assert!(row.len() <= 72);
        }
        assert!(banner_line(9).spans.is_empty());
    }

    #[test]
    fn banner_matches_spec_block() {
        let spec = include_str!("../../../ORRERY_TECHNICAL_SPECIFICATION.md");
        let mut lines = spec.lines();
        for line in lines.by_ref() {
            if line.starts_with("Full banner (9 rows") {
                break;
            }
        }
        for line in lines.by_ref() {
            if line == "```text" {
                break;
            }
        }
        let spec_banner: Vec<&str> = lines.take(9).collect();
        assert_eq!(spec_banner.len(), 9);
        assert_eq!(spec_banner.as_slice(), &BANNER);
    }

    #[test]
    fn banner_letter_cell_is_primary() {
        let span = span_at(3, 15);
        assert_eq!(span.content.as_ref(), "_");
        assert_eq!(span.style.fg, Some(AMBER_PRIMARY));
    }

    #[test]
    fn banner_planet_cell_is_bright() {
        let span = span_at(5, 3);
        assert_eq!(span.content.as_ref(), "@");
        assert_eq!(span.style.fg, Some(AMBER_BRIGHT));
    }

    #[test]
    fn banner_orbit_cell_is_dim() {
        let span = span_at(2, 13);
        assert_eq!(span.content.as_ref(), "~");
        assert_eq!(span.style.fg, Some(AMBER_DIM));
    }

    #[test]
    fn banner_star_rows_are_dark() {
        for row_1 in [1, 9] {
            let line = banner_line(row_1 - 1);
            assert!(!line.spans.is_empty());
            for span in &line.spans {
                assert_eq!(span.style.fg, Some(AMBER_DARK));
            }
        }
    }
}
