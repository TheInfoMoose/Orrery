use ratatui::layout::{Constraint, Layout, Rect};

pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;
pub const BANNER_MIN_HEIGHT: u16 = 40;
pub const MIN_STAGE_COL_WIDTH: u16 = 18;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiRegion {
    IngestButton,
    ModeToggle,
    CyclesControl,
    PassedCounter,
    StagesMinus,
    StagesPlus,
    StepsList,
    StageHeader(usize),
    AgentSlot { stage: usize, slot: usize },
    LiveStream,
    RunStopButton,
    ApproveButton,
    RejectButton,
    SettingsButton,
    HelpButton,
    MouseToggle,
    QuitButton,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageColumn {
    pub index: usize,
    pub area: Rect,
    pub header: Rect,
    pub slots: Vec<(usize, Rect)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiLayoutRegions {
    pub too_small: bool,
    pub header: Rect,
    pub top_menu: Rect,
    pub main_workspace: Rect,
    pub steps_panel: Rect,
    pub stage_canvas: Rect,
    pub live_stream: Rect,
    pub footer_menu: Rect,
    pub top_items: Vec<(UiRegion, Rect)>,
    pub footer_items: Vec<(UiRegion, Rect)>,
    pub stage_columns: Vec<StageColumn>,
}

const TOP_MENU_SPEC: &[(Option<UiRegion>, &str)] = &[
    (Some(UiRegion::IngestButton), "[Ingest]"),
    (Some(UiRegion::ModeToggle), "[Mode: LOOP]"),
    (Some(UiRegion::CyclesControl), "[Cycles: 999]"),
    (Some(UiRegion::PassedCounter), "[Passed: 999/999]"),
    (None, "[Stages:"),
    (Some(UiRegion::StagesMinus), "[-]"),
    (None, " 15 "),
    (Some(UiRegion::StagesPlus), "[+]"),
    (None, "]"),
];

const FOOTER_MENU_SPEC: &[(UiRegion, &str)] = &[
    (UiRegion::RunStopButton, "[Stop]"),
    (UiRegion::ApproveButton, "[Approve]"),
    (UiRegion::RejectButton, "[Reject]"),
    (UiRegion::SettingsButton, "[Settings]"),
    (UiRegion::HelpButton, "[Help]"),
    (UiRegion::MouseToggle, "[Mouse: OFF]"),
    (UiRegion::QuitButton, "[Quit]"),
];

fn inner_rect(rect: Rect) -> Rect {
    if rect.width < 2 || rect.height < 2 {
        return Rect::default();
    }
    Rect::new(rect.x + 1, rect.y + 1, rect.width - 2, rect.height - 2)
}

fn contains(rect: Rect, col: u16, row: u16) -> bool {
    rect.width > 0
        && rect.height > 0
        && col >= rect.x
        && col < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

fn compute_top_items(top_menu: Rect) -> Vec<(UiRegion, Rect)> {
    let inner = inner_rect(top_menu);
    if inner.height == 0 {
        return Vec::new();
    }
    let y = inner.y;
    let right_bound = inner.x.saturating_add(inner.width);
    let mut x = inner.x;
    let mut items = Vec::new();
    for &(region, label) in TOP_MENU_SPEC {
        let w = u16::try_from(label.len()).unwrap_or(u16::MAX);
        if x >= right_bound {
            break;
        }
        let avail_w = w.min(right_bound - x);
        if let Some(r) = region {
            items.push((r, Rect::new(x, y, avail_w, 1)));
        }
        x = x.saturating_add(w).saturating_add(1);
    }
    items
}

fn compute_footer_items(footer_menu: Rect) -> Vec<(UiRegion, Rect)> {
    let inner = inner_rect(footer_menu);
    if inner.height == 0 {
        return Vec::new();
    }
    let y = inner.y;
    let right_bound = inner.x.saturating_add(inner.width);
    let mut x = inner.x;
    let mut items = Vec::new();
    for &(region, label) in FOOTER_MENU_SPEC {
        let w = u16::try_from(label.len()).unwrap_or(u16::MAX);
        if x >= right_bound {
            break;
        }
        let avail_w = w.min(right_bound - x);
        items.push((region, Rect::new(x, y, avail_w, 1)));
        x = x.saturating_add(w).saturating_add(1);
    }
    items
}

fn compute_stage_columns(
    stage_canvas: Rect,
    stage_count: usize,
    stage_scroll: usize,
    slot_scroll: usize,
) -> Vec<StageColumn> {
    let inner = inner_rect(stage_canvas);
    if inner.width == 0 || inner.height == 0 || stage_scroll >= stage_count {
        return Vec::new();
    }
    let remaining_stages = stage_count - stage_scroll;
    let max_by_width = usize::from((inner.width / MIN_STAGE_COL_WIDTH).max(1));
    let visible = remaining_stages.min(max_by_width);
    let visible_u16 = u16::try_from(visible).unwrap_or(1);
    let base_w = inner.width / visible_u16;

    let mut columns = Vec::with_capacity(visible);
    let mut col_x = inner.x;
    for i in 0..visible {
        let stage_idx = stage_scroll + i;
        let col_w = if i + 1 == visible {
            (inner.x + inner.width).saturating_sub(col_x)
        } else {
            base_w
        };
        let col_area = Rect::new(col_x, inner.y, col_w, inner.height);
        let header_h = col_area.height.min(3);
        let header = Rect::new(col_area.x, col_area.y, col_area.width, header_h);

        let mut slots = Vec::new();
        let slots_start_y = col_area.y.saturating_add(header_h);
        let avail_slot_rows = usize::from(col_area.height.saturating_sub(header_h));
        for row_offset in 0..avail_slot_rows {
            let slot_idx = slot_scroll.saturating_add(row_offset);
            if slot_idx >= 15 {
                break;
            }
            let row_offset_u16 = u16::try_from(row_offset).unwrap_or(0);
            let slot_rect = Rect::new(
                col_area.x,
                slots_start_y.saturating_add(row_offset_u16),
                col_area.width,
                1,
            );
            slots.push((slot_idx, slot_rect));
        }

        columns.push(StageColumn {
            index: stage_idx,
            area: col_area,
            header,
            slots,
        });
        col_x = col_x.saturating_add(col_w);
    }
    columns
}

impl UiLayoutRegions {
    pub fn compute(
        area: Rect,
        stage_count: usize,
        stage_scroll: usize,
        slot_scroll: usize,
    ) -> Self {
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return Self {
                too_small: true,
                ..Self::default()
            };
        }

        let header_len = if area.height >= BANNER_MIN_HEIGHT {
            11
        } else {
            3
        };

        let [header, top_menu, main_workspace, live_stream, footer_menu] = Layout::vertical([
            Constraint::Length(header_len),
            Constraint::Length(3),
            Constraint::Fill(3),
            Constraint::Fill(2),
            Constraint::Length(3),
        ])
        .areas(area);

        let [steps_panel, stage_canvas] =
            Layout::horizontal([Constraint::Percentage(25), Constraint::Fill(1)])
                .areas(main_workspace);

        let top_items = compute_top_items(top_menu);
        let footer_items = compute_footer_items(footer_menu);
        let stage_columns =
            compute_stage_columns(stage_canvas, stage_count, stage_scroll, slot_scroll);

        Self {
            too_small: false,
            header,
            top_menu,
            main_workspace,
            steps_panel,
            stage_canvas,
            live_stream,
            footer_menu,
            top_items,
            footer_items,
            stage_columns,
        }
    }

    pub fn region_at(&self, col: u16, row: u16) -> Option<UiRegion> {
        if self.too_small {
            return None;
        }
        for &(region, rect) in &self.top_items {
            if contains(rect, col, row) {
                return Some(region);
            }
        }
        for &(region, rect) in &self.footer_items {
            if contains(rect, col, row) {
                return Some(region);
            }
        }
        for stage_col in &self.stage_columns {
            if contains(stage_col.header, col, row) {
                return Some(UiRegion::StageHeader(stage_col.index));
            }
            for &(slot_idx, slot_rect) in &stage_col.slots {
                if contains(slot_rect, col, row) {
                    return Some(UiRegion::AgentSlot {
                        stage: stage_col.index,
                        slot: slot_idx,
                    });
                }
            }
        }
        if contains(inner_rect(self.steps_panel), col, row) {
            return Some(UiRegion::StepsList);
        }
        if contains(inner_rect(self.live_stream), col, row) {
            return Some(UiRegion::LiveStream);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disjoint(a: Rect, b: Rect) -> bool {
        if a.width == 0 || a.height == 0 || b.width == 0 || b.height == 0 {
            return true;
        }
        let a_right = a.x + a.width;
        let a_bottom = a.y + a.height;
        let b_right = b.x + b.width;
        let b_bottom = b.y + b.height;
        a_right <= b.x || b_right <= a.x || a_bottom <= b.y || b_bottom <= a.y
    }

    fn assert_four_corners(regions: &UiLayoutRegions, rect: Rect, expected: UiRegion) {
        assert!(rect.width > 0 && rect.height > 0);
        let x0 = rect.x;
        let x1 = rect.x + rect.width - 1;
        let y0 = rect.y;
        let y1 = rect.y + rect.height - 1;
        for (col, row) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
            assert_eq!(
                regions.region_at(col, row),
                Some(expected),
                "corner ({col}, {row}) of {rect:?}"
            );
        }
    }

    #[test]
    fn too_small_below_80x24() {
        for (w, h) in [(79, 24), (80, 23)] {
            let r = UiLayoutRegions::compute(Rect::new(0, 0, w, h), 4, 0, 0);
            assert!(r.too_small);
            assert_eq!(r.header, Rect::default());
            assert_eq!(r.top_menu, Rect::default());
            assert_eq!(r.main_workspace, Rect::default());
            assert_eq!(r.steps_panel, Rect::default());
            assert_eq!(r.stage_canvas, Rect::default());
            assert_eq!(r.live_stream, Rect::default());
            assert_eq!(r.footer_menu, Rect::default());
            assert!(r.top_items.is_empty());
            assert!(r.footer_items.is_empty());
            assert!(r.stage_columns.is_empty());
            assert_eq!(r.region_at(10, 10), None);
        }
    }

    #[test]
    fn rows_cover_height_80x24() {
        let r = UiLayoutRegions::compute(Rect::new(0, 0, 80, 24), 4, 0, 0);
        assert!(!r.too_small);
        assert_eq!(r.header.height, 3);
        assert_eq!(r.top_menu.height, 3);
        assert_eq!(r.main_workspace.height, 9);
        assert_eq!(r.live_stream.height, 6);
        assert_eq!(r.footer_menu.height, 3);
        let sum = r.header.height
            + r.top_menu.height
            + r.main_workspace.height
            + r.live_stream.height
            + r.footer_menu.height;
        assert_eq!(sum, 24);
    }

    #[test]
    fn rows_cover_height_at_120x40_and_200x60() {
        for (w, h) in [(120, 40), (200, 60)] {
            let r = UiLayoutRegions::compute(Rect::new(0, 0, w, h), 4, 0, 0);
            assert!(!r.too_small);
            assert_eq!(r.header.height, 11);
            let sum = r.header.height
                + r.top_menu.height
                + r.main_workspace.height
                + r.live_stream.height
                + r.footer_menu.height;
            assert_eq!(sum, h);
        }
    }

    #[test]
    fn header_switches_at_40() {
        let r39 = UiLayoutRegions::compute(Rect::new(0, 0, 80, 39), 4, 0, 0);
        let r40 = UiLayoutRegions::compute(Rect::new(0, 0, 80, 40), 4, 0, 0);
        assert_eq!(r39.header.height, 3);
        assert_eq!(r40.header.height, 11);
    }

    #[test]
    fn regions_do_not_overlap() {
        for (w, h) in [(80, 24), (120, 40), (200, 60)] {
            let r = UiLayoutRegions::compute(Rect::new(0, 0, w, h), 4, 0, 0);
            let verticals = [
                r.header,
                r.top_menu,
                r.main_workspace,
                r.live_stream,
                r.footer_menu,
            ];
            for i in 0..verticals.len() {
                for j in (i + 1)..verticals.len() {
                    assert!(
                        disjoint(verticals[i], verticals[j]),
                        "vertical overlap at {w}x{h}: {i} vs {j}"
                    );
                }
            }
            assert!(
                disjoint(r.steps_panel, r.stage_canvas),
                "steps_panel and stage_canvas overlap at {w}x{h}"
            );
        }
    }

    #[test]
    fn fifteen_stages_scroll_horizontally() {
        let stage_scroll = 5;
        let r = UiLayoutRegions::compute(Rect::new(0, 0, 80, 24), 15, stage_scroll, 0);
        let inner_w = r.stage_canvas.width - 2;
        let visible_count = u16::try_from(r.stage_columns.len()).unwrap();
        assert!(visible_count > 0);
        assert!(visible_count * MIN_STAGE_COL_WIDTH <= inner_w);
        assert_eq!(r.stage_columns[0].index, stage_scroll);
        let total_col_w: u16 = r.stage_columns.iter().map(|c| c.area.width).sum();
        assert_eq!(total_col_w, inner_w);
    }

    #[test]
    fn slots_scroll_vertically() {
        let slot_scroll = 6;
        let r = UiLayoutRegions::compute(Rect::new(0, 0, 80, 24), 4, 0, slot_scroll);
        let col = &r.stage_columns[0];
        assert!(!col.slots.is_empty());
        assert!(col.slots.len() < 15);
        assert_eq!(col.slots[0].0, slot_scroll);
    }

    #[test]
    fn hit_test_corners() {
        let r = UiLayoutRegions::compute(Rect::new(0, 0, 120, 40), 4, 0, 0);
        assert_eq!(r.top_items.len(), 6);
        assert_eq!(r.footer_items.len(), 7);
        for &(region, rect) in &r.top_items {
            assert_four_corners(&r, rect, region);
        }
        for &(region, rect) in &r.footer_items {
            assert_four_corners(&r, rect, region);
        }
        for stage_col in &r.stage_columns {
            assert_four_corners(&r, stage_col.header, UiRegion::StageHeader(stage_col.index));
            for &(slot_idx, slot_rect) in &stage_col.slots {
                assert_four_corners(
                    &r,
                    slot_rect,
                    UiRegion::AgentSlot {
                        stage: stage_col.index,
                        slot: slot_idx,
                    },
                );
            }
        }
        assert_four_corners(&r, inner_rect(r.steps_panel), UiRegion::StepsList);
        assert_four_corners(&r, inner_rect(r.live_stream), UiRegion::LiveStream);
    }

    #[test]
    fn hit_test_outside_is_none() {
        let r = UiLayoutRegions::compute(Rect::new(0, 0, 120, 40), 4, 0, 0);
        // Border cells of top_menu, footer_menu, steps_panel, stage_canvas, live_stream, and header
        assert_eq!(r.region_at(r.header.x, r.header.y), None);
        assert_eq!(r.region_at(r.top_menu.x, r.top_menu.y), None);
        assert_eq!(r.region_at(r.steps_panel.x, r.steps_panel.y), None);
        assert_eq!(r.region_at(r.stage_canvas.x, r.stage_canvas.y), None);
        assert_eq!(r.region_at(r.live_stream.x, r.live_stream.y), None);
        assert_eq!(r.region_at(r.footer_menu.x, r.footer_menu.y), None);
    }

    #[test]
    fn top_menu_fits_at_80_columns() {
        let r = UiLayoutRegions::compute(Rect::new(0, 0, 80, 24), 4, 0, 0);
        let expected_labels: Vec<(UiRegion, &str)> = TOP_MENU_SPEC
            .iter()
            .filter_map(|&(region, label)| region.map(|reg| (reg, label)))
            .collect();
        assert_eq!(r.top_items.len(), expected_labels.len());
        for ((region, rect), (exp_region, label)) in r.top_items.iter().zip(expected_labels.iter())
        {
            assert_eq!(region, exp_region);
            assert_eq!(usize::from(rect.width), label.len());
        }
        let stages_plus_rect = r
            .top_items
            .iter()
            .find_map(|&(region, rect)| (region == UiRegion::StagesPlus).then_some(rect))
            .unwrap();
        let top_menu_inner_right = r.top_menu.x + r.top_menu.width - 1;
        assert!(stages_plus_rect.x + stages_plus_rect.width <= top_menu_inner_right);
    }
}
