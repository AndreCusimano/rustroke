//! Charts: [`Plot`] draws data on an x/y plane (lines, points, areas,
//! bars, histograms, heat maps, reference lines, text) with axes, ticks,
//! grid, legend, panning, zooming and a hover tooltip; [`PieChart`] draws
//! pie and donut charts.

use std::borrow::Cow;
use std::hash::Hash;
use std::ops::RangeInclusive;
use std::sync::Arc;

use rustroke_core::{
    Color, DisplayList, Galley, Point, PointerButton, Rect, Stroke, Transform, Vec2, point, vec2,
};
use rustroke_text::TextStyle;

use crate::{Id, Response, Sense, Ui, WidgetInfo, WidgetRole};

// ---------------------------------------------------------------------------
// Data

/// The points of a series, borrowed or owned: `[x, y]` pairs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlotPoints<'a>(Cow<'a, [[f64; 2]]>);

impl<'a> PlotPoints<'a> {
    /// The points as a slice.
    pub fn as_slice(&self) -> &[[f64; 2]] {
        &self.0
    }

    /// Points at x = 0, 1, 2… with these y values.
    pub fn from_ys(ys: &[f64]) -> PlotPoints<'static> {
        PlotPoints(Cow::Owned(
            ys.iter().enumerate().map(|(i, y)| [i as f64, *y]).collect(),
        ))
    }

    /// `count` points of `f(x)` evenly spread over `x` (a function graph).
    pub fn from_fn(
        x: RangeInclusive<f64>,
        count: usize,
        f: impl Fn(f64) -> f64,
    ) -> PlotPoints<'static> {
        let (a, b) = (*x.start(), *x.end());
        let n = count.max(2);
        PlotPoints(Cow::Owned(
            (0..n)
                .map(|i| {
                    let x = a + (b - a) * i as f64 / (n - 1) as f64;
                    [x, f(x)]
                })
                .collect(),
        ))
    }
}

impl From<Vec<[f64; 2]>> for PlotPoints<'static> {
    fn from(points: Vec<[f64; 2]>) -> Self {
        Self(Cow::Owned(points))
    }
}

impl<'a> From<&'a [[f64; 2]]> for PlotPoints<'a> {
    fn from(points: &'a [[f64; 2]]) -> Self {
        Self(Cow::Borrowed(points))
    }
}

impl<'a> From<&'a Vec<[f64; 2]>> for PlotPoints<'a> {
    fn from(points: &'a Vec<[f64; 2]>) -> Self {
        Self(Cow::Borrowed(points))
    }
}

impl<'a, const N: usize> From<&'a [[f64; 2]; N]> for PlotPoints<'a> {
    fn from(points: &'a [[f64; 2]; N]) -> Self {
        Self(Cow::Borrowed(points))
    }
}

impl<'a> From<&'a PlotBuffer> for PlotPoints<'a> {
    fn from(buffer: &'a PlotBuffer) -> Self {
        buffer.points()
    }
}

/// The latest points of a live series: pushing beyond the capacity drops
/// the oldest. Keep it in the app state, push new samples as they arrive
/// and plot it (with [`Plot::follow_x`] for a scrolling view).
///
/// ```ignore
/// self.temperature.push(time, reading);
/// Plot::new("live").follow_x(30.0).show(ui, |plot| {
///     plot.line(Line::new(&self.temperature).name("Temperature"));
/// });
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct PlotBuffer {
    data: Vec<[f64; 2]>,
    start: usize,
    capacity: usize,
}

impl PlotBuffer {
    /// Keeps the last `capacity` points.
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity.max(1) * 2),
            start: 0,
            capacity: capacity.max(1),
        }
    }

    /// Adds a point, dropping the oldest one when full.
    pub fn push(&mut self, x: f64, y: f64) {
        self.data.push([x, y]);
        if self.data.len() - self.start > self.capacity {
            self.start += 1;
        }
        // Compact now and then, so pushing stays cheap.
        if self.start >= self.capacity {
            self.data.drain(..self.start);
            self.start = 0;
        }
    }

    /// The points kept, oldest first.
    pub fn as_slice(&self) -> &[[f64; 2]] {
        &self.data[self.start..]
    }

    /// The points kept, to plot.
    pub fn points(&self) -> PlotPoints<'_> {
        PlotPoints(Cow::Borrowed(self.as_slice()))
    }

    /// The newest point.
    pub fn last(&self) -> Option<[f64; 2]> {
        self.as_slice().last().copied()
    }

    /// Number of points kept.
    pub fn len(&self) -> usize {
        self.data.len() - self.start
    }

    /// True if no point was pushed (or all were cleared).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Removes every point.
    pub fn clear(&mut self) {
        self.data.clear();
        self.start = 0;
    }
}

// ---------------------------------------------------------------------------
// Series

/// How a line is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum LineStyle {
    /// Continuous.
    #[default]
    Solid,
    /// Dashes of `length` points separated by `gap`.
    Dashed {
        /// Dash length in points.
        length: f32,
        /// Space between dashes in points.
        gap: f32,
    },
    /// Round dots every `spacing` points.
    Dotted {
        /// Distance between dots in points.
        spacing: f32,
    },
}

impl LineStyle {
    /// Dashes of 6 points with gaps of 4.
    pub fn dashed() -> Self {
        Self::Dashed {
            length: 6.0,
            gap: 4.0,
        }
    }

    /// Dots every 4 points.
    pub fn dotted() -> Self {
        Self::Dotted { spacing: 4.0 }
    }
}

/// The shape of a point marker.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MarkerShape {
    /// A disc.
    #[default]
    Circle,
    /// A square.
    Square,
    /// A square on its tip.
    Diamond,
    /// A triangle pointing up.
    Triangle,
    /// An ×.
    Cross,
    /// A +.
    Plus,
}

/// A line through points, optionally filled down to a baseline (an area
/// chart), with markers and labels on its minimum and maximum.
#[derive(Clone, Debug)]
pub struct Line<'a> {
    points: PlotPoints<'a>,
    name: String,
    color: Option<Color>,
    width: f32,
    style: LineStyle,
    fill: Option<f64>,
    fill_alpha: f32,
    markers: Option<MarkerShape>,
    mark_min_max: bool,
}

impl<'a> Line<'a> {
    /// A line through `points` (in order).
    pub fn new(points: impl Into<PlotPoints<'a>>) -> Self {
        Self {
            points: points.into(),
            name: String::new(),
            color: None,
            width: 1.5,
            style: LineStyle::Solid,
            fill: None,
            fill_alpha: 0.25,
            markers: None,
            mark_min_max: false,
        }
    }

    /// Name shown in the legend and the tooltip.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Color (default: the next color of the plot's palette).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Width in points (default 1.5).
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Solid, dashed or dotted.
    pub fn style(mut self, style: LineStyle) -> Self {
        self.style = style;
        self
    }

    /// Fills the area between the line and `y = baseline` with the line's
    /// color, faded (see [`Line::fill_alpha`]).
    pub fn fill(mut self, baseline: f64) -> Self {
        self.fill = Some(baseline);
        self
    }

    /// Opacity of the fill (default 0.25).
    pub fn fill_alpha(mut self, alpha: f32) -> Self {
        self.fill_alpha = alpha;
        self
    }

    /// Draws a marker on every point.
    pub fn markers(mut self, shape: MarkerShape) -> Self {
        self.markers = Some(shape);
        self
    }

    /// Marks the lowest and highest point with their values.
    pub fn mark_min_max(mut self, mark: bool) -> Self {
        self.mark_min_max = mark;
        self
    }
}

/// Points drawn as markers (a scatter plot).
#[derive(Clone, Debug)]
pub struct Points<'a> {
    points: PlotPoints<'a>,
    name: String,
    color: Option<Color>,
    radius: f32,
    shape: MarkerShape,
    filled: bool,
}

impl<'a> Points<'a> {
    /// A marker on each of `points`.
    pub fn new(points: impl Into<PlotPoints<'a>>) -> Self {
        Self {
            points: points.into(),
            name: String::new(),
            color: None,
            radius: 3.0,
            shape: MarkerShape::Circle,
            filled: true,
        }
    }

    /// Name shown in the legend and the tooltip.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Color (default: the next color of the palette).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Marker radius in points (default 3).
    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    /// Marker shape.
    pub fn shape(mut self, shape: MarkerShape) -> Self {
        self.shape = shape;
        self
    }

    /// Filled (default) or outlined markers.
    pub fn filled(mut self, filled: bool) -> Self {
        self.filled = filled;
        self
    }
}

/// One bar of a [`BarChart`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    /// Position along the axis the bars stand on.
    pub position: f64,
    /// Length of the bar from its base.
    pub value: f64,
    /// Where the bar starts (0, or the top of the bar below when stacked).
    pub base: f64,
    /// Its own color.
    pub color: Option<Color>,
}

/// Bars (vertical, or horizontal with [`BarChart::horizontal`]); several
/// charts can be stacked ([`BarChart::stack_on`]) or placed side by side
/// (by offsetting their positions).
#[derive(Clone, Debug)]
pub struct BarChart {
    bars: Vec<Bar>,
    name: String,
    color: Option<Color>,
    width: f64,
    horizontal: bool,
}

impl BarChart {
    /// Bars from `(position, value)` pairs.
    pub fn new(values: impl IntoIterator<Item = (f64, f64)>) -> Self {
        Self {
            bars: values
                .into_iter()
                .map(|(position, value)| Bar {
                    position,
                    value,
                    base: 0.0,
                    color: None,
                })
                .collect(),
            name: String::new(),
            color: None,
            width: 0.8,
            horizontal: false,
        }
    }

    /// From bars with their own base and color.
    pub fn from_bars(bars: Vec<Bar>) -> Self {
        let mut chart = Self::new([]);
        chart.bars = bars;
        chart
    }

    /// A histogram of `values`: `bins` equal bars from the smallest to
    /// the largest value, each as tall as the number of values in it.
    pub fn histogram(values: &[f64], bins: usize) -> Self {
        let finite = values.iter().copied().filter(|v| v.is_finite());
        let (min, max) = finite.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
            (a.min(v), b.max(v))
        });
        if min > max {
            return Self::new([]);
        }
        Self::histogram_in(values, bins, min..=max)
    }

    /// A histogram of `values` with `bins` bars covering `range` (values
    /// outside are left out).
    pub fn histogram_in(values: &[f64], bins: usize, range: RangeInclusive<f64>) -> Self {
        let bins = bins.max(1);
        let (min, max) = (*range.start(), *range.end());
        let span = (max - min).max(f64::MIN_POSITIVE);
        let mut counts = vec![0u32; bins];
        for &v in values {
            if v.is_finite() && v >= min && v <= max {
                let i = (((v - min) / span) * bins as f64) as usize;
                counts[i.min(bins - 1)] += 1;
            }
        }
        let step = span / bins as f64;
        let mut chart = Self::new(
            counts
                .iter()
                .enumerate()
                .map(|(i, c)| (min + step * (i as f64 + 0.5), f64::from(*c))),
        );
        chart.width = step;
        chart
    }

    /// Name shown in the legend and the tooltip.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Color of the bars (default: the next color of the palette).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Width of each bar in axis units (default 0.8).
    pub fn width(mut self, width: f64) -> Self {
        self.width = width;
        self
    }

    /// Bars grow to the right from the y axis instead of up.
    pub fn horizontal(mut self) -> Self {
        self.horizontal = true;
        self
    }

    /// Stacks these bars on `below`: each bar starts at the top of the
    /// bar of `below` at the same position.
    pub fn stack_on(mut self, below: &BarChart) -> Self {
        for bar in &mut self.bars {
            if let Some(under) = below.bars.iter().find(|b| b.position == bar.position) {
                bar.base = under.base + under.value;
            }
        }
        self
    }
}

