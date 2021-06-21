use cassowary::strength::{REQUIRED, STRONG, WEAK};
use cassowary::WeightedRelation::*;
use cassowary::{Solver, Variable};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::Style;
use tui::widgets::Widget;

pub struct VerticalBar {
    pub window_index: usize,
    pub window_length: usize,
    pub total_length: usize,
    pub style: Style,
}

impl Widget for VerticalBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let scaling: f64 = f64::from(area.height) / self.total_length as f64;
        let scaled_window_length =
            (scaling * std::cmp::min(self.window_length, self.total_length) as f64).round() as u16;
        let window_offset = (scaling * self.window_index as f64) as u16;
        debug_assert!(
            window_offset < area.height,
            "{} < {} (scaling was {} from {} / {}",
            window_offset,
            area.height,
            scaling,
            area.height,
            self.total_length,
        );
        debug_assert!(
            window_offset + scaled_window_length <= area.height,
            "{} + {} <= {} (scaling was {} from {} / {}, window_length: {}, window_index: {})",
            window_offset,
            scaled_window_length,
            area.height,
            scaling,
            f64::from(area.height),
            self.total_length,
            self.window_length,
            self.window_index,
        );
        let window_area = Rect::new(
            area.x,
            area.y + window_offset,
            area.width,
            scaled_window_length,
        );
        buf.set_style(window_area, self.style);
    }
}

// bounds consists of pairs of variables representing left and right position of the column
pub fn commit_list_column_width_solver(bounds: &[Variable], window_width: &Variable) -> Solver {
    let mut solver = Solver::new();
    solver
        .add_constraints(&[
            *window_width | GE(REQUIRED) | 0.0, // positive window width
            bounds[0] | EQ(REQUIRED) | 0.0,     // left align
            bounds[3] | EQ(REQUIRED) | bounds[4] - 1.0, // right align
            bounds[5] | EQ(REQUIRED) | *window_width, // right align
            bounds[2] | GE(REQUIRED) | bounds[1], // no overlap
            bounds[4] | GE(REQUIRED) | bounds[3], // no overlap
            // positive widths
            bounds[0] | LE(REQUIRED) | bounds[1],
            bounds[2] | LE(REQUIRED) | bounds[3],
            bounds[4] | LE(REQUIRED) | bounds[5],
            // preferred widths:
            bounds[1] - bounds[0] | EQ(WEAK) | *window_width * (72.0 / 100.0),
            bounds[3] - bounds[2] | EQ(WEAK) | *window_width * (18.0 / 100.0),
            bounds[5] - bounds[4] | EQ(WEAK) | *window_width * (9.0 / 100.0),
            // constrain some columns to a range:
            bounds[3] - bounds[2] | LE(REQUIRED) | 40.0,
            bounds[3] - bounds[2] | GE(STRONG) | 20.0,
            bounds[5] - bounds[4] | LE(REQUIRED) | 15.0,
            bounds[5] - bounds[4] | GE(STRONG) | 10.0,
            // require one column to have a minimum size
            bounds[1] - bounds[0] | GE(STRONG) | 50.0,
            // fixed length
            //box1.right - box1.left | EQ(WEAK) | 79.0,
            //box2.right - box2.left | EQ(WEAK) | 20.0,
            //box3.right - box3.left | EQ(WEAK) | 10.0,
        ])
        .unwrap();
    solver
        .add_edit_variable(*window_width, STRONG)
        .expect("Unable to add edit variable");

    solver
}

pub fn solver_changes_to_lengths(
    solver: &Solver,
    bounds: &[Variable],
) -> Vec<tui::layout::Constraint> {
    let widths: Vec<_> = bounds
        .windows(2)
        .map(|bounds| solver.get_value(bounds[1]) - solver.get_value(bounds[0]))
        .collect();
    vec![
        tui::layout::Constraint::Length((widths[0] + widths[1]) as u16),
        tui::layout::Constraint::Length((widths[2] + widths[3]) as u16),
        tui::layout::Constraint::Length((widths[4]) as u16),
    ]
}
