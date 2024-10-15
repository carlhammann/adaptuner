use std::cmp::Ordering;

use egui::{
    self, debug_text::print, widgets, Align, Align2, CentralPanel, Context, FontId, Frame, Layout,
    Stroke, TopBottomPanel,
};
use emath::{self, pos2, vec2, Pos2, Rect, TSTransform, Vec2};
use epaint::{Color32, PathStroke, Shape, PathShape, TextShape};

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
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                //ui.heading("adaptuner");
                widgets::global_theme_preference_switch(ui);
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            Frame::canvas(ui.style()).show(ui, |ui| {
                //ui.ctx().request_repaint();
                let desired_size = ui.available_size(); // ui.available_width() * vec2(1.0, 0.35);
                let (_id, rect) = ui.allocate_space(desired_size);
                //let rect = ui.available_rect();

                //let to_screen = emath::RectTransform::from_to(
                //    Rect::from_x_y_ranges(0.0..=1.0, 0.0..=1.0),
                //    rect,
                //);

                let mut shapes = vec![];
                let mut points = vec![];

                for (i, (p, d)) in PointsInsideRect::new(
                    &rect,
                    &vec![vec2(60.0, 40.0), vec2(80.0, -20.0)],
                    &pos2(100.0, 100.0),
                    &vec![0, 0],
                )
                .enumerate()
                {
                    //shapes.push(Shape::circle_filled(p, 10.0, Color32::RED));
                    points.push(p);
                    ctx.fonts(|fonts| {
                        shapes.push(Shape::text(
                            fonts,
                            p,
                            Align2::CENTER_CENTER,
                            format!("{} {}", i, d),
                            FontId::monospace(10.0),
                            Color32::WHITE,
                        ))
                    });
                }

                shapes.push(Shape::Path(PathShape::line(points, PathStroke::new(1.0, Color32::WHITE))));
                ui.painter().with_clip_rect(rect).extend(shapes);
            });
        });
    }
}

//struct DiscoveredPoint {
//    coeff_dist: Coeff,
//    coeff: Vec<Coeff>,
//    pos: Pos2,
//}

//impl PartialEq for DiscoveredPoint {
//    fn eq(&self, other: &Self) -> bool {
//        self.coeff == other.coeff
//    }
//}

//impl Eq for DiscoveredPoint {}
//
//impl PartialOrd for DiscoveredPoint {
//    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//        Some(self.cmp(other))
//    }
//}
//
//impl Ord for DiscoveredPoint {
//    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//        self.coeff_dist.cmp(&other.coeff_dist)
//    }
//}

struct PointsInsideRect<'a> {
    bounding_box: &'a Rect,
    directions: &'a [Vec2],
    //start_pos: &'a Pos2,
    start_coeff: &'a [Coeff],
    discovered: Vec<(Coeff, Vec<Coeff>, Pos2)>,
}

impl<'a> PointsInsideRect<'a> {
    fn new(
        bounding_box: &'a Rect,
        directions: &'a [Vec2],
        start_pos: &'a Pos2,
        start_coeff: &'a [Coeff],
    ) -> Self {
        let discovered = vec![(0, start_coeff.to_vec(), start_pos.clone())];
        PointsInsideRect {
            bounding_box,
            directions,
            //start_pos,
            start_coeff,
            discovered,
        }
    }
}

impl<'a> Iterator for PointsInsideRect<'a> {
    type Item = (Pos2, Coeff);
    fn next(&mut self) -> Option<Self::Item> {
        match self.discovered.pop() {
            None {} => return None,
            Some((coeff_dist, coeff, pos)) => {
                let mut insert_new_coeff = |dimension: usize, up_or_down: bool, direction: Vec2| {
                    let mut new_coeff_dist = 0;
                    for (j, &c) in coeff.iter().enumerate() {
                        let x = if j == dimension {
                            if up_or_down {
                                c + 1
                            } else {
                                c - 1
                            }
                        } else {
                            c
                        };
                        new_coeff_dist += (x - self.start_coeff[j]).abs();
                    }

                    if new_coeff_dist > coeff_dist {
                        let mut new_coeff = coeff.clone();
                        if up_or_down {
                            new_coeff[dimension] += 1;
                        } else {
                            new_coeff[dimension] -= 1;
                        }
                        match self.discovered.binary_search_by(|(d, c, _)| {
                            if *d < new_coeff_dist {
                                Ordering::Greater
                            } else if *d > new_coeff_dist {
                                Ordering::Less
                            } else {
                                c.cmp(&new_coeff)
                            }
                        }) {
                            Ok(_) => {} // the new point is already in the queue
                            Err(index) => {
                                let mut new_pos = pos.clone();
                                new_pos += direction;
                                if self.bounding_box.contains(new_pos) {
                                    self.discovered
                                        .insert(index, (new_coeff_dist, new_coeff, new_pos));
                                }
                            }
                        }
                    }
                };
                for (i, &v) in self.directions.iter().enumerate() {
                    //let mut new_coeff = coeff.clone();
                    //new_coeff[i] += 1;
                    insert_new_coeff(i, true, v);
                    insert_new_coeff(i, false, -v);
                }
                Some((pos, coeff_dist))
            }
        }
    }
}

