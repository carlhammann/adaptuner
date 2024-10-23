use std::cmp::Ordering;

use egui::{
    widgets, Align, Align2, CentralPanel, Context, FontId, Frame, Layout, Slider, TopBottomPanel,
};
use emath::{vec2, Pos2, Rect, Vec2};
use epaint::{Color32, PathStroke, Shape};

use adaptuner::{
    interval::{
        stack::Stack,
        stacktype::{
            fivelimit::ConcreteFiveLimitStackType,
            r#trait::{FiveLimitStackType, StackCoeff, StackType},
        },
    },
    notename::NoteNameStyle,
    notestore::{TunedNoteStore, Tuning},
};

fn main() -> eframe::Result {
    eframe::run_native(
        "foo",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(State::new()))),
    )
}

impl<T> eframe::App for State<T>
where
    T: FiveLimitStackType + std::hash::Hash, // TODO remove the hash constraint, when we've found
                                             // something better to hash than the Stacks
{
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widgets::global_theme_preference_switch(ui);
            });
        });
        TopBottomPanel::bottom("slider_panel").show(ctx, |ui| {
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
            ui.add(Slider::new(&mut self.max_coeff_dist, 0..=20).text("max distance"));
            ui.add(
                Slider::new(&mut self.zoom, 0.1..=10.0)
                    .smart_aim(false)
                    .logarithmic(true)
                    .text("zoom"),
            );
        });
        CentralPanel::default().show(ctx, |ui| {
            Frame::canvas(ui.style()).show(ui, |ui| {
                let desired_size = ui.available_size();
                let (_id, rect) = ui.allocate_space(desired_size);

                let mut directions = self.directions.clone();
                for d in directions.iter_mut() {
                    *d = *d * self.zoom;
                }

                let mut background_lines = vec![];
                let mut background_text = vec![];
                let mut lines = vec![];
                let mut text = vec![];

                let stack_pos = |stack: &Stack<T>| {
                    let mut res = rect.center();
                    for (i, &c) in stack.coefficients().iter().enumerate() {
                        res += self.directions[i] * c as f32;
                    }
                    res
                };

                let mut draw_neighbours = |stack: &Stack<T>| {
                    let p0 = stack_pos(stack);
                    for (p, c) in PointsInsideRect::new(
                        &rect,
                        &directions,
                        self.max_coeff_dist,
                        &p0,
                        &self.active_temperaments,
                        stack,
                    ) {
                        for (i, &d) in directions.iter().enumerate() {
                            background_lines.push(Shape::line_segment(
                                [p, p + d],
                                PathStroke::new(1.0, ui.style().visuals.gray_out(self.colors[i])),
                            ));
                            background_lines.push(Shape::line_segment(
                                [p, p - d],
                                PathStroke::new(1.0, ui.style().visuals.gray_out(self.colors[i])),
                            ));
                        }

                        ctx.fonts(|fonts| {
                            background_text.push(Shape::text(
                                fonts,
                                p,
                                Align2::CENTER_CENTER,
                                c.notename(&NoteNameStyle::JohnstonFiveLimitFull),
                                FontId::proportional(15.0),
                                ui.style().visuals.weak_text_color(),
                            ));
                        });
                    }
                    //if ui
                    //    .interact(
                    //        Rect::from_center_size(p, vec2(20.0, 20.0)),
                    //        Id::new(stack.clone()), // todo this should work without cloning: what's a
                    //        // good id?
                    //        Sense::hover(),
                    //    )
                    //    .hovered()
                    //{
                    //    lines.push(Shape::line_segment(
                    //        [pos2(rect.left(), p.y), pos2(rect.right(), p.y)],
                    //        PathStroke::new(1.0, ui.style().visuals.weak_text_color()),
                    //    ));
                    //    let mut tmp_coeff = stack.coefficients().to_vec();
                    //    let mut start = p.clone();
                    //    ctx.fonts(|fonts| {
                    //        text.push(Shape::text(
                    //            fonts,
                    //            p,
                    //            Align2::CENTER_CENTER,
                    //            stack.notename(&NoteNameStyle::JohnstonFiveLimitFull),
                    //            FontId::proportional(15.0),
                    //            ui.style().visuals.strong_text_color(),
                    //        ))
                    //    });
                    //    while tmp_coeff != self.reference.coefficients() {
                    //        for (j, &d) in directions.iter().enumerate() {
                    //            if self.reference.coefficients()[j] != tmp_coeff[j] {
                    //                let incr = if self.reference.coefficients()[j] > tmp_coeff[j] {
                    //                    1
                    //                } else {
                    //                    -1
                    //                };
                    //                let end = start + incr as f32 * d;
                    //                lines.push(tipped_line(start, end, 4.0, self.colors[j]));
                    //                start = end;
                    //                tmp_coeff[j] += incr;
                    //                break;
                    //            }
                    //        }
                    //    }
                    //}
                };

                let mut hightlight_notename = |stack: &Stack<T>| {
                    let p = stack_pos(stack);
                    ctx.fonts(|fonts| {
                        text.push(Shape::text(
                            fonts,
                            p,
                            Align2::CENTER_CENTER,
                            stack.notename(&NoteNameStyle::JohnstonFiveLimitFull),
                            FontId::proportional(15.0),
                            ui.style().visuals.strong_text_color(),
                        ));
                    });
                };

                let mut draw_path = |width: f32, from: &Stack<T>, to: &Stack<T>| {
                    let mut tmp_coeff = from.coefficients().to_vec();
                    let mut start = stack_pos(from);
                    while tmp_coeff != to.coefficients() {
                        for (j, &d) in directions.iter().enumerate() {
                            if to.coefficients()[j] != tmp_coeff[j] {
                                let incr = if to.coefficients()[j] > tmp_coeff[j] {
                                    1
                                } else {
                                    -1
                                };
                                let end = start + incr as f32 * d;
                                lines.push(tipped_line(start, end, width, self.colors[j]));
                                start = end;
                                tmp_coeff[j] += incr;
                                break;
                            }
                        }
                    }
                };

                for (s, parent) in self.notes.iter_with_parent() {
                    draw_neighbours(&s.stack);
                    hightlight_notename(&s.stack);
                    match parent {
                        None => {}
                        Some(t) => {
                            if s.status.is_on() {
                                draw_path(4.0, &s.stack, &t.stack)
                            } else {
                                draw_path(2.0, &s.stack, &t.stack)
                            }
                        }
                    }
                }

                ui.painter().with_clip_rect(rect).extend(background_lines);
                ui.painter().with_clip_rect(rect).extend(lines);
                ui.painter().with_clip_rect(rect).extend(background_text);
                ui.painter().with_clip_rect(rect).extend(text);
            });
        });
    }
}

