use std::cmp::Ordering;

use egui::{
    pos2, widgets, Align, Align2, CentralPanel, Context, FontId, Frame, Id, Layout, Sense, Slider,
    Stroke, TopBottomPanel,
};
use emath::{vec2, Pos2, Rect, Vec2};
use epaint::{Color32, PathStroke, Shape};

use adaptuner::{
    interval::{
        stack::Stack,
        stacktype::{fivelimit::ConcreteFiveLimitStackType, r#trait::StackCoeff},
    },
    notename::NoteNameStyle,
};

fn main() -> eframe::Result {
    eframe::run_native(
        "foo",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(State::new()))),
    )
}

impl eframe::App for State {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.add(
                Slider::new(&mut self.directions[0].x, -100.0..=100.0)
                    .smart_aim(false)
                    .text("x1"),
            );
            ui.add(
                Slider::new(&mut self.directions[1].x, -100.0..=100.0)
                    .smart_aim(false)
                    .text("x2"),
            );
            ui.add(
                Slider::new(&mut self.directions[2].x, -100.0..=100.0)
                    .smart_aim(false)
                    .text("x3"),
            );
            ui.add(Slider::new(&mut self.max_coeff_dist, 1..=20).text("max distance"));
            ui.add(
                Slider::new(&mut self.zoom, 0.1..=10.0)
                    .smart_aim(false)
                    .logarithmic(true)
                    .text("zoom"),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                //ui.heading("adaptuner");
                widgets::global_theme_preference_switch(ui);
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            Frame::canvas(ui.style()).show(ui, |ui| {
                let desired_size = ui.available_size();
                let (_id, rect) = ui.allocate_space(desired_size);

                let mut directions = self.directions.clone();
                for d in directions.iter_mut() {
                    *d = *d * self.zoom;
                }

                let mut grid_lines = vec![];
                let mut grid_text = vec![];
                let mut lines = vec![];
                let mut balls = vec![];
                let mut text = vec![];

                for (_i, (p, c)) in PointsInsideRect::new(
                    &rect,
                    &directions,
                    self.max_coeff_dist,
                    &rect.center(),
                    &self.reference,
                )
                .enumerate()
                {
                    let stack: Stack<ConcreteFiveLimitStackType> =
                        Stack::new(&vec![false; 2], c.clone());
                    ctx.fonts(|fonts| {
                        grid_text.push(Shape::text(
                            fonts,
                            p,
                            Align2::CENTER_CENTER,
                            stack.notename(&NoteNameStyle::JohnstonFiveLimitFull),
                            FontId::proportional(15.0),
                            ui.style().visuals.text_color(),
                        ))
                    });
                    for (i, &d) in directions.iter().enumerate() {
                        if c[i] < self.reference[i] {
                            grid_lines.push(Shape::line_segment(
                                [p, p + d],
                                PathStroke::new(1.0, ui.style().visuals.gray_out(self.colors[i])),
                            ));
                        }
                        if c[i] > self.reference[i] {
                            grid_lines.push(Shape::line_segment(
                                [p, p - d],
                                PathStroke::new(1.0, ui.style().visuals.gray_out(self.colors[i])),
                            ));
                        }
                    }
                    if ui
                        .interact(
                            Rect::from_center_size(p, vec2(20.0, 20.0)),
                            Id::new(c.clone()), // todo this shoudl work without cloning: what's a
                            // good id?
                            Sense::hover(),
                        )
                        .hovered()
                    {
                        lines.push(Shape::line_segment(
                            [pos2(rect.left(), p.y), pos2(rect.right(), p.y)],
                            PathStroke::new(1.0, ui.style().visuals.weak_text_color()),
                        ));
                        let mut tmp = p.clone();
                        let mut tmp_coeff = c.clone();
                        //balls.push(Shape::circle_filled(
                        //    tmp,
                        //    4.0,
                        //    ui.style().visuals.text_color(),
                        //));
                        ctx.fonts(|fonts| {
                            text.push(Shape::text(
                                fonts,
                                p,
                                Align2::CENTER_CENTER,
                                stack.notename(&NoteNameStyle::JohnstonFiveLimitFull),
                                FontId::proportional(15.0),
                                ui.style().visuals.strong_text_color(),
                            ))
                        });
                        while tmp_coeff != self.reference {
                            for (j, &d) in directions.iter().enumerate() {
                                if self.reference[j] != tmp_coeff[j] {
                                    let incr = if self.reference[j] > tmp_coeff[j] {
                                        1
                                    } else {
                                        -1
                                    };
                                    lines.push(Shape::line_segment(
                                        [tmp, tmp + incr as f32 * d],
                                        PathStroke::new(4.0, self.colors[j]),
                                    ));
                                    tmp += incr as f32 * d;
                                    tmp_coeff[j] += incr;
                                    break;
                                }
                            }
                            balls.push(Shape::circle_filled(
                                tmp,
                                4.0,
                                ui.style().visuals.text_color(),
                            ));
                        }
                    }
                }

                ui.painter().with_clip_rect(rect).extend(grid_lines);
                ui.painter().with_clip_rect(rect).extend(grid_text);
                ui.painter().with_clip_rect(rect).extend(lines);
                ui.painter().with_clip_rect(rect).extend(balls);
                ui.painter().with_clip_rect(rect).extend(text);
            });
        });
    }
}

