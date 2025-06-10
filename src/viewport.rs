use std::ops::Range;

#[derive(Debug, Clone, Default)]
pub struct Viewport {
    pub horizontal_offset: usize,
    pub vertical_offset: usize,
    pub visible_width: usize,
    pub visible_height: usize,
    pub sequence_length: usize,
    pub sequence_count: usize,
}

impl Viewport {
    pub fn update_dimensions(&mut self, visible_width: usize, visible_height: usize) {
        self.visible_width = visible_width;
        self.visible_height = visible_height;
    }

    pub fn set_sequence_params(&mut self, sequence_length: usize, sequence_count: usize) {
        self.sequence_length = sequence_length;
        self.sequence_count = sequence_count;
    }

    pub fn horizontal_range(&self) -> Range<usize> {
        let start = self.horizontal_offset;
        if self.sequence_length == 0 {
            start..start
        } else {
            let end = start
                .saturating_add(self.visible_width)
                .min(self.sequence_length);
            start..end
        }
    }

    pub fn vertical_range(&self) -> Range<usize> {
        let start = self.vertical_offset;
        if self.sequence_count == 0 {
            start..start
        } else {
            let end = start
                .saturating_add(self.visible_height)
                .min(self.sequence_count);
            start..end
        }
    }

    pub fn scroll_down(&mut self, amount: usize) {
        let max_scroll = self.sequence_count.saturating_sub(self.visible_height);
        self.vertical_offset = (self.vertical_offset + amount).min(max_scroll);
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.vertical_offset = self.vertical_offset.saturating_sub(amount);
    }

    pub fn scroll_right(&mut self, amount: usize) {
        let max_scroll = self.sequence_length.saturating_sub(self.visible_width);
        self.horizontal_offset = (self.horizontal_offset + amount).min(max_scroll);
    }

    pub fn scroll_left(&mut self, amount: usize) {
        if self.horizontal_offset > 0 {
            self.horizontal_offset = self.horizontal_offset.saturating_sub(amount);
        }
    }

    pub fn jump_to_end(&mut self) {
        self.horizontal_offset = self.sequence_length.saturating_sub(self.visible_width);
    }

    pub fn jump_to_start(&mut self) {
        self.horizontal_offset = 0;
    }

    pub fn jump_to_position(&mut self, position: usize) {
        let max_scroll = self.sequence_length.saturating_sub(self.visible_width);
        self.horizontal_offset = position.min(max_scroll);
    }

    pub fn jump_to_sequence(&mut self, sequence_index: usize) {
        self.vertical_offset = sequence_index.min(self.sequence_count.saturating_sub(1));
    }
    pub fn set_initial_position(&mut self, position: usize) {
        self.horizontal_offset = position.min(self.sequence_length.saturating_sub(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizontal_range() {
        let viewport = Viewport {
            horizontal_offset: 5,
            visible_width: 10,
            sequence_length: 50,
            ..Default::default()
        };
        assert_eq!(viewport.horizontal_range(), 5..15);
    }

    #[test]
    fn test_horozontal_range_at_end() {
        let viewport = Viewport {
            horizontal_offset: 5,
            visible_width: 60,
            sequence_length: 50,
            ..Default::default()
        };
        assert_eq!(viewport.horizontal_range(), 5..50);
    }

    #[test]
    fn test_vertical_range() {
        let viewport = Viewport {
            vertical_offset: 3,
            visible_height: 5,
            sequence_count: 20,
            ..Default::default()
        };
        assert_eq!(viewport.vertical_range(), 3..8);
    }

    #[test]
    fn test_vertical_range_at_end() {
        let viewport = Viewport {
            vertical_offset: 3,
            visible_height: 30,
            sequence_count: 20,
            ..Default::default()
        };
        assert_eq!(viewport.vertical_range(), 3..20);
    }

    #[test]
    fn test_scroll_down_bounds_check() {
        let mut viewport = Viewport {
            vertical_offset: 15,
            visible_height: 5,
            sequence_count: 20,
            ..Default::default()
        };
        viewport.scroll_down(10);
        assert_eq!(viewport.vertical_offset, 15);
    }
    #[test]
    fn test_scroll_up_bounds_check() {
        let mut viewport = Viewport {
            vertical_offset: 2,
            visible_height: 5,
            sequence_count: 20,
            ..Default::default()
        };
        viewport.scroll_up(10);
        assert_eq!(viewport.vertical_offset, 0);
    }
    #[test]
    fn test_scroll_right_bounds_check() {
        let mut viewport = Viewport {
            horizontal_offset: 20,
            visible_width: 10,
            sequence_length: 50,
            ..Default::default()
        };
        viewport.scroll_right(100);
        assert_eq!(viewport.horizontal_offset, 40);
    }

    #[test]
    fn test_scroll_left_bounds_check() {
        let mut viewport = Viewport {
            horizontal_offset: 10,
            visible_width: 10,
            sequence_length: 50,
            ..Default::default()
        };
        viewport.scroll_left(100);
        assert_eq!(viewport.horizontal_offset, 0);
    }

    #[test]
    fn test_jump_to_end() {
        let mut viewport = Viewport {
            horizontal_offset: 5,
            visible_width: 10,
            sequence_length: 50,
            ..Default::default()
        };
        viewport.jump_to_end();
        assert_eq!(viewport.horizontal_offset, 40);
    }

    #[test]
    fn test_jump_to_start() {
        let mut viewport = Viewport {
            horizontal_offset: 5,
            visible_width: 10,
            sequence_length: 50,
            ..Default::default()
        };
        viewport.jump_to_start();
        assert_eq!(viewport.horizontal_offset, 0);
    }

    #[test]
    fn test_jump_to_position() {
        let mut viewport = Viewport {
            horizontal_offset: 5,
            visible_width: 10,
            sequence_length: 50,
            ..Default::default()
        };
        viewport.jump_to_position(20);
        assert_eq!(viewport.horizontal_offset, 20);
    }

    #[test]
    fn test_jump_to_position_out_of_bounds() {
        let mut viewport = Viewport {
            horizontal_offset: 5,
            visible_width: 10,
            sequence_length: 50,
            ..Default::default()
        };
        viewport.jump_to_position(100);
        assert_eq!(viewport.horizontal_offset, 40);
    }

    #[test]
    fn test_jump_to_sequence() {
        let mut viewport = Viewport {
            vertical_offset: 5,
            visible_height: 10,
            sequence_count: 50,
            ..Default::default()
        };
        viewport.jump_to_sequence(20);
        assert_eq!(viewport.vertical_offset, 20);
    }
}