/// A horizontal line across the plot at `y` (a limit, a target).
#[derive(Clone, Debug)]
pub struct HLine {
    value: f64,
    name: String,
    color: Option<Color>,
    width: f32,
    style: LineStyle,
}

/// A vertical line across the plot at `x` (an event, a cursor).
#[derive(Clone, Debug)]
pub struct VLine {
    value: f64,
    name: String,
    color: Option<Color>,
    width: f32,
    style: LineStyle,
}

macro_rules! reference_line {
    ($ty:ident, $what:literal) => {
        impl $ty {
            #[doc = concat!("A line at ", $what, " = `value`.")]
            pub fn new(value: f64) -> Self {
                Self {
                    value,
                    name: String::new(),
                    color: None,
                    width: 1.0,
                    style: LineStyle::Solid,
                }
            }

            /// Name shown in the legend.
            pub fn name(mut self, name: impl Into<String>) -> Self {
                self.name = name.into();
                self
            }

            /// Color (default: the next color of the palette).
            pub fn color(mut self, color: Color) -> Self {
                self.color = Some(color);
                self
            }

            /// Width in points (default 1).
            pub fn width(mut self, width: f32) -> Self {
                self.width = width;
                self
            }

            /// Solid, dashed or dotted.
            pub fn style(mut self, style: LineStyle) -> Self {
                self.style = style;
                self
            }
        }
    };
}
reference_line!(HLine, "y");
reference_line!(VLine, "x");

/// Text at a point of the plot (an annotation).
#[derive(Clone, Debug)]
pub struct PlotText {
    position: [f64; 2],
    text: String,
    color: Option<Color>,
    anchor: [crate::Align; 2],
}

impl PlotText {
    /// `text` centered on `position` (`[x, y]`).
    pub fn new(position: [f64; 2], text: impl Into<String>) -> Self {
        Self {
            position,
            text: text.into(),
            color: None,
            anchor: [crate::Align::Center, crate::Align::Center],
        }
    }

    /// Text color (default: the theme's text color).
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Which point of the text sits on the position: horizontally
    /// (`Min` = left) and vertically (`Min` = top).
    pub fn anchor(mut self, x: crate::Align, y: crate::Align) -> Self {
        self.anchor = [x, y];
        self
    }
}

/// Colors for the values of a [`HeatMap`], from low to high.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Colormap {
    /// Dark blue through green to yellow (readable, also in grey).
    #[default]
    Viridis,
    /// Black to white.
    Grays,
    /// Black through red and yellow to white.
    Heat,
    /// Blue through white to red, for values around a middle.
    Diverging,
    /// Your colors, evenly spread from low to high.
    Custom(Vec<Color>),
}

impl Colormap {
    /// The color for `t` from 0 (lowest) to 1 (highest).
    pub fn sample(&self, t: f32) -> Color {
        let srgb = |c: [u8; 3]| Color::from_srgb8(c[0], c[1], c[2]);
        let stops: Vec<Color> = match self {
            Self::Viridis => [
                [68, 1, 84],
                [59, 82, 139],
                [33, 145, 140],
                [94, 201, 98],
                [253, 231, 37],
            ]
            .map(srgb)
            .to_vec(),
            Self::Grays => vec![Color::BLACK, Color::WHITE],
            Self::Heat => [[0, 0, 0], [190, 30, 20], [250, 200, 40], [255, 255, 255]]
                .map(srgb)
                .to_vec(),
            Self::Diverging => [[49, 54, 149], [247, 247, 247], [165, 0, 38]]
                .map(srgb)
                .to_vec(),
            Self::Custom(colors) => colors.clone(),
        };
        match stops.len() {
            0 => Color::TRANSPARENT,
            1 => stops[0],
            n => {
                let t = t.clamp(0.0, 1.0) * (n - 1) as f32;
                let i = (t.floor() as usize).min(n - 2);
                stops[i].lerp(stops[i + 1], t - i as f32)
            }
        }
    }
}

/// A grid of values drawn as colored cells (a heat map), over a
/// rectangle of the plot.
#[derive(Clone, Debug)]
pub struct HeatMap<'a> {
    values: Cow<'a, [f64]>,
    columns: usize,
    x: [f64; 2],
    y: [f64; 2],
    range: Option<[f64; 2]>,
    colormap: Colormap,
    name: String,
    show_values: bool,
}

impl<'a> HeatMap<'a> {
    /// `values` row by row (the first row at the bottom), `columns` per
    /// row. Cells cover x from 0 to `columns` and y from 0 to the number
    /// of rows unless moved with [`HeatMap::x`] / [`HeatMap::y`].
    pub fn new(values: impl Into<Cow<'a, [f64]>>, columns: usize) -> Self {
        let values = values.into();
        let columns = columns.max(1);
        let rows = values.len().div_ceil(columns);
        Self {
            values,
            columns,
            x: [0.0, columns as f64],
            y: [0.0, rows as f64],
            range: None,
            colormap: Colormap::Viridis,
            name: String::new(),
            show_values: false,
        }
    }

    /// The x range the cells cover.
    pub fn x(mut self, range: RangeInclusive<f64>) -> Self {
        self.x = [*range.start(), *range.end()];
        self
    }

    /// The y range the cells cover.
    pub fn y(mut self, range: RangeInclusive<f64>) -> Self {
        self.y = [*range.start(), *range.end()];
        self
    }

    /// The values mapped to the first and last color (default: the
    /// smallest and largest value).
    pub fn range(mut self, range: RangeInclusive<f64>) -> Self {
        self.range = Some([*range.start(), *range.end()]);
        self
    }

    /// The colors.
    pub fn colormap(mut self, colormap: Colormap) -> Self {
        self.colormap = colormap;
        self
    }

    /// Name shown in the tooltip.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Writes each value in its cell (when the cell is big enough).
    pub fn show_values(mut self, show: bool) -> Self {
        self.show_values = show;
        self
    }

    fn rows(&self) -> usize {
        self.values.len().div_ceil(self.columns)
    }

    fn value_range(&self) -> [f64; 2] {
        self.range.unwrap_or_else(|| {
            let (a, b) = self
                .values
                .iter()
                .filter(|v| v.is_finite())
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                    (a.min(*v), b.max(*v))
                });
            if a > b { [0.0, 1.0] } else { [a, b] }
        })
    }
}

/// Anything added to a plot.
#[derive(Clone, Debug)]
enum Item<'a> {
    Line(Line<'a>),
    Points(Points<'a>),
    Bars(BarChart),
    HLine(HLine),
    VLine(VLine),
    Text(PlotText),
    HeatMap(HeatMap<'a>),
}

impl Item<'_> {
    fn name(&self) -> &str {
        match self {
            Self::Line(l) => &l.name,
            Self::Points(p) => &p.name,
            Self::Bars(b) => &b.name,
            Self::HLine(l) => &l.name,
            Self::VLine(l) => &l.name,
            Self::HeatMap(h) => &h.name,
            Self::Text(_) => "",
        }
    }

    fn color(&self) -> Option<Color> {
        match self {
            Self::Line(l) => l.color,
            Self::Points(p) => p.color,
            Self::Bars(b) => b.color,
            Self::HLine(l) => l.color,
            Self::VLine(l) => l.color,
            Self::Text(t) => t.color,
            Self::HeatMap(_) => None,
        }
    }

    /// Whether the item takes a color from the palette.
    fn uses_palette(&self) -> bool {
        !matches!(self, Self::Text(_) | Self::HeatMap(_))
    }

    /// Extends `b` to include the item's data.
    fn extend_bounds(&self, b: &mut DataBounds) {
        match self {
            Self::Line(l) => {
                for p in l.points.as_slice() {
                    b.add(*p);
                }
                if let Some(base) = l.fill {
                    b.add_y(base);
                }
            }
            Self::Points(p) => {
                for q in p.points.as_slice() {
                    b.add(*q);
                }
            }
            Self::Bars(chart) => {
                let half = chart.width / 2.0;
                for bar in &chart.bars {
                    let (pos, lo, hi) = (bar.position, bar.base, bar.base + bar.value);
                    if chart.horizontal {
                        b.add([lo, pos - half]);
                        b.add([hi, pos + half]);
                    } else {
                        b.add([pos - half, lo]);
                        b.add([pos + half, hi]);
                    }
                }
            }
            Self::HLine(l) => b.add_y(l.value),
            Self::VLine(l) => b.add_x(l.value),
            Self::Text(t) => b.add(t.position),
            Self::HeatMap(h) => {
                b.add([h.x[0], h.y[0]]);
                b.add([h.x[1], h.y[1]]);
            }
        }
    }
}

/// Smallest and largest x and y of some data.
#[derive(Clone, Copy, Debug)]
struct DataBounds {
    min: [f64; 2],
    max: [f64; 2],
    /// Only positive values (for logarithmic axes).
    min_positive: [f64; 2],
}

impl Default for DataBounds {
    fn default() -> Self {
        Self {
            min: [f64::INFINITY; 2],
            max: [f64::NEG_INFINITY; 2],
            min_positive: [f64::INFINITY; 2],
        }
    }
}

impl DataBounds {
    fn add_axis(&mut self, axis: usize, v: f64) {
        if v.is_finite() {
            self.min[axis] = self.min[axis].min(v);
            self.max[axis] = self.max[axis].max(v);
            if v > 0.0 {
                self.min_positive[axis] = self.min_positive[axis].min(v);
            }
        }
    }

    fn add(&mut self, p: [f64; 2]) {
        if p[0].is_finite() && p[1].is_finite() {
            self.add_axis(0, p[0]);
            self.add_axis(1, p[1]);
        }
    }

    fn add_x(&mut self, x: f64) {
        self.add_axis(0, x);
    }

    fn add_y(&mut self, y: f64) {
        self.add_axis(1, y);
    }
}

// ---------------------------------------------------------------------------
// Axes

/// Where an axis puts its ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Ticks {
    /// Round values (1, 2 or 5 times a power of ten) about every 50–80
    /// points.
    #[default]
    Auto,
    /// About this many round values across the axis.
    Count(usize),
    /// Every `step` units.
    Step(f64),
    /// No ticks or labels.
    None,
}