struct Ranges<'a, A> {
    lower: &'a [A],
    upper: &'a [A],
    current: Vec<A>,
}

impl<'a, A> Ranges<'a, A>
where
    A: PartialOrd + num_traits::NumAssignOps + num_traits::NumOps + num_traits::One + Copy,
{
    fn new(lower: &'a [A], upper: &'a [A]) -> Self {
        Ranges {
            lower,
            upper,
            current: Vec::from(lower),
        }
    }

    fn next(&mut self) -> Option<&[A]> {
        let n = self.upper.len();

        let increment_at = |cur: &mut [A], i: usize| {
            for j in (i + 1)..n {
                cur[j] = self.lower[j];
            }
            if self.lower[i] < self.upper[i] {
                cur[i] += A::one();
            } else if self.lower[i] > self.upper[i] {
                cur[i] -= A::one();
            }
        };

        let mut i = n - 1;
        loop {
            if i == 0 {
                if self.current[0] == self.upper[0] {
                    return None;
                } else {
                    increment_at(&mut self.current, 0);
                    return Some(&self.current);
                }
            } else if self.current[i] == self.upper[i] {
                i -= 1;
            } else {
                increment_at(&mut self.current, i);
                return Some(&self.current);
            }
        }
    }

    fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(&[A]),
    {
        let mut p: Option<&[A]> = Some(&self.current);
        while p.is_some() {
            f(p.expect("this can't happen: option is_some(), but expect() fails."));
            p = self.next();
        }
    }
}

enum Drawable {
    AText(TextShape),
    ALineSegment {
        points: [Pos2; 2],
        stroke: PathStroke,
    },
}

type Coeff = i32;

struct State {
    directions: Vec<Vec2>,
    colors: Vec<Color32>,
    active: Vec<Vec<Coeff>>,

    bounding_box: Rect,
    min: Vec2,
    max: Vec2,
    to_draw: Vec<Drawable>,
}

impl State {
    fn new() -> Self {
        State {
            directions: vec![
                vec2(-1.0, (3.0 / 2.0_f32).log2()),
                vec2(1.0, (5.0_f32).log2()),
            ],
            colors: vec![Color32::from_rgb(255, 0, 0), Color32::from_rgb(0, 255, 0)],
            active: vec![vec![3, 2]],
            bounding_box: Rect::NOTHING,
            min: vec2(0.0, 0.0),
            max: vec2(0.0, 0.0),
            to_draw: vec![],
        }
    }
}