struct PointsInsideRect<'a> {
    bounding_box: &'a Rect,
    directions: &'a [Vec2],
    max_coeff_dist: StackCoeff,
    start_coeff: &'a [StackCoeff],
    discovered: Vec<(StackCoeff, Vec<StackCoeff>, Pos2)>,
}

impl<'a> PointsInsideRect<'a> {
    fn new(
        bounding_box: &'a Rect,
        directions: &'a [Vec2],
        max_coeff_dist: StackCoeff,
        start_pos: &'a Pos2,
        start_coeff: &'a [StackCoeff],
    ) -> Self {
        let discovered = vec![(0, start_coeff.to_vec(), start_pos.clone())];
        PointsInsideRect {
            bounding_box,
            directions,
            max_coeff_dist,
            start_coeff,
            discovered,
        }
    }
}

impl<'a> Iterator for PointsInsideRect<'a> {
    type Item = (Pos2, Vec<StackCoeff>);
    fn next(&mut self) -> Option<Self::Item> {
        match self.discovered.pop() {
            None {} => return None,
            Some((coeff_dist, coeff, pos)) => {
                let mut insert_new_coeff = |dimension: usize, up_or_down: bool, direction: Vec2| {
                    let mut new_coeff_dist = 0;
                    let n = coeff.len();
                    let get_coeff = |i| {
                        if i == dimension {
                            if up_or_down {
                                coeff[i] + 1
                            } else {
                                coeff[i] - 1
                            }
                        } else {
                            coeff[i]
                        }
                    };
                    for i in 0..n {
                        new_coeff_dist += (get_coeff(i) - self.start_coeff[i]).abs();
                    }

                    if new_coeff_dist > coeff_dist && new_coeff_dist <= self.max_coeff_dist {
                        match self.discovered.binary_search_by(|(d, c, _)| {
                            if *d < new_coeff_dist {
                                Ordering::Greater
                            } else if *d > new_coeff_dist {
                                Ordering::Less
                            } else {
                                let mut res = Ordering::Equal;
                                for i in 0..n {
                                    if c[i] < get_coeff(i) {
                                        res = Ordering::Greater;
                                        break;
                                    }
                                    if c[i] > get_coeff(i) {
                                        res = Ordering::Less;
                                        break;
                                    }
                                }
                                res
                            }
                        }) {
                            Ok(_) => {} // the new point is already in the queue
                            Err(index) => {
                                let mut new_pos = pos.clone();
                                new_pos += direction;
                                let mut new_coeff = coeff.clone();
                                if up_or_down {
                                    new_coeff[dimension] += 1;
                                } else {
                                    new_coeff[dimension] -= 1;
                                }
                                if self.bounding_box.contains(new_pos) {
                                    self.discovered
                                        .insert(index, (new_coeff_dist, new_coeff, new_pos));
                                }
                            }
                        }
                    }
                };
                for (i, &v) in self.directions.iter().enumerate() {
                    insert_new_coeff(i, true, v);
                    insert_new_coeff(i, false, -v);
                }
                Some((pos, coeff))
            }
        }
    }
}

struct State {
    directions: Vec<Vec2>,
    max_coeff_dist: StackCoeff,
    zoom: f32,
    colors: Vec<Color32>,

    reference: Vec<StackCoeff>,
    //active: Vec<Vec<StackCoeff>>,
}

impl State {
    fn new() -> Self {
        State {
            directions: vec![
                vec2(0.0, -120.0),
                vec2(-100.0, -70.19550008653874),
                vec2(45.0, -38.63137138648348),
            ],
            max_coeff_dist: 2,
            zoom: 1.0,
            colors: vec![Color32::RED, Color32::GREEN, Color32::BLUE],
            reference: vec![1, -1, 1],
            //active: vec![], //vec![3, 2]],
        }
    }
}
