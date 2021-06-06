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