/// Formats an axis value for its tick label.
pub type TickFormatter = Arc<dyn Fn(f64) -> String + Send + Sync>;

/// One axis of a [`Plot`]: title, range, ticks, labels, grid lines and
/// scale.
#[derive(Clone)]
pub struct Axis {
    label: String,
    range: Option<[f64; 2]>,
    ticks: Ticks,
    minor_ticks: bool,
    format: Option<TickFormatter>,
    grid: bool,
    show: bool,
    log: bool,
    inverted: bool,
    limits: Option<[f64; 2]>,
}

impl std::fmt::Debug for Axis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Axis")
            .field("label", &self.label)
            .field("range", &self.range)
            .field("ticks", &self.ticks)
            .field("log", &self.log)
            .finish_non_exhaustive()
    }
}

impl Default for Axis {
    fn default() -> Self {
        Self::new()
    }
}

impl Axis {
    /// An axis fitting the data, with automatic ticks, labels and grid.
    pub fn new() -> Self {
        Self {
            label: String::new(),
            range: None,
            ticks: Ticks::Auto,
            minor_ticks: true,
            format: None,
            grid: true,
            show: true,
            log: false,
            inverted: false,
            limits: None,
        }
    }

    /// Title written along the axis (e.g. `"Time [s]"`).
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// The range shown at first (and after a double-click), instead of
    /// fitting the data.
    pub fn range(mut self, range: RangeInclusive<f64>) -> Self {
        self.range = Some([*range.start(), *range.end()]);
        self
    }

    /// The farthest the view can be panned or zoomed out to.
    pub fn limits(mut self, limits: RangeInclusive<f64>) -> Self {
        self.limits = Some([*limits.start(), *limits.end()]);
        self
    }

    /// Where the ticks go.
    pub fn ticks(mut self, ticks: Ticks) -> Self {
        self.ticks = ticks;
        self
    }

    /// Smaller unlabeled ticks and grid lines between the main ones
    /// (default on).
    pub fn minor_ticks(mut self, show: bool) -> Self {
        self.minor_ticks = show;
        self
    }

    /// Formats tick labels and tooltip values (units, dates, …).
    pub fn format(mut self, format: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
        self.format = Some(Arc::new(format));
        self
    }

    /// Grid lines at the ticks (default on).
    pub fn grid(mut self, grid: bool) -> Self {
        self.grid = grid;
        self
    }

    /// Shows the tick labels and title (default on).
    pub fn show(mut self, show: bool) -> Self {
        self.show = show;
        self
    }

    /// Logarithmic scale (only positive values are drawn).
    pub fn log(mut self, log: bool) -> Self {
        self.log = log;
        self
    }

    /// Values grow the other way (right to left, or top to bottom).
    pub fn inverted(mut self, inverted: bool) -> Self {
        self.inverted = inverted;
        self
    }

    /// The text for `value` (`step`: distance between ticks).
    fn format_value(&self, value: f64, step: f64) -> String {
        if let Some(format) = &self.format {
            return format(value);
        }
        format_number(value, step)
    }

    /// Axis value ↔ the linear value used for placement.
    fn to_linear(&self, v: f64) -> f64 {
        if self.log {
            if v > 0.0 { v.log10() } else { f64::NAN }
        } else {
            v
        }
    }

    fn value_of_linear(&self, v: f64) -> f64 {
        if self.log { 10f64.powf(v) } else { v }
    }
}

/// A number with as many decimals as `step` needs.
fn format_number(value: f64, step: f64) -> String {
    let abs = value.abs();
    if abs != 0.0 && !(1e-4..1e7).contains(&abs) {
        return format!("{value:.2e}");
    }
    let decimals = if step > 0.0 && step.is_finite() {
        (-step.log10().floor()).clamp(0.0, 10.0) as usize
    } else {
        2
    };
    let s = format!("{value:.decimals$}");
    if s.starts_with('-') && s[1..].chars().all(|c| c == '0' || c == '.') {
        s[1..].to_owned()
    } else {
        s
    }
}

/// A number with at most two decimals, without trailing zeros.
fn short_number(value: f64) -> String {
    let s = format_number(value, 0.01);
    if s.contains('.') && !s.contains('e') {
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        s
    }
}

/// A round step (1, 2 or 5 × 10ⁿ) close to `span / count`.
fn nice_step(span: f64, count: f64) -> f64 {
    if span <= 0.0 || !span.is_finite() {
        return 1.0;
    }
    let raw = span / count.max(1.0);
    let power = 10f64.powf(raw.log10().floor());
    let mantissa = raw / power;
    let nice = if mantissa < 1.5 {
        1.0
    } else if mantissa < 3.5 {
        2.0
    } else if mantissa < 7.5 {
        5.0
    } else {
        10.0
    };
    nice * power
}

/// Major and minor tick positions (in linear axis values) for a range.
fn ticks_for(
    axis: &Axis,
    lo: f64,
    hi: f64,
    length_px: f32,
    spacing_px: f32,
) -> (Vec<f64>, Vec<f64>, f64) {
    let span = hi - lo;
    if span.is_nan() || span <= 0.0 || matches!(axis.ticks, Ticks::None) {
        return (Vec::new(), Vec::new(), 1.0);
    }
    if axis.log {
        // Decades, with 2…9 as minor ticks.
        let step = (span / f64::from((length_px / spacing_px).max(1.0)))
            .ceil()
            .max(1.0);
        let mut major = Vec::new();
        let mut minor = Vec::new();
        let mut d = (lo / step).floor() * step;
        while d <= hi + 1e-9 && major.len() < 200 {
            if d >= lo - 1e-9 {
                major.push(d);
            }
            if axis.minor_ticks && step == 1.0 {
                for k in 2..10 {
                    let m = d + f64::from(k).log10();
                    if m > lo && m < hi {
                        minor.push(m);
                    }
                }
            }
            d += step;
        }
        return (major, minor, 1.0);
    }
    let step = match axis.ticks {
        Ticks::Step(s) if s > 0.0 && span / s < 1000.0 => s,
        Ticks::Count(n) => nice_step(span, n as f64),
        _ => nice_step(span, f64::from((length_px / spacing_px).max(1.0))),
    };
    let first = (lo / step).ceil() * step;
    let mut major = Vec::new();
    let mut v = first;
    while v <= hi + step * 1e-9 && major.len() < 1000 {
        major.push(if v.abs() < step * 1e-9 { 0.0 } else { v });
        v += step;
    }
    let mut minor = Vec::new();
    if axis.minor_ticks {
        let mantissa = step / 10f64.powf(step.log10().floor());
        let parts = if (mantissa - 2.0).abs() < 0.01 {
            4.0
        } else {
            5.0
        };
        let small = step / parts;
        let mut v = (lo / small).ceil() * small;
        while v <= hi && minor.len() < 5000 {
            let on_major = ((v / step).round() * step - v).abs() < small * 1e-6;
            if !on_major {
                minor.push(v);
            }
            v += small;
        }
    }
    (major, minor, step)
}

// ---------------------------------------------------------------------------
// The plot

/// The data range shown by a plot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotBounds {
    /// Smallest x and y shown.
    pub min: [f64; 2],
    /// Largest x and y shown.
    pub max: [f64; 2],
}

impl PlotBounds {
    /// Width along x.
    pub fn width(&self) -> f64 {
        self.max[0] - self.min[0]
    }

    /// Height along y.
    pub fn height(&self) -> f64 {
        self.max[1] - self.min[1]
    }

    /// Whether `point` is inside.
    pub fn contains(&self, point: [f64; 2]) -> bool {
        (self.min[0]..=self.max[0]).contains(&point[0])
            && (self.min[1]..=self.max[1]).contains(&point[1])
    }
}

/// A corner of the plot area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Corner {
    /// Top left.
    TopLeft,
    /// Top right.
    #[default]
    TopRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom right.
    BottomRight,
}

/// State of a plot kept between frames.
#[derive(Clone, Debug, Default, PartialEq)]
struct PlotMemory {
    /// Linear bounds chosen by panning or zooming, per axis (`None`:
    /// automatic).
    user: [Option<[f64; 2]>; 2],
    /// Names of series hidden from the legend.
    hidden: Vec<String>,
    /// Start of a box zoom (secondary-button drag).
    box_start: Option<Point>,
    /// Bounds shown last frame.
    shown: Option<PlotBounds>,
}

/// What [`Plot::show`] returns.
#[derive(Clone, Debug)]
pub struct PlotResponse<R> {
    /// What the closure returned.
    pub inner: R,
    /// The plot's response (hover, clicks, drags).
    pub response: Response,
    /// The pointer position in plot coordinates, while over the plot.
    pub pointer: Option<[f64; 2]>,
    /// The data range shown.
    pub bounds: PlotBounds,
    /// The name of the series under the pointer (the tooltip's).
    pub hovered: Option<String>,
}

/// Adds series to a [`Plot`].
pub struct PlotUi<'a> {
    items: Vec<Item<'a>>,
    bounds: PlotBounds,
    pointer: Option<[f64; 2]>,
}

impl std::fmt::Debug for PlotUi<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlotUi")
            .field("items", &self.items.len())
            .finish_non_exhaustive()
    }
}

impl<'a> PlotUi<'a> {
    /// A line (or area).
    pub fn line(&mut self, line: Line<'a>) {
        self.items.push(Item::Line(line));
    }

    /// A scatter of markers.
    pub fn points(&mut self, points: Points<'a>) {
        self.items.push(Item::Points(points));
    }

    /// Bars (also histograms, see [`BarChart::histogram`]).
    pub fn bar_chart(&mut self, chart: BarChart) {
        self.items.push(Item::Bars(chart));
    }

    /// A horizontal reference line.
    pub fn hline(&mut self, line: HLine) {
        self.items.push(Item::HLine(line));
    }

    /// A vertical reference line.
    pub fn vline(&mut self, line: VLine) {
        self.items.push(Item::VLine(line));
    }

    /// Text at a point.
    pub fn text(&mut self, text: PlotText) {
        self.items.push(Item::Text(text));
    }

    /// A heat map.
    pub fn heatmap(&mut self, heatmap: HeatMap<'a>) {
        self.items.push(Item::HeatMap(heatmap));
    }

    /// The range shown in the previous frame.
    pub fn bounds(&self) -> PlotBounds {
        self.bounds
    }

    /// The pointer in plot coordinates (previous frame's mapping), while
    /// over the plot.
    pub fn pointer(&self) -> Option<[f64; 2]> {
        self.pointer
    }
}