fn tipped_line(start: Pos2, end: Pos2, width: f32, color: Color32) -> Shape {
    let v = (end - start).normalized() * width.min(end.distance(start) / 2.0);
    let w = v.clone().rot90();
    Shape::convex_polygon(
        vec![
            start,
            start + v + w,
            end - v + w,
            end,
            end - v - w,
            start + v - w,
        ],
        color,
        PathStroke::NONE,
    )
}

struct PointsInsideRect<'a, T: StackType> {
    bounding_box: &'a Rect,
    directions: &'a [Vec2],
    max_coeff_dist: StackCoeff,
    active_temperaments: &'a [bool],
    start_stack: &'a Stack<T>,
    discovered: Vec<(StackCoeff, Stack<T>, Pos2)>,
}

impl<'a, T: StackType> PointsInsideRect<'a, T> {
    fn new(
        bounding_box: &'a Rect,
        directions: &'a [Vec2],
        max_coeff_dist: StackCoeff,
        reference_pos: &'a Pos2,
        active_temperaments: &'a [bool],
        start_stack: &'a Stack<T>,
    ) -> Self {
        PointsInsideRect {
            bounding_box,
            directions,
            max_coeff_dist,
            active_temperaments,
            start_stack,
            discovered: {
                let mut pos = reference_pos.clone();
                for (i, &c) in start_stack.coefficients().iter().enumerate() {
                    pos += c as f32 * directions[i];
                }
                vec![(0, start_stack.clone(), pos)]
            },
        }
    }
}

impl<'a, T: StackType> Iterator for PointsInsideRect<'a, T> {
    type Item = (Pos2, Stack<T>);
    fn next(&mut self) -> Option<Self::Item> {
        match self.discovered.pop() {
            None {} => return None,
            Some((coeff_dist, stack, pos)) => {
                let mut insert_new_coeff = |dimension: usize, up_or_down: bool, direction: Vec2| {
                    let updated_coeff = |i| {
                        if i == dimension {
                            if up_or_down {
                                stack.coefficients()[i] + 1
                            } else {
                                stack.coefficients()[i] - 1
                            }
                        } else {
                            stack.coefficients()[i]
                        }
                    };
                    let n = T::num_intervals();
                    let mut new_coeff_dist = 0;
                    for i in 0..n {
                        new_coeff_dist +=
                            (updated_coeff(i) - self.start_stack.coefficients()[i]).abs();
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
                                    if c.coefficients()[i] < updated_coeff(i) {
                                        res = Ordering::Greater;
                                        break;
                                    }
                                    if c.coefficients()[i] > updated_coeff(i) {
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
                                let mut new_stack = stack.clone();
                                if up_or_down {
                                    new_stack.increment_at(self.active_temperaments, dimension, 1);
                                } else {
                                    new_stack.increment_at(self.active_temperaments, dimension, -1);
                                }
                                if self.bounding_box.contains(new_pos) {
                                    self.discovered
                                        .insert(index, (new_coeff_dist, new_stack, new_pos));
                                }
                            }
                        }
                    }
                };
                for (i, &v) in self.directions.iter().enumerate() {
                    insert_new_coeff(i, true, v);
                    insert_new_coeff(i, false, -v);
                }
                Some((pos, stack))
            }
        }
    }
}

struct State<T: StackType> {
    directions: Vec<Vec2>,
    max_coeff_dist: StackCoeff,
    zoom: f32,
    colors: Vec<Color32>,

    //reference: Stack<T>,
    //active: Vec<Stack<T>>,
    active_temperaments: Vec<bool>,
    notes: TunedNoteStore<T>,
}

impl State<ConcreteFiveLimitStackType> {
    fn new() -> Self {
        let active_temperaments = vec![false; ConcreteFiveLimitStackType::num_temperaments()];
        let mut notes = TunedNoteStore::new(Stack::new_zero(), Tuning::FromMidiNote(60.0));

        State {
            directions: vec![
                vec2(0.0, -120.0),
                vec2(-100.0, -70.19550008653874),
                vec2(45.0, -38.63137138648348),
            ],
            max_coeff_dist: 2,
            zoom: 1.0,
            colors: vec![Color32::RED, Color32::GREEN, Color32::BLUE],

            //reference: Stack::new(&active_temperaments, vec![-1, 2, 0]),
            //active: vec![
            //    Stack::new(&active_temperaments, vec![2, -1, 1]),
            //    Stack::new(&active_temperaments, vec![0, 0, 0]),
            //    Stack::new(&active_temperaments, vec![-1, -1, 0]),
            //],
            active_temperaments,
            notes,
        }
    }
}
