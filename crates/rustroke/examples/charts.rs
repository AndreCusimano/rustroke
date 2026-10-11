//! Charts: two temperatures on one plot (each with its own color and line
//! style, a limit line, the extremes marked), a live sensor that scrolls,
//! an area chart, a scatter plot, stacked bars, a histogram, a heat map on
//! a logarithmic-free grid, a log-scale plot and a donut chart.
//!
//! Drag to pan, scroll to zoom, right-drag to zoom into a box,
//! double-click to reset; click a legend entry to hide its series.
//!
//! Run with: `cargo run -p rustroke --example charts`

use rustroke::{
    App, Axis, BarChart, Color, Colormap, Corner, Frame, Grid, HLine, HeatMap, Line, LineStyle,
    MarkerShape, PieChart, Plot, PlotBuffer, PlotPoints, Points, ScrollArea, Track, VLine,
    WindowOptions,
};

struct Demo {
    t1: Vec<[f64; 2]>,
    t2: Vec<[f64; 2]>,
    sensor: PlotBuffer,
    last_sample: f64,
    scatter: Vec<[f64; 2]>,
    samples: Vec<f64>,
    heat: Vec<f64>,
}

/// A deterministic pseudo-random number in 0..1.
fn noise(i: u32) -> f64 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    f64::from(x % 10_000) / 10_000.0
}

impl Demo {
    fn new() -> Self {
        let t1 = (0..200)
            .map(|i| {
                let t = f64::from(i) * 0.5;
                [t, 20.0 + 8.0 * (t / 12.0).sin() + noise(i) * 1.5]
            })
            .collect();
        let t2 = (0..200)
            .map(|i| {
                let t = f64::from(i) * 0.5;
                [t, 24.0 + 5.0 * (t / 9.0 + 1.0).cos() + noise(i + 999) * 1.5]
            })
            .collect();
        let scatter = (0..150)
            .map(|i| {
                let x = noise(i) * 10.0;
                [x, x * 0.8 + (noise(i + 500) - 0.5) * 4.0]
            })
            .collect();
        // Roughly normal: the sum of a few uniform numbers.
        let samples = (0..2000)
            .map(|i| (0..6).map(|k| noise(i * 7 + k)).sum::<f64>() * 10.0)
            .collect();
        let heat = (0..20 * 12)
            .map(|i| {
                let (x, y) = (f64::from(i % 20), f64::from(i / 20));
                ((x / 3.0).sin() * (y / 2.5).cos() * 50.0 + 50.0).round()
            })
            .collect();
        Self {
            t1,
            t2,
            sensor: PlotBuffer::new(600),
            last_sample: 0.0,
            scatter,
            samples,
            heat,
        }
    }
}