/// Data on an x/y plane: lines, areas, points, bars, histograms, heat
/// maps, reference lines and text, with axes (title, range, ticks,
/// formatted labels, grid, logarithmic or inverted scale), a legend that
/// hides series when clicked, and a tooltip with the nearest value.
///
/// Drag to pan, scroll or pinch to zoom (around the pointer), drag with
/// the secondary button to zoom into a box, double-click to go back to
/// the initial view. For live data, push samples into a [`PlotBuffer`]
/// and use [`Plot::follow_x`] to keep the latest ones in view.
///
/// ```ignore
/// Plot::new("temperatures")
///     .height(240.0)
///     .x_axis(Axis::new().label("Time [s]"))
///     .y_axis(Axis::new().label("°C").format(|v| format!("{v:.1}")))
///     .legend(Corner::TopLeft)
///     .show(ui, |plot| {
///         plot.line(Line::new(&self.t1).name("Temperature 1"));
///         plot.line(Line::new(&self.t2).name("Temperature 2").style(LineStyle::dashed()));
///         plot.hline(HLine::new(80.0).name("Limit").color(red));
///     });
/// ```
///
/// There is one x axis and one y axis; any number of series share them,
/// each with its own color, width and line style.
#[derive(Clone, Debug)]
pub struct Plot {
    id_salt: Id,
    width: Option<f32>,
    height: f32,
    axes: [Axis; 2],
    legend: Option<Corner>,
    drag: bool,
    zoom: bool,
    box_zoom: bool,
    double_click_reset: bool,
    crosshair: bool,
    tooltip: bool,
    follow_x: Option<f64>,
    include: [Vec<f64>; 2],
    margin_fraction: f32,
    min_span: [f64; 2],
    palette: Option<Vec<Color>>,
    background: Option<Color>,
}