//
//    /// The length of the argument Vec must be equal to the length of self.directions
//    fn coeffs_to_vector(&self, x: &[Coeff]) -> Vec2 {
//        let mut res = vec2(0.0, 0.0);
//        for (i, &c) in x.iter().enumerate() {
//            res += self.directions[i] * c as f32;
//        }
//        res
//    }
//
//    fn transform(&self) -> TSTransform {
//        todo!()
//        //Transform2D::translation(-self.min.x, -self.min.y)
//        //    .then_scale(
//        //        if self.min.x == self.max.x {
//        //            0.0
//        //        } else {
//        //            self.bounding_box.width / (self.max.x - self.min.x)
//        //        },
//        //        if self.min.y == self.max.y {
//        //            0.0
//        //        } else {
//        //            self.bounding_box.height / (self.min.y - self.max.y) // this will flip
//        //        },
//        //    )
//        //    .then_translate(vec2(
//        //        self.bounding_box.x,
//        //        self.bounding_box.y + self.bounding_box.height,
//        //    ))
//    }
//
//    fn update_min_max(&mut self, v: &Vec2) {
//        self.min.x = self.min.x.min(v.x);
//        self.max.x = self.max.x.max(v.x);
//        self.min.y = self.min.y.min(v.y);
//        self.max.y = self.max.y.max(v.y);
//        //println!(
//        //    "min.x={} min.y={} max.x={} max.y={}",
//        //    self.min.x, self.min.y, self.max.x, self.max.y
//        //);
//    }
//
//    fn add_limb(&mut self, x: &[Coeff], dimension: usize, direction: bool, stroke: &PathStroke) {
//        let start = self.state.coeffs_to_vector(x);
//        self.tmp_coeff.clone_from_slice(x);
//        if direction {
//            self.tmp_coeff[dimension] += 1;
//        } else {
//            self.tmp_coeff[dimension] -= 1;
//        }
//        let end = self.state.coeffs_to_vector(&self.tmp_coeff);
//
//        self.update_min_max(&start);
//        self.update_min_max(&end);
//
//        //self.to_draw.push(Drawable::StrokePath(
//        //    Path::line(Point::new(start.x, start.y), Point::new(end.x, end.y)),
//        //    stroke.with_color(self.state.colors[dimension]),
//        //));
//        //
//        //self.to_draw.push(Drawable::AText(Text {
//        //    position: Point::new(start.x, start.y),
//        //    ..Text::from(format!("({},{})", x[0], x[1]))
//        //}));
//        //
//        //self.to_draw.push(Drawable::HorizontalLine(start.y));
//    }
//
//    fn add_path_to(&mut self, target: &[Coeff]) {
//        let zero = vec![0; self.state.directions.len()];
//        let limb_stroke = PathStroke::default();
//        //Stroke::default()
//        //       .with_width(7.0)
//        //       .with_line_cap(LineCap::Round),
//        Ranges::new(&zero, target).for_each(|waypoint| {
//            for (dimension, &target_val) in target.iter().enumerate() {
//                if waypoint[dimension] != target_val {
//                    self.add_limb(waypoint, dimension, target_val >= 0, &limb_stroke);
//                }
//            }
//        });
//    }
//
//    //fn add_grid(&mut self) {
//    //    let n = self.state.directions.len();
//    //    let grid_stroke = Stroke::default().with_width(2.0);
//    //
//    //    let mut min = vec![-1; n];
//    //    let mut max = vec![1; n];
//    //
//    //    for v in &self.state.active {
//    //        for i in 0..n {
//    //            min[i] = min[i].min(v[i] - 1);
//    //            max[i] = max[i].max(v[i] + 1);
//    //        }
//    //    }
//    //    Ranges::new(&min, &max).for_each(|waypoint| {
//    //        for dimension in 0..n {
//    //            self.add_limb(waypoint, dimension, true, grid_stroke);
//    //            self.add_limb(waypoint, dimension, false, grid_stroke);
//    //        }
//    //    });
//    //
//    //    //for i in 0..n {
//    //    //    tmp[i] = 0;
//    //    //}
//    //    //for v in &self.state.active {
//    //    //    for i in 0..n {
//    //    //        tmp[i] = tmp[i].max(v[i] + 1);
//    //    //    }
//    //    //}
//    //    //Ranges::new(&tmp).for_all(|waypoint| {
//    //    //    for dimension in 0..n {
//    //    //        self.add_limb(waypoint, dimension, true, grid_stroke);
//    //    //    }
//    //    //});
//    //}
//
//    // will drain [to_draw]
//    //fn do_draw(&mut self, frame: &mut Frame<Renderer>) {
//    //    let trans = self.transform();
//    //
//    //    self.to_draw.sort_by(|a, b| match (a, b) {
//    //        (Shape::TextShape { .. }, Shape::TextShape { .. }) => Ordering::Equal,
//    //        (Shape::TextShape { .. }, _) => Ordering::Greater,
//    //        (_, Shape::TextShape { .. }) => Ordering::Less,
//    //        (_, _) => Ordering::Equal,
//    //    });
//    //
//    //    for what in &mut self.to_draw.drain(0..) {
//    //        match what {
//    //            Drawable::StrokePath(mut p, s) => {
//    //                p = p.transform(&trans);
//    //                frame.stroke(&p, s);
//    //            }
//    //            Drawable::FillPath(mut p, f) => {
//    //                p = p.transform(&trans);
//    //                frame.fill(&p, f);
//    //            }
//    //            Drawable::AText(mut t) => {
//    //                let r = trans.transform_point(Point2D::new(t.position.x, t.position.y));
//    //                t.position.x = r.x;
//    //                t.position.y = r.y;
//    //                frame.fill_text(t);
//    //            }
//    //            Drawable::HorizontalLine(y) => {
//    //                let r1 = trans.transform_point(Point2D::new(self.min.x, y));
//    //                let r2 = trans.transform_point(Point2D::new(self.max.x, y));
//    //                frame.stroke(
//    //                    &Path::line(Point::new(r1.x, r1.y), Point::new(r2.x, r2.y + 0.01)),
//    //                    Stroke::default(),
//    //                );
//    //            }
//    //        }
//    //    }
//    //}
//}