impl App for Demo {
    fn update(&mut self, frame: &mut Frame) {
        frame.clear_color = frame.ctx().style().visuals.background;
        // A new sensor reading every 50 ms.
        let now = frame.time.as_secs_f64();
        while self.last_sample + 0.05 <= now {
            self.last_sample += 0.05;
            let t = self.last_sample;
            let i = (t * 20.0) as u32;
            self.sensor.push(
                t,
                (t * 1.3).sin() * 3.0 + (t * 0.21).sin() * 6.0 + noise(i) - 0.5,
            );
        }
        let red = frame.ctx().style().visuals.error;
        frame.ui(|ui| {
            ScrollArea::vertical().show(ui, |ui| {
                Grid::new("charts")
                    .columns([Track::Fraction(1.0), Track::Fraction(1.0)])
                    .align_y(rustroke::StackAlign::Start)
                    .gap(16.0)
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.heading("Two temperatures");
                            Plot::new("temps")
                                .x_axis(Axis::new().label("Time [s]"))
                                .y_axis(Axis::new().label("°C").format(|v| format!("{v:.1}")))
                                .legend(Corner::TopLeft)
                                .show(ui, |plot| {
                                    plot.line(
                                        Line::new(&self.t1)
                                            .name("Temperature 1")
                                            .mark_min_max(true),
                                    );
                                    plot.line(
                                        Line::new(&self.t2)
                                            .name("Temperature 2")
                                            .style(LineStyle::dashed())
                                            .width(2.0),
                                    );
                                    plot.hline(
                                        HLine::new(28.0)
                                            .name("Limit")
                                            .color(red)
                                            .style(LineStyle::dotted()),
                                    );
                                });
                        });
                        ui.vertical(|ui| {
                            ui.heading("Live sensor (last 10 s)");
                            Plot::new("live")
                                .follow_x(10.0)
                                .x_axis(
                                    Axis::new()
                                        .label("Time [s]")
                                        .format(|v| format!("{v:.0} s")),
                                )
                                .y_axis(Axis::new().range(-12.0..=12.0))
                                .show(ui, |plot| {
                                    plot.line(Line::new(&self.sensor).name("Sensor").fill(0.0));
                                });
                        });
                        ui.end_row();

                        ui.vertical(|ui| {
                            ui.heading("Area and function");
                            Plot::new("area").include_y(0.0).show(ui, |plot| {
                                plot.line(
                                    Line::new(PlotPoints::from_fn(0.0..=10.0, 200, |x| {
                                        (x * 1.5).sin() * x
                                    }))
                                    .name("x · sin(1.5x)")
                                    .fill(0.0),
                                );
                                plot.vline(VLine::new(5.0).style(LineStyle::dashed()));
                            });
                        });
                        ui.vertical(|ui| {
                            ui.heading("Scatter");
                            Plot::new("scatter")
                                .legend(Corner::BottomRight)
                                .show(ui, |plot| {
                                    plot.points(
                                        Points::new(&self.scatter)
                                            .name("Samples")
                                            .shape(MarkerShape::Diamond),
                                    );
                                    plot.line(
                                        Line::new(&[[0.0, 0.0], [10.0, 8.0]])
                                            .name("Trend")
                                            .width(2.0),
                                    );
                                });
                        });
                        ui.end_row();

                        ui.vertical(|ui| {
                            ui.heading("Stacked bars");
                            let a = BarChart::new(
                                (1..=6).map(|m| (f64::from(m), 3.0 + noise(m) * 5.0)),
                            )
                            .name("Product A");
                            let b = BarChart::new(
                                (1..=6).map(|m| (f64::from(m), 2.0 + noise(m + 50) * 4.0)),
                            )
                            .name("Product B")
                            .stack_on(&a);
                            Plot::new("bars")
                                .include_y(0.0)
                                .legend(Corner::TopLeft)
                                .x_axis(
                                    Axis::new()
                                        .ticks(rustroke::Ticks::Step(1.0))
                                        .minor_ticks(false)
                                        .format(|m| {
                                            ["", "Jan", "Feb", "Mar", "Apr", "May", "Jun"]
                                                .get(m as usize)
                                                .copied()
                                                .unwrap_or("")
                                                .to_owned()
                                        }),
                                )
                                .show(ui, |plot| {
                                    plot.bar_chart(a);
                                    plot.bar_chart(b);
                                });
                        });
                        ui.vertical(|ui| {
                            ui.heading("Histogram");
                            Plot::new("hist").include_y(0.0).show(ui, |plot| {
                                plot.bar_chart(
                                    BarChart::histogram(&self.samples, 30).name("Samples"),
                                );
                            });
                        });
                        ui.end_row();

                        ui.vertical(|ui| {
                            ui.heading("Heat map");
                            Plot::new("heat").allow_drag(false).allow_zoom(false).show(
                                ui,
                                |plot| {
                                    plot.heatmap(
                                        HeatMap::new(&self.heat[..], 20)
                                            .colormap(Colormap::Viridis)
                                            .name("Field"),
                                    );
                                },
                            );
                        });
                        ui.vertical(|ui| {
                            ui.heading("Logarithmic scale");
                            Plot::new("log")
                                .y_axis(Axis::new().log(true).label("Count"))
                                .show(ui, |plot| {
                                    plot.line(
                                        Line::new(PlotPoints::from_fn(0.0..=10.0, 100, |x| {
                                            10f64.powf(x / 2.0)
                                        }))
                                        .name("10^(x/2)")
                                        .markers(MarkerShape::Circle),
                                    );
                                });
                        });
                        ui.end_row();

                        ui.vertical(|ui| {
                            ui.heading("Donut");
                            PieChart::new(vec![
                                ("Rust".into(), 62.0),
                                ("C++".into(), 21.0),
                                ("Python".into(), 11.0),
                                ("Other".into(), 6.0),
                            ])
                            .donut(0.55)
                            .show(ui);
                        });
                        ui.vertical(|ui| {
                            ui.heading("Pie");
                            PieChart::new(vec![
                                ("North".into(), 30.0),
                                ("South".into(), 45.0),
                                ("East".into(), 25.0),
                            ])
                            .colors(vec![
                                Color::from_srgb8(97, 175, 239),
                                Color::from_srgb8(229, 192, 123),
                                Color::from_srgb8(152, 195, 121),
                            ])
                            .show(ui);
                        });
                        ui.end_row();
                    });
            });
        });
    }
}

fn main() -> Result<(), rustroke::RunError> {
    rustroke::run(
        WindowOptions {
            title: "Rustroke — charts".into(),
            inner_size: (1100.0, 860.0),
            ..Default::default()
        },
        Demo::new(),
    )
}