impl Plot {
    /// A plot identified by `id_salt` (unique in its Ui), 200 points tall
    /// and as wide as the available space.
    pub fn new(id_salt: impl Hash) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            width: None,
            height: 200.0,
            axes: [Axis::new(), Axis::new()],
            legend: None,
            drag: true,
            zoom: true,
            box_zoom: true,
            double_click_reset: true,
            crosshair: true,
            tooltip: true,
            follow_x: None,
            include: [Vec::new(), Vec::new()],
            margin_fraction: 0.05,
            min_span: [0.0, 0.0],
            palette: None,
            background: None,
        }
    }

    /// Width in points (default: the available width).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Height in points (default 200).
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// The x axis.
    pub fn x_axis(mut self, axis: Axis) -> Self {
        self.axes[0] = axis;
        self
    }

    /// The y axis.
    pub fn y_axis(mut self, axis: Axis) -> Self {
        self.axes[1] = axis;
        self
    }

    /// Shows a legend with the named series in `corner`; clicking an
    /// entry hides or shows its series.
    pub fn legend(mut self, corner: Corner) -> Self {
        self.legend = Some(corner);
        self
    }

    /// Dragging pans the view (default on).
    pub fn allow_drag(mut self, allow: bool) -> Self {
        self.drag = allow;
        self
    }

    /// Scrolling and pinching zoom (default on).
    pub fn allow_zoom(mut self, allow: bool) -> Self {
        self.zoom = allow;
        self
    }

    /// Dragging with the secondary button zooms into a box (default on).
    pub fn allow_box_zoom(mut self, allow: bool) -> Self {
        self.box_zoom = allow;
        self
    }

    /// Double-clicking goes back to the initial view (default on).
    pub fn allow_reset(mut self, allow: bool) -> Self {
        self.double_click_reset = allow;
        self
    }

    /// Lines following the pointer (default on).
    pub fn crosshair(mut self, show: bool) -> Self {
        self.crosshair = show;
        self
    }

    /// A tooltip with the nearest point's values (default on).
    pub fn tooltip(mut self, show: bool) -> Self {
        self.tooltip = show;
        self
    }

    /// Shows the last `width` units of x (the newest data at the right
    /// edge), scrolling as data arrives; y fits the data in view. Panning
    /// or zooming stops following until a double-click.
    pub fn follow_x(mut self, width: f64) -> Self {
        self.follow_x = Some(width);
        self
    }

    /// Always shows this x value (e.g. 0) when fitting the data.
    pub fn include_x(mut self, x: f64) -> Self {
        self.include[0].push(x);
        self
    }

    /// Always shows this y value (e.g. 0) when fitting the data.
    pub fn include_y(mut self, y: f64) -> Self {
        self.include[1].push(y);
        self
    }

    /// Empty space around the data when fitting it, as a fraction of its
    /// range (default 0.05).
    pub fn data_margin(mut self, fraction: f32) -> Self {
        self.margin_fraction = fraction;
        self
    }

    /// The smallest ranges zooming in can reach (x, y).
    pub fn min_zoom_span(mut self, x: f64, y: f64) -> Self {
        self.min_span = [x, y];
        self
    }

    /// Colors given to series without their own, in order (default: a
    /// palette starting with the theme's accent).
    pub fn palette(mut self, colors: Vec<Color>) -> Self {
        self.palette = Some(colors);
        self
    }

    /// Background of the plot area (default: the text fields' fill).
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Shows the plot with the series added by `add_items`.
    pub fn show<'a, R>(
        self,
        ui: &mut Ui<'_>,
        add_items: impl FnOnce(&mut PlotUi<'a>) -> R,
    ) -> PlotResponse<R> {
        let id = ui.id().with(self.id_salt);
        let style = ui.style();
        let visuals = &style.visuals;
        let mut memory: PlotMemory = ui.ctx().data(id).unwrap_or_default();
        let old_memory = memory.clone();

        let width = self.width.unwrap_or_else(|| ui.fill_width(320.0));
        let response = ui.allocate_response(vec2(width, self.height), Sense::POINTER_DRAG);
        let rect = response.rect;
        ui.describe(&response, WidgetInfo::new(WidgetRole::Image, "Plot"));

        // Last frame's mapping, for the pointer position given to the
        // closure.
        let small = TextStyle {
            size: (style.body.size * 0.85).round(),
            ..style.body.clone()
        };
        let prev = memory.shown.unwrap_or(PlotBounds {
            min: [0.0, 0.0],
            max: [1.0, 1.0],
        });
        let mut plot_ui = PlotUi {
            items: Vec::new(),
            bounds: prev,
            pointer: None,
        };
        let inner = add_items(&mut plot_ui);
        let items = plot_ui.items;

        // Colors.
        let palette = self
            .palette
            .clone()
            .unwrap_or_else(|| default_palette(visuals.accent, visuals.dark_mode));
        let mut next_color = 0;
        let colors: Vec<Color> = items
            .iter()
            .map(|item| {
                item.color().unwrap_or_else(|| {
                    if item.uses_palette() && !palette.is_empty() {
                        let c = palette[next_color % palette.len()];
                        next_color += 1;
                        c
                    } else {
                        visuals.text
                    }
                })
            })
            .collect();
        let visible: Vec<bool> = items
            .iter()
            .map(|i| i.name().is_empty() || !memory.hidden.iter().any(|h| h == i.name()))
            .collect();

        // Data bounds of the visible items, in linear axis values.
        let mut data = DataBounds::default();
        for (item, shown) in items.iter().zip(&visible) {
            if *shown {
                item.extend_bounds(&mut data);
            }
        }
        for (axis, values) in self.include.iter().enumerate() {
            for v in values {
                data.add_axis(axis, *v);
            }
        }

        // Automatic bounds per axis.
        let mut auto = [[0.0, 1.0]; 2];
        for (axis, a) in self.axes.iter().enumerate() {
            let (lo, hi) = if let Some([lo, hi]) = a.range {
                (a.to_linear(lo), a.to_linear(hi))
            } else {
                let lo = if a.log {
                    data.min_positive[axis]
                } else {
                    data.min[axis]
                };
                let hi = data.max[axis];
                if lo > hi {
                    (
                        a.to_linear(if a.log { 1.0 } else { 0.0 }),
                        a.to_linear(if a.log { 10.0 } else { 1.0 }),
                    )
                } else {
                    let (lo, hi) = (a.to_linear(lo), a.to_linear(hi));
                    let span = if hi - lo > 0.0 {
                        hi - lo
                    } else {
                        lo.abs().max(1.0)
                    };
                    let m = span * f64::from(self.margin_fraction);
                    if hi - lo > 0.0 {
                        (lo - m, hi + m)
                    } else {
                        (lo - span / 2.0, hi + span / 2.0)
                    }
                }
            };
            auto[axis] = [lo, hi];
        }
        if let Some(window) = self.follow_x.filter(|w| *w > 0.0)
            && data.max[0].is_finite()
        {
            let hi = self.axes[0].to_linear(data.max[0]);
            let lo = if self.axes[0].log {
                self.axes[0].to_linear((data.max[0] - window).max(f64::MIN_POSITIVE))
            } else {
                hi - window
            };
            auto[0] = [lo, hi];
            // y fits the data in view.
            if self.axes[1].range.is_none() {
                let mut seen = DataBounds::default();
                let (x0, x1) = (self.axes[0].value_of_linear(lo), data.max[0]);
                for (item, shown) in items.iter().zip(&visible) {
                    if !*shown {
                        continue;
                    }
                    match item {
                        Item::Line(Line { points, .. }) | Item::Points(Points { points, .. }) => {
                            for p in points.as_slice() {
                                if p[0] >= x0 && p[0] <= x1 {
                                    seen.add(*p);
                                }
                            }
                        }
                        Item::HLine(l) => seen.add_y(l.value),
                        _ => {}
                    }
                }
                for v in &self.include[1] {
                    seen.add_y(*v);
                }
                if seen.min[1] <= seen.max[1] {
                    let a = &self.axes[1];
                    let lo = a.to_linear(if a.log {
                        seen.min_positive[1]
                    } else {
                        seen.min[1]
                    });
                    let hi = a.to_linear(seen.max[1]);
                    let span = if hi - lo > 0.0 {
                        hi - lo
                    } else {
                        lo.abs().max(1.0)
                    };
                    let m = span * f64::from(self.margin_fraction);
                    auto[1] = if hi - lo > 0.0 {
                        [lo - m, hi + m]
                    } else {
                        [lo - span / 2.0, hi + span / 2.0]
                    };
                }
            }
        }
        let mut bounds = [
            memory.user[0].unwrap_or(auto[0]),
            memory.user[1].unwrap_or(auto[1]),
        ];

        // Layout: room for tick labels and titles.
        let label_height = small.size * small.line_height;
        let (y_label_width, title_y) = if self.axes[1].show {
            let (major, _, step) =
                ticks_for(&self.axes[1], bounds[1][0], bounds[1][1], self.height, 40.0);
            let widest = major
                .iter()
                .map(|v| {
                    let text = self.axes[1].format_value(self.axes[1].value_of_linear(*v), step);
                    ui.layout_text(&text, &small, None).size.x
                })
                .fold(0.0f32, f32::max);
            let title = if self.axes[1].label.is_empty() {
                0.0
            } else {
                label_height + 4.0
            };
            (widest + 6.0, title)
        } else {
            (0.0, 0.0)
        };
        let (x_label_height, title_x) = if self.axes[0].show {
            let title = if self.axes[0].label.is_empty() {
                0.0
            } else {
                label_height + 2.0
            };
            (label_height + 4.0, title)
        } else {
            (0.0, 0.0)
        };
        let plot_rect = Rect::from_min_max(
            point(rect.min.x + title_y + y_label_width, rect.min.y + 4.0),
            point(
                rect.max.x - if self.axes[0].show { 8.0 } else { 0.0 },
                rect.max.y - x_label_height - title_x,
            ),
        );
        let plot_rect = Rect::from_min_max(
            plot_rect.min,
            plot_rect.max.max(plot_rect.min + vec2(10.0, 10.0)),
        );

        // Interaction.
        let to_data = |b: &[[f64; 2]; 2], p: Point| -> [f64; 2] {
            let tx = ((p.x - plot_rect.min.x) / plot_rect.width()) as f64;
            let ty = ((plot_rect.max.y - p.y) / plot_rect.height()) as f64;
            let tx = if self.axes[0].inverted { 1.0 - tx } else { tx };
            let ty = if self.axes[1].inverted { 1.0 - ty } else { ty };
            [
                b[0][0] + tx * (b[0][1] - b[0][0]),
                b[1][0] + ty * (b[1][1] - b[1][0]),
            ]
        };
        let pointer = response.hover_pos().filter(|p| plot_rect.contains(*p));
        let mut changed = [false; 2];
        if self.drag && response.dragged() && memory.box_start.is_none() {
            let d = response.drag_delta();
            let sx = (bounds[0][1] - bounds[0][0]) / f64::from(plot_rect.width());
            let sy = (bounds[1][1] - bounds[1][0]) / f64::from(plot_rect.height());
            let dx = f64::from(d.x) * sx * if self.axes[0].inverted { -1.0 } else { 1.0 };
            let dy = f64::from(d.y) * sy * if self.axes[1].inverted { -1.0 } else { 1.0 };
            bounds[0] = [bounds[0][0] - dx, bounds[0][1] - dx];
            bounds[1] = [bounds[1][0] + dy, bounds[1][1] + dy];
            changed = [d.x != 0.0, d.y != 0.0];
            ui.ctx().set_cursor(crate::CursorIcon::Grabbing);
        }
        if self.zoom
            && let Some(p) = pointer
        {
            let scroll = ui.input().scroll_delta.y;
            let pinch = ui.input().zoom_delta;
            let mut factor = f64::from(pinch);
            if scroll != 0.0 {
                factor *= f64::from((scroll * 0.0025).exp());
                let _ = ui.input_mut().consume_scroll();
            }
            if (factor - 1.0).abs() > 1e-6 {
                let center = to_data(&bounds, p);
                for (axis, c) in center.iter().enumerate() {
                    let [lo, hi] = bounds[axis];
                    let mut new = [c - (c - lo) / factor, c + (hi - c) / factor];
                    if new[1] - new[0] < self.min_span[axis] {
                        new = bounds[axis];
                    }
                    bounds[axis] = new;
                    changed[axis] = true;
                }
            }
        }
        // Box zoom with the secondary button.
        let mut zoom_box = None;
        if self.box_zoom && response.dragged_by(PointerButton::Secondary) {
            let now = response.hover_pos().or(ui.input().pointer.pos());
            if memory.box_start.is_none() {
                memory.box_start = now.map(|p| p - response.drag_delta());
            }
            if let (Some(a), Some(b)) = (memory.box_start, now) {
                zoom_box = Some(Rect::from_min_max(a.min(b), a.max(b)).intersect(plot_rect));
            }
        } else if let Some(start) = memory.box_start.take()
            && let Some(end) = ui.input().pointer.pos()
        {
            let r = Rect::from_min_max(start.min(end), start.max(end)).intersect(plot_rect);
            if r.width() > 4.0 && r.height() > 4.0 {
                let a = to_data(&bounds, point(r.min.x, r.max.y));
                let b = to_data(&bounds, point(r.max.x, r.min.y));
                bounds = [
                    [a[0].min(b[0]), a[0].max(b[0])],
                    [a[1].min(b[1]), a[1].max(b[1])],
                ];
                changed = [true, true];
            }
        }
        if self.double_click_reset && response.double_clicked() {
            memory.user = [None, None];
            bounds = auto;
            changed = [false, false];
        }
        // Keep within the limits.
        for axis in 0..2 {
            if let Some([lo, hi]) = self.axes[axis].limits {
                let (lo, hi) = (self.axes[axis].to_linear(lo), self.axes[axis].to_linear(hi));
                let span = (bounds[axis][1] - bounds[axis][0]).min(hi - lo);
                let mut start = bounds[axis][0].max(lo);
                if start + span > hi {
                    start = hi - span;
                }
                bounds[axis] = [start, start + span];
            }
            if changed[axis] {
                memory.user[axis] = Some(bounds[axis]);
            }
        }

        let transform = PlotTransform {
            rect: plot_rect,
            bounds,
            inverted: [self.axes[0].inverted, self.axes[1].inverted],
            log: [self.axes[0].log, self.axes[1].log],
        };
        let shown = PlotBounds {
            min: [
                self.axes[0].value_of_linear(bounds[0][0]),
                self.axes[1].value_of_linear(bounds[1][0]),
            ],
            max: [
                self.axes[0].value_of_linear(bounds[0][1]),
                self.axes[1].value_of_linear(bounds[1][1]),
            ],
        };

        // Background and grid.
        let saved_clip = ui.clip_rect();
        let background = self.background.unwrap_or(visuals.text_field_fill);
        ui.painter().rect_filled(plot_rect, 2.0, background);
        let grid_color = visuals.inactive.stroke.color;
        let ticks: [(Vec<f64>, Vec<f64>, f64); 2] = [
            ticks_for(
                &self.axes[0],
                bounds[0][0],
                bounds[0][1],
                plot_rect.width(),
                80.0,
            ),
            ticks_for(
                &self.axes[1],
                bounds[1][0],
                bounds[1][1],
                plot_rect.height(),
                40.0,
            ),
        ];
        ui.set_clip_rect(plot_rect);
        for (axis, (major, minor, _)) in ticks.iter().enumerate() {
            if !self.axes[axis].grid {
                continue;
            }
            for (values, color) in [
                (minor, grid_color.with_alpha(grid_color.a * 0.35)),
                (major, grid_color.with_alpha(grid_color.a * 0.9)),
            ] {
                for v in values {
                    let (a, b) = if axis == 0 {
                        let x = transform.x_linear(*v);
                        (point(x, plot_rect.min.y), point(x, plot_rect.max.y))
                    } else {
                        let y = transform.y_linear(*v);
                        (point(plot_rect.min.x, y), point(plot_rect.max.x, y))
                    };
                    ui.painter().line(a, b, Stroke::new(1.0, color));
                }
            }
        }

        // Series.
        for ((item, color), shown) in items.iter().zip(&colors).zip(&visible) {
            if *shown {
                draw_item(
                    ui,
                    item,
                    *color,
                    &transform,
                    &self.axes,
                    &small,
                    visuals.text,
                );
            }
        }
        ui.clip_rect_restore(saved_clip);
        ui.painter()
            .rect_stroke(plot_rect, 2.0, Stroke::new(1.0, grid_color));

        // Tick labels and titles.
        let text_color = visuals.weak_text;
        if self.axes[0].show {
            let (major, _, step) = &ticks[0];
            for v in major {
                let x = transform.x_linear(*v);
                let text = self.axes[0].format_value(self.axes[0].value_of_linear(*v), *step);
                let g = ui.layout_text(&text, &small, None);
                let left = (x - g.size.x / 2.0)
                    .min(rect.max.x - g.size.x)
                    .max(rect.min.x);
                ui.painter()
                    .galley(point(left, plot_rect.max.y + 3.0), g, text_color);
            }
            if !self.axes[0].label.is_empty() {
                let g = ui.layout_text(&self.axes[0].label, &small, None);
                let pos = point(plot_rect.center().x - g.size.x / 2.0, rect.max.y - g.size.y);
                ui.painter().galley(pos, g, visuals.text);
            }
        }
        if self.axes[1].show {
            let (major, _, step) = &ticks[1];
            for v in major {
                let y = transform.y_linear(*v);
                let text = self.axes[1].format_value(self.axes[1].value_of_linear(*v), *step);
                let g = ui.layout_text(&text, &small, None);
                let top = (y - g.size.y / 2.0)
                    .min(rect.max.y - g.size.y)
                    .max(rect.min.y);
                ui.painter()
                    .galley(point(plot_rect.min.x - 4.0 - g.size.x, top), g, text_color);
            }
            if !self.axes[1].label.is_empty() {
                let g = ui.layout_text(&self.axes[1].label, &small, None);
                // Rotated a quarter turn, reading upwards.
                let origin = point(rect.min.x, plot_rect.center().y + g.size.x / 2.0);
                let t = Transform::from_axes(origin, vec2(0.0, -1.0), vec2(1.0, 0.0));
                ui.painter().galley_transformed(g, t, visuals.text);
            }
        }

        // Legend.
        if let Some(corner) = self.legend {
            draw_legend(
                ui,
                id,
                corner,
                plot_rect,
                &items,
                &colors,
                &mut memory.hidden,
                &small,
            );
        }

        // Hover: crosshair and the nearest point.
        let mut hovered = None;
        let mut pointer_data = None;
        if let Some(p) = pointer
            && zoom_box.is_none()
            && !response.dragged()
        {
            let lin = to_data(&bounds, p);
            pointer_data = Some([
                self.axes[0].value_of_linear(lin[0]),
                self.axes[1].value_of_linear(lin[1]),
            ]);
            ui.set_clip_rect(plot_rect);
            if self.crosshair {
                let c = visuals.weak_text.with_alpha(0.6);
                ui.painter().line(
                    point(p.x, plot_rect.min.y),
                    point(p.x, plot_rect.max.y),
                    Stroke::new(1.0, c),
                );
                ui.painter().line(
                    point(plot_rect.min.x, p.y),
                    point(plot_rect.max.x, p.y),
                    Stroke::new(1.0, c),
                );
            }
            ui.clip_rect_restore(saved_clip);
            if self.tooltip
                && let Some((index, value, screen)) = nearest(&items, &visible, &transform, p)
            {
                let color = colors[index];
                ui.painter()
                    .circle(screen, 4.0, color, Stroke::new(1.5, background));
                let name = items[index].name();
                let xs = self.axes[0].format_value(value[0], ticks[0].2 / 10.0);
                let ys = self.axes[1].format_value(value[1], ticks[1].2 / 10.0);
                let text = if name.is_empty() {
                    format!("x: {xs}\ny: {ys}")
                } else {
                    format!("{name}\nx: {xs}\ny: {ys}")
                };
                let g = ui.layout_text(&text, &small, None);
                let pad = 6.0;
                let size = g.size + Vec2::splat(2.0 * pad);
                let mut pos = screen + vec2(12.0, 12.0);
                if pos.x + size.x > plot_rect.max.x {
                    pos.x = screen.x - 12.0 - size.x;
                }
                if pos.y + size.y > plot_rect.max.y {
                    pos.y = screen.y - 12.0 - size.y;
                }
                let tip = Rect::from_min_size(pos, size);
                ui.painter().shadow(tip, 4.0, visuals.shadow);
                ui.painter()
                    .rect(tip, 4.0, visuals.window_fill, visuals.window_stroke);
                ui.painter().galley(pos + Vec2::splat(pad), g, visuals.text);
                hovered = (!name.is_empty()).then(|| name.to_owned());
            }
        }
        if let Some(b) = zoom_box {
            ui.painter().rect(
                b,
                0.0,
                visuals.selection.with_alpha(0.25),
                Stroke::new(1.0, visuals.accent),
            );
        }

        memory.shown = Some(shown);
        if memory != old_memory {
            ui.ctx().insert_data(id, memory);
        }
        if self.follow_x.is_some() || response.dragged() {
            ui.ctx().request_repaint();
        }
        PlotResponse {
            inner,
            response,
            pointer: pointer_data,
            bounds: shown,
            hovered,
        }
    }
}

