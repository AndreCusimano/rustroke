//! How a [`crate::Ui`] places the widgets added to it.

/// Alignment along an axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// Left or top.
    #[default]
    Min,
    /// Middle.
    Center,
    /// Right or bottom.
    Max,
}

impl Align {
    /// Offset that aligns something of `size` within `available`.
    /// Never negative: content larger than the space starts at its start.
    pub fn offset(self, available: f32, size: f32) -> f32 {
        let free = (available - size).max(0.0);
        match self {
            Self::Min => 0.0,
            Self::Center => free / 2.0,
            Self::Max => free,
        }
    }
}

/// The direction widgets are added in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    /// Each widget below the previous one.
    #[default]
    TopDown,
    /// Each widget right of the previous one.
    LeftToRight,
    /// Each widget left of the previous one (starting at the right edge).
    RightToLeft,
}

/// Placement rules of a [`crate::Ui`]: the main direction widgets follow,
/// and how they are aligned or stretched across it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layout {
    /// The direction widgets are added in.
    pub direction: Direction,
    /// Alignment across the main direction (horizontal for top-down,
    /// vertical for rows).
    pub cross_align: Align,
    /// Stretch widgets to fill the cross axis (e.g. full-width buttons).
    pub cross_justify: bool,
    /// Stretch widgets to fill the rest of the main axis.
    pub main_justify: bool,
    /// Rows only: start a new line when a widget doesn't fit.
    pub main_wrap: bool,
}

impl Layout {
    /// A column, with widgets aligned horizontally by `cross_align`.
    pub fn top_down(cross_align: Align) -> Self {
        Self {
            direction: Direction::TopDown,
            cross_align,
            ..Self::default()
        }
    }

    /// A row, with widgets aligned vertically by `cross_align`.
    pub fn left_to_right(cross_align: Align) -> Self {
        Self {
            direction: Direction::LeftToRight,
            cross_align,
            ..Self::default()
        }
    }

    /// A row filled from the right edge: the first widget added is the
    /// rightmost one.
    pub fn right_to_left(cross_align: Align) -> Self {
        Self {
            direction: Direction::RightToLeft,
            cross_align,
            ..Self::default()
        }
    }

    /// One widget filling the whole space, with its content centered.
    pub fn centered_and_justified() -> Self {
        Self {
            direction: Direction::TopDown,
            cross_align: Align::Center,
            cross_justify: true,
            main_justify: true,
            main_wrap: false,
        }
    }

    /// Sets [`Layout::cross_justify`].
    pub fn with_cross_justify(self, justify: bool) -> Self {
        Self {
            cross_justify: justify,
            ..self
        }
    }

    /// Sets [`Layout::main_justify`].
    pub fn with_main_justify(self, justify: bool) -> Self {
        Self {
            main_justify: justify,
            ..self
        }
    }

    /// Sets [`Layout::main_wrap`].
    pub fn with_main_wrap(self, wrap: bool) -> Self {
        Self {
            main_wrap: wrap,
            ..self
        }
    }

    /// True for rows (left-to-right or right-to-left).
    pub fn is_horizontal(&self) -> bool {
        self.direction != Direction::TopDown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_offsets() {
        assert_eq!(Align::Min.offset(100.0, 40.0), 0.0);
        assert_eq!(Align::Center.offset(100.0, 40.0), 30.0);
        assert_eq!(Align::Max.offset(100.0, 40.0), 60.0);
        assert_eq!(Align::Max.offset(10.0, 40.0), 0.0);
    }
}