/// A palette of distinct colors starting with `accent`.
fn default_palette(accent: Color, dark: bool) -> Vec<Color> {
    let base: [[u8; 3]; 8] = if dark {
        [
            [250, 179, 135],
            [166, 227, 161],
            [243, 139, 168],
            [203, 166, 247],
            [249, 226, 175],
            [148, 226, 213],
            [245, 194, 231],
            [180, 190, 254],
        ]
    } else {
        [
            [214, 96, 30],
            [46, 140, 70],
            [200, 40, 80],
            [130, 80, 200],
            [180, 140, 0],
            [20, 140, 140],
            [190, 70, 160],
            [70, 90, 200],
        ]
    };
    std::iter::once(accent)
        .chain(base.iter().map(|c| Color::from_srgb8(c[0], c[1], c[2])))
        .collect()
}

/// Maps linear axis values to the screen.
#[derive(Clone, Copy, Debug)]
struct PlotTransform {
    rect: Rect,
    bounds: [[f64; 2]; 2],
    inverted: [bool; 2],
    log: [bool; 2],
}

impl PlotTransform {
    fn t(&self, axis: usize, v: f64) -> f32 {
        let [lo, hi] = self.bounds[axis];
        let t = ((v - lo) / (hi - lo)) as f32;
        if self.inverted[axis] { 1.0 - t } else { t }
    }

    fn x_linear(&self, x: f64) -> f32 {
        self.rect.min.x + self.t(0, x) * self.rect.width()
    }

    fn y_linear(&self, y: f64) -> f32 {
        self.rect.max.y - self.t(1, y) * self.rect.height()
    }

    fn linear(&self, axis: usize, v: f64) -> f64 {
        if self.log[axis] {
            if v > 0.0 { v.log10() } else { f64::NAN }
        } else {
            v
        }
    }

    /// The screen position of a data point (NaN coordinates for values
    /// a log axis can't show).
    fn screen(&self, p: [f64; 2]) -> Point {
        point(
            self.x_linear(self.linear(0, p[0])),
            self.y_linear(self.linear(1, p[1])),
        )
    }

    fn visible_x(&self) -> [f64; 2] {
        let [lo, hi] = self.bounds[0];
        if self.log[0] {
            [10f64.powf(lo), 10f64.powf(hi)]
        } else {
            [lo, hi]
        }
    }
}

/// Screen points of a series: only those near the visible x range, and
/// for long sorted series the extremes of each pixel column.
fn screen_points(points: &[[f64; 2]], t: &PlotTransform) -> Vec<Point> {
    let sorted = points.windows(2).all(|w| w[0][0] <= w[1][0]);
    let slice = if sorted && points.len() > 2 {
        let [lo, hi] = t.visible_x();
        let start = points.partition_point(|p| p[0] < lo).saturating_sub(1);
        let end = (points.partition_point(|p| p[0] <= hi) + 1).min(points.len());
        &points[start..end.max(start)]
    } else {
        points
    };
    let width = t.rect.width().max(1.0) as usize;
    if !sorted || slice.len() <= width * 4 {
        return slice
            .iter()
            .map(|p| t.screen(*p))
            .filter(|p| p.x.is_finite() && p.y.is_finite())
            .collect();
    }
    // Several points per pixel column: keep the first, lowest, highest and
    // last of each column, in order, so the shape is exact at screen
    // resolution.
    let mut out = Vec::with_capacity(width * 4);
    let mut column = i64::MIN;
    let mut bucket: Vec<Point> = Vec::new();
    let flush = |bucket: &mut Vec<Point>, out: &mut Vec<Point>| {
        if bucket.is_empty() {
            return;
        }
        let first = bucket[0];
        let last = *bucket.last().expect("not empty");
        let (mut lo, mut hi) = (first, first);
        for p in bucket.iter() {
            if p.y < lo.y {
                lo = *p;
            }
            if p.y > hi.y {
                hi = *p;
            }
        }
        out.push(first);
        if lo.x <= hi.x {
            out.push(lo);
            out.push(hi);
        } else {
            out.push(hi);
            out.push(lo);
        }
        out.push(last);
        bucket.clear();
    };
    for p in slice {
        let s = t.screen(*p);
        if !(s.x.is_finite() && s.y.is_finite()) {
            continue;
        }
        let c = s.x.floor() as i64;
        if c != column {
            flush(&mut bucket, &mut out);
            column = c;
        }
        bucket.push(s);
    }
    flush(&mut bucket, &mut out);
    out
}

fn draw_polyline(
    painter: &mut DisplayList,
    points: Vec<Point>,
    width: f32,
    color: Color,
    style: LineStyle,
) {
    if points.len() < 2 {
        return;
    }
    match style {
        LineStyle::Solid => painter.polyline(points, Stroke::new(width, color)),
        LineStyle::Dashed { length, gap } => {
            painter.dashed_line(&points, Stroke::new(width, color), length, gap);
        }
        LineStyle::Dotted { spacing } => painter.dotted_line(&points, width * 0.75, spacing, color),
    }
}

fn draw_marker(
    painter: &mut DisplayList,
    c: Point,
    r: f32,
    shape: MarkerShape,
    color: Color,
    filled: bool,
) {
    let (fill, stroke) = if filled {
        (color, Stroke::NONE)
    } else {
        (Color::TRANSPARENT, Stroke::new(1.5, color))
    };
    match shape {
        MarkerShape::Circle => painter.circle(c, r, fill, stroke),
        MarkerShape::Square => painter.rect(
            Rect::from_center_size(c, Vec2::splat(2.0 * r * 0.9)),
            0.0,
            fill,
            stroke,
        ),
        MarkerShape::Diamond => painter.polygon(
            vec![
                c + vec2(0.0, -r * 1.2),
                c + vec2(r * 1.2, 0.0),
                c + vec2(0.0, r * 1.2),
                c + vec2(-r * 1.2, 0.0),
            ],
            fill,
            stroke,
        ),
        MarkerShape::Triangle => painter.polygon(
            vec![
                c + vec2(0.0, -r * 1.2),
                c + vec2(r * 1.1, r * 0.8),
                c + vec2(-r * 1.1, r * 0.8),
            ],
            fill,
            stroke,
        ),
        MarkerShape::Cross => {
            let s = Stroke::new(1.5, color);
            painter.line(c + vec2(-r, -r), c + vec2(r, r), s);
            painter.line(c + vec2(-r, r), c + vec2(r, -r), s);
        }
        MarkerShape::Plus => {
            let s = Stroke::new(1.5, color);
            painter.line(c + vec2(-r, 0.0), c + vec2(r, 0.0), s);
            painter.line(c + vec2(0.0, -r), c + vec2(0.0, r), s);
        }
    }
}

/// Triangles filling between a line and the horizontal `base_y` (screen
/// coordinates), split where the line crosses it.
fn area_triangles(
    points: &[Point],
    base_y: f32,
    color: Color,
) -> (Vec<Point>, Vec<Color>, Vec<u32>) {
    let mut out_points = Vec::new();
    let mut indices = Vec::new();
    let mut quad = |a: Point, b: Point| {
        let i = out_points.len() as u32;
        out_points.extend([a, b, point(b.x, base_y), point(a.x, base_y)]);
        indices.extend([i, i + 1, i + 2, i, i + 2, i + 3]);
    };
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let (da, db) = (a.y - base_y, b.y - base_y);
        if da * db < 0.0 {
            // Crosses the baseline: two triangles meeting there.
            let t = da / (da - db);
            let m = point(a.x + (b.x - a.x) * t, base_y);
            quad(a, m);
            quad(m, b);
        } else {
            quad(a, b);
        }
    }
    (out_points, vec![color], indices)
}

fn draw_item(
    ui: &mut Ui<'_>,
    item: &Item<'_>,
    color: Color,
    t: &PlotTransform,
    axes: &[Axis; 2],
    small: &TextStyle,
    text_color: Color,
) {
    let rect = t.rect;
    match item {
        Item::Line(line) => {
            let pts = screen_points(line.points.as_slice(), t);
            if let Some(base) = line.fill {
                let base_y = t
                    .y_linear(t.linear(1, base))
                    .clamp(rect.min.y - 1.0, rect.max.y + 1.0);
                let fill = color.with_alpha(color.a * line.fill_alpha);
                let (p, c, i) = area_triangles(&pts, base_y, fill);
                ui.painter().triangles(p, c, i);
            }
            if let Some(shape) = line.markers {
                for p in &pts {
                    draw_marker(ui.painter(), *p, line.width + 2.0, shape, color, true);
                }
            }
            draw_polyline(ui.painter(), pts, line.width, color, line.style);
            if line.mark_min_max {
                mark_extremes(ui, line.points.as_slice(), t, axes, color, small);
            }
        }
        Item::Points(points) => {
            for p in points.points.as_slice() {
                let s = t.screen(*p);
                if rect.expand(points.radius * 2.0).contains(s) {
                    draw_marker(
                        ui.painter(),
                        s,
                        points.radius,
                        points.shape,
                        color,
                        points.filled,
                    );
                }
            }
        }
        Item::Bars(chart) => {
            let half = chart.width / 2.0;
            for bar in &chart.bars {
                let (lo, hi) = (bar.base, bar.base + bar.value);
                let (a, b) = if chart.horizontal {
                    ([lo, bar.position - half], [hi, bar.position + half])
                } else {
                    ([bar.position - half, lo], [bar.position + half, hi])
                };
                let (pa, pb) = (t.screen(a), t.screen(b));
                if !(pa.x.is_finite() && pa.y.is_finite() && pb.x.is_finite() && pb.y.is_finite()) {
                    continue;
                }
                let r = Rect::from_min_max(pa.min(pb), pa.max(pb));
                let c = bar.color.unwrap_or(color);
                ui.painter()
                    .rect(r, 1.0, c.with_alpha(c.a * 0.85), Stroke::new(1.0, c));
            }
        }
        Item::HLine(l) => {
            let y = t.y_linear(t.linear(1, l.value));
            draw_polyline(
                ui.painter(),
                vec![point(rect.min.x, y), point(rect.max.x, y)],
                l.width,
                color,
                l.style,
            );
        }
        Item::VLine(l) => {
            let x = t.x_linear(t.linear(0, l.value));
            draw_polyline(
                ui.painter(),
                vec![point(x, rect.min.y), point(x, rect.max.y)],
                l.width,
                color,
                l.style,
            );
        }
        Item::Text(text) => {
            let s = t.screen(text.position);
            let g = ui.layout_text(&text.text, small, None);
            let pos = point(
                s.x - align_shift(text.anchor[0], g.size.x),
                s.y - align_shift(text.anchor[1], g.size.y),
            );
            let color = text.color.unwrap_or(text_color);
            ui.painter().galley(pos, g, color);
        }
        Item::HeatMap(h) => draw_heatmap(ui, h, t, small),
    }
}

/// How far to move text of `size` left (or up) so `align` sits on the
/// anchor.
fn align_shift(align: crate::Align, size: f32) -> f32 {
    match align {
        crate::Align::Min => 0.0,
        crate::Align::Center => size / 2.0,
        crate::Align::Max => size,
    }
}

fn mark_extremes(
    ui: &mut Ui<'_>,
    points: &[[f64; 2]],
    t: &PlotTransform,
    axes: &[Axis; 2],
    color: Color,
    small: &TextStyle,
) {
    let finite = points
        .iter()
        .filter(|p| p[0].is_finite() && p[1].is_finite());
    let min = finite.clone().min_by(|a, b| a[1].total_cmp(&b[1]));
    let max = finite.max_by(|a, b| a[1].total_cmp(&b[1]));
    let style = ui.style();
    for (p, above, word) in [(max, true, "max"), (min, false, "min")] {
        let Some(p) = p else { continue };
        let s = t.screen(*p);
        if !t.rect.contains(s) {
            continue;
        }
        draw_marker(ui.painter(), s, 4.0, MarkerShape::Diamond, color, true);
        let text = format!("{word} {}", axes[1].format_value(p[1], 0.01));
        let g = ui.layout_text(&text, small, None);
        let x = (s.x - g.size.x / 2.0)
            .min(t.rect.max.x - g.size.x - 2.0)
            .max(t.rect.min.x + 2.0);
        let y = if above {
            s.y - 8.0 - g.size.y
        } else {
            s.y + 8.0
        };
        let y = y.min(t.rect.max.y - g.size.y - 2.0).max(t.rect.min.y + 2.0);
        let r = Rect::from_min_size(point(x, y), g.size).expand(2.0);
        ui.painter()
            .rect_filled(r, 3.0, style.visuals.window_fill.with_alpha(0.85));
        ui.painter().galley(point(x, y), g, color);
    }
}

fn draw_heatmap(ui: &mut Ui<'_>, h: &HeatMap<'_>, t: &PlotTransform, small: &TextStyle) {
    let rows = h.rows();
    let [lo, hi] = h.value_range();
    let span = if hi > lo { hi - lo } else { 1.0 };
    let cw = (h.x[1] - h.x[0]) / h.columns as f64;
    let ch = (h.y[1] - h.y[0]) / rows.max(1) as f64;
    let mut points = Vec::with_capacity(h.values.len() * 4);
    let mut colors = Vec::with_capacity(h.values.len() * 4);
    let mut indices = Vec::with_capacity(h.values.len() * 6);
    let mut labels = Vec::new();
    for (i, v) in h.values.iter().enumerate() {
        if !v.is_finite() {
            continue;
        }
        let (col, row) = (i % h.columns, i / h.columns);
        let x0 = h.x[0] + cw * col as f64;
        let y0 = h.y[0] + ch * row as f64;
        let a = t.screen([x0, y0]);
        let b = t.screen([x0 + cw, y0 + ch]);
        if !(a.x.is_finite() && a.y.is_finite() && b.x.is_finite() && b.y.is_finite()) {
            continue;
        }
        let r = Rect::from_min_max(a.min(b), a.max(b));
        if r.intersect(t.rect).is_empty() {
            continue;
        }
        let c = h.colormap.sample(((v - lo) / span) as f32);
        let base = points.len() as u32;
        points.extend([
            r.min,
            point(r.max.x, r.min.y),
            r.max,
            point(r.min.x, r.max.y),
        ]);
        colors.extend([c; 4]);
        indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        if h.show_values && r.width() >= 28.0 && r.height() >= small.size + 4.0 {
            labels.push((r, *v, c));
        }
    }
    ui.painter().triangles(points, colors, indices);
    for (r, v, c) in labels {
        let g = ui.layout_text(&format_number(v, span / 100.0), small, None);
        // Dark text on light cells, light text on dark ones.
        let light = c.r * 0.2126 + c.g * 0.7152 + c.b * 0.0722 > 0.35;
        let color = if light { Color::BLACK } else { Color::WHITE };
        ui.painter().galley(r.center() - g.size / 2.0, g, color);
    }
}

/// The visible data point nearest to `pointer` (within 16 points):
/// item index, data value and screen position.
fn nearest(
    items: &[Item<'_>],
    visible: &[bool],
    t: &PlotTransform,
    pointer: Point,
) -> Option<(usize, [f64; 2], Point)> {
    let mut best: Option<(f32, usize, [f64; 2], Point)> = None;
    let mut consider = |index: usize, value: [f64; 2], s: Point| {
        if !(s.x.is_finite() && s.y.is_finite()) {
            return;
        }
        let d = (s - pointer).length_sq();
        if d < 16.0 * 16.0 && best.is_none_or(|b| d < b.0) {
            best = Some((d, index, value, s));
        }
    };
    for (index, item) in items.iter().enumerate() {
        if !visible[index] {
            continue;
        }
        match item {
            Item::Line(Line { points, .. }) | Item::Points(Points { points, .. }) => {
                for p in points.as_slice() {
                    consider(index, *p, t.screen(*p));
                }
            }
            Item::Bars(chart) => {
                for bar in &chart.bars {
                    let top = bar.base + bar.value;
                    let p = if chart.horizontal {
                        [top, bar.position]
                    } else {
                        [bar.position, top]
                    };
                    consider(index, p, t.screen(p));
                }
            }
            _ => {}
        }
    }
    best.map(|(_, i, v, s)| (i, v, s))
}

#[allow(clippy::too_many_arguments)]
fn draw_legend(
    ui: &mut Ui<'_>,
    id: Id,
    corner: Corner,
    plot_rect: Rect,
    items: &[Item<'_>],
    colors: &[Color],
    hidden: &mut Vec<String>,
    small: &TextStyle,
) {
    let style = ui.style();
    let entries: Vec<(usize, Arc<Galley>)> = items
        .iter()
        .enumerate()
        .filter(|(_, i)| !i.name().is_empty() && !matches!(i, Item::Text(_)))
        .map(|(n, i)| (n, ui.layout_text(i.name(), small, None)))
        .collect();
    if entries.is_empty() {
        return;
    }
    let row = small.size * small.line_height + 4.0;
    let swatch = 14.0;
    let width = entries.iter().map(|(_, g)| g.size.x).fold(0.0f32, f32::max) + swatch + 18.0;
    let size = vec2(width, row * entries.len() as f32 + 8.0);
    let m = 8.0;
    let min = match corner {
        Corner::TopLeft => plot_rect.min + vec2(m, m),
        Corner::TopRight => point(plot_rect.max.x - m - size.x, plot_rect.min.y + m),
        Corner::BottomLeft => point(plot_rect.min.x + m, plot_rect.max.y - m - size.y),
        Corner::BottomRight => plot_rect.max - vec2(m, m) - size,
    };
    let frame = Rect::from_min_size(min, size);
    ui.painter().rect(
        frame,
        4.0,
        style.visuals.window_fill.with_alpha(0.9),
        style.visuals.window_stroke,
    );
    let mut seen: Vec<&str> = Vec::new();
    let mut y = frame.min.y + 4.0;
    for (index, galley) in entries {
        let name = items[index].name();
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let row_rect = Rect::from_min_size(point(frame.min.x + 4.0, y), vec2(size.x - 8.0, row));
        let r = ui.interact(id.with(("legend", name)), row_rect, Sense::CLICK);
        let is_hidden = hidden.iter().any(|h| h == name);
        if r.clicked() {
            if is_hidden {
                hidden.retain(|h| h != name);
            } else {
                hidden.push(name.to_owned());
            }
        }
        if r.hovered() {
            ui.painter()
                .rect_filled(row_rect, 3.0, style.visuals.hovered.bg_fill);
        }
        let alpha = if is_hidden { 0.35 } else { 1.0 };
        let c = colors[index];
        let sw = Rect::from_center_size(
            point(row_rect.min.x + 4.0 + swatch / 2.0, row_rect.center().y),
            vec2(swatch, 4.0),
        );
        match &items[index] {
            Item::Points(p) => draw_marker(
                ui.painter(),
                sw.center(),
                3.5,
                p.shape,
                c.with_alpha(c.a * alpha),
                p.filled,
            ),
            Item::Bars(_) => ui.painter().rect_filled(
                Rect::from_center_size(sw.center(), Vec2::splat(10.0)),
                2.0,
                c.with_alpha(c.a * alpha),
            ),
            _ => ui.painter().rect_filled(sw, 2.0, c.with_alpha(c.a * alpha)),
        }
        let text_color = style.visuals.text.with_alpha(style.visuals.text.a * alpha);
        ui.painter().galley(
            point(sw.max.x + 6.0, row_rect.center().y - galley.size.y / 2.0),
            galley,
            text_color,
        );
        y += row;
    }
}

// ---------------------------------------------------------------------------
// Pie chart

/// What [`PieChart::show`] returns.
#[derive(Clone, Debug)]
pub struct PieResponse {
    /// The chart's response.
    pub response: Response,
    /// The slice under the pointer.
    pub hovered: Option<usize>,
}

/// A pie or donut chart: slices proportional to their values, with
/// percentages, a legend and the hovered slice pulled out.
///
/// ```ignore
/// PieChart::new(vec![("Rust".into(), 62.0), ("C++".into(), 25.0), ("Other".into(), 13.0)])
///     .donut(0.55)
///     .show(ui);
/// ```
#[derive(Clone, Debug)]
pub struct PieChart {
    slices: Vec<(String, f64)>,
    colors: Option<Vec<Color>>,
    donut: f32,
    diameter: f32,
    percentages: bool,
    legend: bool,
    start_angle: f32,
}

impl PieChart {
    /// Slices from `(name, value)` pairs (negative values count as 0).
    pub fn new(slices: Vec<(String, f64)>) -> Self {
        Self {
            slices,
            colors: None,
            donut: 0.0,
            diameter: 160.0,
            percentages: true,
            legend: true,
            start_angle: -90.0,
        }
    }

    /// Colors of the slices, in order (default: the palette).
    pub fn colors(mut self, colors: Vec<Color>) -> Self {
        self.colors = Some(colors);
        self
    }

    /// A donut: the hole's size as a fraction of the diameter (0–0.9).
    pub fn donut(mut self, hole: f32) -> Self {
        self.donut = hole.clamp(0.0, 0.9);
        self
    }

    /// Diameter in points (default 160).
    pub fn diameter(mut self, diameter: f32) -> Self {
        self.diameter = diameter;
        self
    }

    /// Writes each slice's percentage on it (default on).
    pub fn percentages(mut self, show: bool) -> Self {
        self.percentages = show;
        self
    }

    /// Lists the slices with their colors and values on the right
    /// (default on).
    pub fn legend(mut self, show: bool) -> Self {
        self.legend = show;
        self
    }

    /// Where the first slice starts, in degrees clockwise from 3 o'clock
    /// (default -90: 12 o'clock).
    pub fn start_angle(mut self, degrees: f32) -> Self {
        self.start_angle = degrees;
        self
    }

    /// Shows the chart.
    pub fn show(self, ui: &mut Ui<'_>) -> PieResponse {
        let style = ui.style();
        let visuals = &style.visuals;
        let small = TextStyle {
            size: (style.body.size * 0.85).round(),
            ..style.body.clone()
        };
        let total: f64 = self.slices.iter().map(|(_, v)| v.max(0.0)).sum();
        let palette = self
            .colors
            .clone()
            .unwrap_or_else(|| default_palette(visuals.accent, visuals.dark_mode));
        let color_of = |i: usize| {
            palette
                .get(i % palette.len().max(1))
                .copied()
                .unwrap_or(visuals.accent)
        };
        let legend: Vec<Arc<Galley>> = if self.legend {
            self.slices
                .iter()
                .map(|(name, v)| {
                    ui.layout_text(&format!("{name}  {}", short_number(*v)), &small, None)
                })
                .collect()
        } else {
            Vec::new()
        };
        let row = small.size * small.line_height + 4.0;
        let legend_width = legend.iter().map(|g| g.size.x).fold(0.0f32, f32::max);
        let legend_width = if legend.is_empty() {
            0.0
        } else {
            legend_width + 30.0
        };
        let pad = 6.0;
        let size = vec2(
            self.diameter + 2.0 * pad + legend_width,
            (self.diameter + 2.0 * pad).max(row * legend.len() as f32),
        );
        let response = ui.allocate_response(size, Sense::HOVER);
        ui.describe(&response, WidgetInfo::new(WidgetRole::Image, "Pie chart"));
        let rect = response.rect;
        let center = point(rect.min.x + pad + self.diameter / 2.0, rect.center().y);
        let radius = self.diameter / 2.0;
        let inner = radius * self.donut;

        // Which slice the pointer is on.
        let angle_of = |p: Point| {
            let d = p - center;
            let a = d.y.atan2(d.x).to_degrees() - self.start_angle;
            a.rem_euclid(360.0)
        };
        let mut hovered = None;
        if total > 0.0
            && let Some(p) = response.hover_pos()
        {
            let dist = (p - center).length();
            if dist <= radius + 4.0 && dist >= inner {
                let a = f64::from(angle_of(p));
                let mut acc = 0.0;
                for (i, (_, v)) in self.slices.iter().enumerate() {
                    let sweep = v.max(0.0) / total * 360.0;
                    if a >= acc && a < acc + sweep {
                        hovered = Some(i);
                        break;
                    }
                    acc += sweep;
                }
            }
        }

        if total > 0.0 {
            let mut start = self.start_angle;
            for (i, (_, v)) in self.slices.iter().enumerate() {
                let sweep = (v.max(0.0) / total * 360.0) as f32;
                if sweep <= 0.0 {
                    continue;
                }
                let mid = (start + sweep / 2.0).to_radians();
                let pull = if hovered == Some(i) { 6.0 } else { 0.0 };
                let c = center + vec2(mid.cos(), mid.sin()) * pull;
                let steps = ((sweep / 4.0).ceil() as usize).max(2);
                let mut points = Vec::with_capacity(steps * 2 + 2);
                let mut outline = Vec::with_capacity(steps + 1);
                for k in 0..=steps {
                    let a = (start + sweep * k as f32 / steps as f32).to_radians();
                    let dir = vec2(a.cos(), a.sin());
                    points.push(c + dir * radius);
                    points.push(c + dir * inner);
                    outline.push(c + dir * radius);
                }
                let mut indices = Vec::with_capacity(steps * 6);
                for k in 0..steps as u32 {
                    let (o0, i0, o1, i1) = (2 * k, 2 * k + 1, 2 * k + 2, 2 * k + 3);
                    indices.extend([o0, o1, i1, o0, i1, i0]);
                }
                let color = color_of(i);
                ui.painter().triangles(points, vec![color], indices);
                // A smooth rim and gaps between slices.
                ui.painter().polyline(outline, Stroke::new(1.0, color));
                let bg = visuals.background;
                for edge in [start, start + sweep] {
                    let a = edge.to_radians();
                    let dir = vec2(a.cos(), a.sin());
                    ui.painter().line(
                        c + dir * inner,
                        c + dir * (radius + 1.0),
                        Stroke::new(1.5, bg),
                    );
                }
                if self.percentages && sweep >= 18.0 {
                    let pct = format!("{:.0}%", v.max(0.0) / total * 100.0);
                    let g = ui.layout_text(&pct, &small, None);
                    let r = if inner > 0.0 {
                        (inner + radius) / 2.0
                    } else {
                        radius * 0.62
                    };
                    let at = c + vec2(mid.cos(), mid.sin()) * r - g.size / 2.0;
                    let light = color.r * 0.2126 + color.g * 0.7152 + color.b * 0.0722 > 0.35;
                    ui.painter()
                        .galley(at, g, if light { Color::BLACK } else { Color::WHITE });
                }
                start += sweep;
            }
        } else {
            ui.painter()
                .circle(center, radius, Color::TRANSPARENT, visuals.inactive.stroke);
        }

        // Legend.
        let lx = rect.min.x + self.diameter + 2.0 * pad + 8.0;
        let top = rect.center().y - row * legend.len() as f32 / 2.0;
        for (i, g) in legend.into_iter().enumerate() {
            let y = top + row * i as f32;
            let sw = Rect::from_min_size(point(lx, y + (row - 10.0) / 2.0), Vec2::splat(10.0));
            ui.painter().rect_filled(sw, 2.0, color_of(i));
            let color = if hovered == Some(i) {
                visuals.text
            } else {
                visuals.weak_text.lerp(visuals.text, 0.6)
            };
            ui.painter()
                .galley(point(sw.max.x + 8.0, y + (row - g.size.y) / 2.0), g, color);
        }
        PieResponse { response, hovered }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_round_and_labels_fit_them() {
        assert_eq!(nice_step(10.0, 5.0), 2.0);
        assert_eq!(nice_step(100.0, 4.0), 20.0);
        assert_eq!(nice_step(0.7, 7.0), 0.1);
        assert_eq!(format_number(2.5, 0.5), "2.5");
        assert_eq!(format_number(20.0, 10.0), "20");
        assert_eq!(format_number(-0.0001, 0.1), "0.0");
        assert_eq!(format_number(3.0e9, 1.0e9), "3.00e9");
        assert_eq!(short_number(62.0), "62");
        assert_eq!(short_number(1.25), "1.25");
        let (major, minor, step) = ticks_for(&Axis::new(), 0.0, 10.0, 400.0, 80.0);
        assert_eq!(step, 2.0);
        assert_eq!(major, [0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert!(minor.contains(&1.0) && !minor.contains(&2.0));
    }

    #[test]
    fn buffer_keeps_the_latest_points() {
        let mut b = PlotBuffer::new(3);
        for i in 0..10 {
            b.push(f64::from(i), 0.0);
        }
        assert_eq!(b.len(), 3);
        assert_eq!(b.as_slice()[0][0], 7.0);
        assert_eq!(b.last(), Some([9.0, 0.0]));
    }

    #[test]
    fn histogram_counts_values_in_bins() {
        let chart = BarChart::histogram(&[0.0, 0.1, 0.9, 1.0, 5.0, f64::NAN], 5);
        let counts: Vec<f64> = chart.bars.iter().map(|b| b.value).collect();
        assert_eq!(counts, [3.0, 1.0, 0.0, 0.0, 1.0]);
        assert_eq!(chart.width, 1.0);
        let stacked = BarChart::new([(1.0, 2.0)]).stack_on(&BarChart::new([(1.0, 3.0)]));
        assert_eq!(stacked.bars[0].base, 3.0);
    }

    #[test]
    fn area_is_split_where_the_line_crosses_the_baseline() {
        let (points, _, indices) =
            area_triangles(&[point(0.0, -10.0), point(10.0, 10.0)], 0.0, Color::WHITE);
        // Two quads meeting at x = 5 on the baseline.
        assert_eq!(indices.len(), 12);
        assert!(points.contains(&point(5.0, 0.0)));
    }

    #[test]
    fn colormaps_span_their_stops() {
        assert_eq!(Colormap::Grays.sample(0.0), Color::BLACK);
        assert_eq!(Colormap::Grays.sample(1.0), Color::WHITE);
        assert_eq!(
            Colormap::Custom(vec![Color::WHITE]).sample(0.3),
            Color::WHITE
        );
    }

    #[test]
    fn long_sorted_series_are_reduced_per_pixel_column() {
        let points: Vec<[f64; 2]> = (0..100_000)
            .map(|i| [f64::from(i), f64::from(i % 7)])
            .collect();
        let t = PlotTransform {
            rect: Rect::from_min_size(point(0.0, 0.0), vec2(100.0, 100.0)),
            bounds: [[0.0, 100_000.0], [0.0, 6.0]],
            inverted: [false; 2],
            log: [false; 2],
        };
        let pts = screen_points(&points, &t);
        assert!(pts.len() <= 101 * 4, "{}", pts.len());
        // The extremes survive.
        assert!(pts.iter().any(|p| p.y <= 0.5) && pts.iter().any(|p| p.y >= 99.5));
    }
}
