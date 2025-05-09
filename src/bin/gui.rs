use std::{marker::PhantomData, sync::mpsc, time::Instant};

use eframe::{
    self,
    egui::{self, vec2},
    epaint::pos2,
    App,
};
use midi_msg::{ChannelVoiceMsg, ControlChange, MidiMsg};

use adaptuner::{
    gui::r#trait::GUIState,
    interval::{
        stack::Stack,
        stacktype::{
            fivelimit::ConcreteFiveLimitStackType,
            r#trait::{FiveLimitStackType, StackCoeff, StackType},
        },
    },
    keystate::KeyState,
    msg,
    notename::johnston::fivelimit::{BaseName, NoteName},
};

const BASSCLEF: egui::ImageSource = egui::include_image!("../../assets/svg/bassclef.svg");
const TREBLECLEF: egui::ImageSource = egui::include_image!("../../assets/svg/trebleclef.svg");
const WHOLENOTE: egui::ImageSource = egui::include_image!("../../assets/svg/wholenote.svg");
const NATURAL: egui::ImageSource = egui::include_image!("../../assets/svg/natural.svg");
const SHARP: egui::ImageSource = egui::include_image!("../../assets/svg/sharp.svg");
const FLAT: egui::ImageSource = egui::include_image!("../../assets/svg/flat.svg");
const DOUBLESHARP: egui::ImageSource = egui::include_image!("../../assets/svg/doublesharp.svg");
const DOUBLEFLAT: egui::ImageSource = egui::include_image!("../../assets/svg/doubleflat.svg");

// All of these values were measured in Inkscape or extracted from the files in ../../assets/svg/
const MEASURED_LINE_SPACING: f32 = 7.029 / 4.0;
const MEASURED_LINE_THICKNESS: f32 = 0.176;
const MEASURED_LEDGER_LINE_THICKNESS: f32 = 0.3; // actually measured: 0.351, but that looks too thick
const MEASURED_TREBLECLEF_SIZE: egui::Vec2 = vec2(4.5197721, 12.856398);
const MEASURED_BASSCLEF_SIZE: egui::Vec2 = vec2(4.7236228, 5.454659);
const MEASURED_WHOLENOTE_SIZE: egui::Vec2 = vec2(3.4513346, 1.9119411);
const MEASURED_NATURAL_SIZE: egui::Vec2 = vec2(1.2722845, 5.3703051);
const MEASURED_SHARP_SIZE: egui::Vec2 = vec2(1.9330286, 5.2718959);
const MEASURED_FLAT_SIZE: egui::Vec2 = vec2(1.5956272, 4.4143343);
const MEASURED_DOUBLESHARP_SIZE: egui::Vec2 = vec2(1.8978827, 1.8978826);
const MEASURED_DOUBLEFLAT_SIZE: egui::Vec2 = vec2(2.7343567, 4.4143343);

// The following are in the units of [NoteRenderer::line_spacing]
const TREBLECLEF_OFFSET: egui::Vec2 = vec2(0.0, -0.0896);
const BASSCLEF_OFFSET: egui::Vec2 = vec2(0.0, 0.5);
const WHOLENOTE_OFFSET: egui::Vec2 = vec2(0.0, 0.0);
const NATURAL_OFFSET: egui::Vec2 = vec2(0.0, 0.0);
const SHARP_OFFSET: egui::Vec2 = vec2(0.0, 0.0);
const FLAT_OFFSET: egui::Vec2 = vec2(0.0, -0.55);
const DOUBLESHARP_OFFSET: egui::Vec2 = vec2(0.0, 0.0);
const DOUBLEFLAT_OFFSET: egui::Vec2 = vec2(0.0, -0.55);
const STACKED_NOTE_HORIZONTAL_OFFSET: f32 = 1.6626; // how far we move notes to the right in clusters
const ACCIDENTAL_ACCIDENTAL_SPACE: f32 = 0.3; // If one note receives several accidentals, this is the horizontal space between them
const NOTE_ACCIDENTAL_SPACE: f32 = 0.5; // The minimal horizontal space between a note an its accidental

struct SizedOffsetTexture {
    id: egui::TextureId,

    /// in units of [NoteRenderer::line_spacing]
    size: egui::Vec2,

    /// in units of [NoteRenderer::line_spacing]
    offset: egui::Vec2,
}

struct NoteShapes {
    bassclef: SizedOffsetTexture,
    trebleclef: SizedOffsetTexture,
    wholenote: SizedOffsetTexture,
    natural: SizedOffsetTexture,
    sharp: SizedOffsetTexture,
    flat: SizedOffsetTexture,
    doublesharp: SizedOffsetTexture,
    doubleflat: SizedOffsetTexture,
}

enum NoteShape {
    BassClef,
    TrebleClef,
    WholeNote,
    Natural,
    Sharp,
    Flat,
    DoubleSharp,
    DoubleFlat,
}

struct NoteRenderer<T: StackType> {
    _phantom: PhantomData<T>,

    /// The distance between note lines. (middle of line to middle of line) This is the unit for
    /// all other distances.
    line_spacing: f32,

    /// Thickness of note lines
    line_thickness: f32,

    /// How far the clef's centers are offset from the left margin
    clef_offset: f32,

    noteshapes: NoteShapes,
    x: f32,
}

impl NoteShapes {
    fn new(ctx: &egui::Context, line_spacing: f32) -> Self {
        let scale = line_spacing / MEASURED_LINE_SPACING;
        let init_noteshape = |measured_size, offset, source| {
            let actual_size = scale * measured_size;
            let image = egui::Image::new(source);
            match image.load_for_size(ctx, actual_size) {
                Ok(egui::load::TexturePoll::Ready {
                    texture: egui::load::SizedTexture { id, size: _ },
                }) => SizedOffsetTexture {
                    id,
                    size: actual_size / line_spacing,
                    offset
                },
                _ => panic!(
                    "{}",
                    format!("could not load image {:?} at size {}", image, actual_size)
                ),
            }
        };

        NoteShapes {
            bassclef: init_noteshape(MEASURED_BASSCLEF_SIZE, BASSCLEF_OFFSET, BASSCLEF),
            trebleclef: init_noteshape(MEASURED_TREBLECLEF_SIZE, TREBLECLEF_OFFSET, TREBLECLEF),
            wholenote: init_noteshape(MEASURED_WHOLENOTE_SIZE, WHOLENOTE_OFFSET, WHOLENOTE),
            natural: init_noteshape(MEASURED_NATURAL_SIZE, NATURAL_OFFSET, NATURAL),
            sharp: init_noteshape(MEASURED_SHARP_SIZE, SHARP_OFFSET, SHARP),
            flat: init_noteshape(MEASURED_FLAT_SIZE, FLAT_OFFSET, FLAT),
            doublesharp: init_noteshape(MEASURED_DOUBLESHARP_SIZE, DOUBLESHARP_OFFSET, DOUBLESHARP),
            doubleflat: init_noteshape(MEASURED_DOUBLEFLAT_SIZE, DOUBLEFLAT_OFFSET, DOUBLEFLAT),
        }
    }
}

impl<T: FiveLimitStackType> NoteRenderer<T> {
    fn new(ctx: &egui::Context, line_spacing: f32) -> Self {
        Self {
            _phantom: PhantomData,
            line_spacing,
            line_thickness: MEASURED_LINE_THICKNESS / MEASURED_LINE_SPACING,
            clef_offset: 3.0,
            noteshapes: NoteShapes::new(ctx, line_spacing),
            x: 0.0,
        }
    }

    fn reload_noteshapes(&mut self, ctx: &egui::Context) {
        ctx.forget_image(BASSCLEF.uri().unwrap());
        ctx.forget_image(TREBLECLEF.uri().unwrap());
        ctx.forget_image(WHOLENOTE.uri().unwrap());
        ctx.forget_image(SHARP.uri().unwrap());
        ctx.forget_image(FLAT.uri().unwrap());
        ctx.forget_image(DOUBLESHARP.uri().unwrap());
        ctx.forget_image(DOUBLEFLAT.uri().unwrap());

        self.noteshapes = NoteShapes::new(ctx, self.line_spacing);
    }

    fn draw_noteshape(
        &self,
        shape: NoteShape,
        center: egui::Pos2,
        tint: egui::Color32,
        clip_rect: egui::Rect,
        ui: &mut egui::Ui,
    ) {
        let sot = match shape {
            NoteShape::WholeNote => &self.noteshapes.wholenote,
            NoteShape::Sharp => &self.noteshapes.sharp,
            NoteShape::Flat => &self.noteshapes.flat,
            NoteShape::Natural => &self.noteshapes.natural,
            NoteShape::BassClef => &self.noteshapes.bassclef,
            NoteShape::TrebleClef => &self.noteshapes.trebleclef,
            NoteShape::DoubleSharp => &self.noteshapes.doublesharp,
            NoteShape::DoubleFlat => &self.noteshapes.doubleflat,
        };
        ui.painter().with_clip_rect(clip_rect).image(
            sot.id,
            egui::Rect::from_center_size(
                center + self.line_spacing * sot.offset,
                self.line_spacing * sot.size,
            ),
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            tint,
        );
    }

    fn draw_lines(&self, rect: egui::Rect, ui: &mut egui::Ui) {
        let mut y = rect.height() / 2.0;
        //ui.painter().with_clip_rect(rect).hline(
        //    rect.x_range(),
        //    y,
        //    egui::Stroke::new(
        //        self.line_thickness * self.line_spacing,
        //        ui.style().visuals.weak_text_color(),
        //    ),
        //);

        for _ in 0..5 {
            y -= self.line_spacing;
            ui.painter().with_clip_rect(rect).hline(
                rect.x_range(),
                y,
                egui::Stroke::new(
                    self.line_thickness * self.line_spacing,
                    ui.style().visuals.strong_text_color(),
                ),
            );
        }

        //y -= self.line_spacing;
        //for _ in 0..10 {
        //    if y < 0.0 {
        //        break;
        //    }
        //    ui.painter().with_clip_rect(rect).hline(
        //        rect.x_range(),
        //        y,
        //        egui::Stroke::new(
        //            self.line_thickness * self.line_spacing,
        //            ui.style().visuals.weak_text_color(),
        //        ),
        //    );
        //    y -= self.line_spacing;
        //}

        let mut y = rect.height() / 2.0;
        for _ in 0..5 {
            y += self.line_spacing;
            ui.painter().with_clip_rect(rect).hline(
                rect.x_range(),
                y,
                egui::Stroke::new(
                    self.line_thickness * self.line_spacing,
                    ui.style().visuals.strong_text_color(),
                ),
            );
        }

        //y += self.line_spacing;
        //for _ in 0..10 {
        //    if y > rect.bottom() {
        //        break;
        //    }
        //    ui.painter().with_clip_rect(rect).hline(
        //        rect.x_range(),
        //        y,
        //        egui::Stroke::new(
        //            self.line_thickness * self.line_spacing,
        //            ui.style().visuals.weak_text_color(),
        //        ),
        //    );
        //    y += self.line_spacing;
        //}
    }

    fn draw_clefs(&self, rect: egui::Rect, ui: &mut egui::Ui) {
        self.draw_noteshape(
            NoteShape::TrebleClef,
            pos2(
                self.line_spacing * self.clef_offset,
                rect.height() / 2.0 - 3.0 * self.line_spacing,
            ),
            ui.style().visuals.strong_text_color(),
            rect,
            ui,
        );

        self.draw_noteshape(
            NoteShape::BassClef,
            pos2(
                self.line_spacing * self.clef_offset,
                rect.height() / 2.0 + 2.0 * self.line_spacing,
            ),
            ui.style().visuals.strong_text_color(),
            rect,
            ui,
        );
    }

    fn draw_notehead_and_ledger_lines(
        &self,
        basename: BaseName,
        octave: StackCoeff,
        horizontal_pos: f32,
        rect: egui::Rect,
        ui: &mut egui::Ui,
    ) {
        let scale = self.line_spacing / MEASURED_LINE_SPACING;
        let vertical_index = (octave - 4) * 7 + (basename as StackCoeff);
        let vertical_pos = rect.height() / 2.0 - self.line_spacing * vertical_index as f32 / 2.0;
        self.draw_noteshape(
            NoteShape::WholeNote,
            pos2(horizontal_pos, vertical_pos),
            ui.style().visuals.strong_text_color(),
            rect,
            ui,
        );

        let ledger_line = |index| {
            let half_thickness = MEASURED_LEDGER_LINE_THICKNESS * scale / 2.0;
            let half_width = 1.25 * self.line_spacing;
            let vpos = rect.height() / 2.0 - self.line_spacing * index as f32 / 2.0;
            ui.painter().with_clip_rect(rect).rect_filled(
                egui::Rect {
                    min: pos2(horizontal_pos - half_width, vpos - half_thickness),
                    max: pos2(horizontal_pos + half_width, vpos + half_thickness),
                },
                half_thickness,
                ui.style().visuals.strong_text_color(),
            );
        };

        if vertical_index == 0 {
            ledger_line(0);
        }

        if vertical_index > 10 {
            for i in (12..=vertical_index).step_by(2) {
                ledger_line(i);
            }
        }

        if vertical_index < -10 {
            let mut i = vertical_index;
            if i.rem_euclid(2) == 1 {
                i += 1;
            }
            while i <= -12 {
                ledger_line(i);
                i += 2;
            }
        }
    }

    fn draw_accidental(
        &self,
        notename: NoteName,
        right_border: f32,
        rect: egui::Rect,
        ui: &mut egui::Ui,
    ) {
        let scale = self.line_spacing / MEASURED_LINE_SPACING;
        let vertical_index = (notename.octave - 4) * 7 + (notename.basename as StackCoeff);
        let vertical_pos = rect.height() / 2.0 - self.line_spacing * vertical_index as f32 / 2.0;
        let center = pos2(right_border, vertical_pos);
        let tint = ui.style().visuals.strong_text_color();
        match notename.sharpflat {
            0 => {}
            1 => self.draw_noteshape(NoteShape::Sharp, center, tint, rect, ui),
            -1 => self.draw_noteshape(NoteShape::Flat, center, tint, rect, ui),
            2 => self.draw_noteshape(NoteShape::DoubleSharp, center, tint, rect, ui),
            -2 => self.draw_noteshape(NoteShape::DoubleFlat, center, tint, rect, ui),
            n => {
                if n > 0 {
                    let q = n / 2;
                    let r = n % 2;
                    for i in 0..q {
                        self.draw_noteshape(
                            NoteShape::DoubleSharp,
                            center
                                - vec2(
                                    i as f32
                                        * (MEASURED_DOUBLESHARP_SIZE.x * scale
                                            + ACCIDENTAL_ACCIDENTAL_SPACE * self.line_spacing),
                                    0.0,
                                ),
                            tint,
                            rect,
                            ui,
                        );
                    }
                    if r == 1 {
                        self.draw_noteshape(
                            NoteShape::Sharp,
                            center
                                - vec2(
                                    q as f32
                                        * (MEASURED_DOUBLESHARP_SIZE.x * scale
                                            + ACCIDENTAL_ACCIDENTAL_SPACE * self.line_spacing),
                                    0.0,
                                ),
                            tint,
                            rect,
                            ui,
                        );
                    }
                } else {
                    let q = -n / 2;
                    let r = -n % 2;
                    for i in 0..q {
                        self.draw_noteshape(
                            NoteShape::DoubleFlat,
                            center
                                - vec2(
                                    i as f32
                                        * (MEASURED_DOUBLEFLAT_SIZE.x * scale
                                            + ACCIDENTAL_ACCIDENTAL_SPACE * self.line_spacing),
                                    0.0,
                                ),
                            tint,
                            rect,
                            ui,
                        );
                    }
                    if r == 1 {
                        self.draw_noteshape(
                            NoteShape::Flat,
                            center
                                - vec2(
                                    q as f32
                                        * (MEASURED_DOUBLEFLAT_SIZE.x * scale
                                            + ACCIDENTAL_ACCIDENTAL_SPACE * self.line_spacing),
                                    0.0,
                                ),
                            tint,
                            rect,
                            ui,
                        );
                    }
                }
            }
        }
    }

    fn draw_chord(
        &self,
        stacks: &[Stack<T>],
        horizontal_axis: f32,
        rect: egui::Rect,
        ui: &mut egui::Ui,
    ) {
        for stack in stacks {
            let n = NoteName::new(stack);
            self.draw_notehead_and_ledger_lines(n.basename, n.octave, horizontal_axis, rect, ui);
            self.draw_accidental(
                n,
                horizontal_axis - self.line_spacing * NOTE_ACCIDENTAL_SPACE,
                rect,
                ui,
            );
        }
    }

    fn draw(&self, active_notes: &[KeyState; 128], tunings: &[Stack<T>], ui: &mut egui::Ui) {
        let desired_size = ui.available_size();
        let (_id, rect) = ui.allocate_space(desired_size);

        self.draw_lines(rect, ui);
        self.draw_clefs(rect, ui);

        let horizontal_pos = self.line_spacing * 3.0 * self.clef_offset;
        let stacks = [
            Stack::from_pure_interval(T::third_index(), 7),
            Stack::from_pure_interval(T::third_index(), 6),
            Stack::from_pure_interval(T::third_index(), 5),
            Stack::from_pure_interval(T::third_index(), 4),
            Stack::from_pure_interval(T::third_index(), 3),
            Stack::from_pure_interval(T::third_index(), 2),
            Stack::from_pure_interval(T::third_index(), 1),
            Stack::new_zero(),
            //Stack::from_pure_interval(T::fifth_index(), 3),
            Stack::from_pure_interval(T::third_index(), -1),
            ////Stack::from_pure_interval(T::fifth_index(), -1), // this is an interesting example!
            Stack::from_pure_interval(T::third_index(), -2),
            Stack::from_pure_interval(T::third_index(), -3),
            Stack::from_pure_interval(T::third_index(), -4),
            Stack::from_pure_interval(T::third_index(), -5),
            Stack::from_pure_interval(T::third_index(), -6),
            Stack::from_pure_interval(T::third_index(), -7),
        ];

        self.draw_chord(&stacks, horizontal_pos, rect, ui);

        //self.draw_notehead_and_ledger_lines(BaseName::C, 4, horizontal_pos, rect, ui);
        //self.draw_notehead_and_ledger_lines(
        //    BaseName::D,
        //    4,
        //    horizontal_pos + self.line_spacing * STACKED_NOTE_HORIZONTAL_OFFSET,
        //    rect,
        //    ui,
        //);
        //self.draw_notehead_and_ledger_lines(BaseName::E, 4, horizontal_pos, rect, ui);
        //self.draw_notehead_and_ledger_lines(
        //    BaseName::F,
        //    4,
        //    horizontal_pos + self.line_spacing * STACKED_NOTE_HORIZONTAL_OFFSET,
        //    rect,
        //    ui,
        //);
        //self.draw_notehead_and_ledger_lines(BaseName::G, 4, horizontal_pos, rect, ui);
        //self.draw_notehead_and_ledger_lines(BaseName::D, 3, horizontal_pos, rect, ui);
        //self.draw_notehead_and_ledger_lines(BaseName::B, 5, horizontal_pos + 70.0, rect, ui);
        //self.draw_notehead_and_ledger_lines(BaseName::F, 6, horizontal_pos + 80.0, rect, ui);
        //self.draw_notehead_and_ledger_lines(BaseName::C, 2, horizontal_pos + 50.0, rect, ui);
        //self.draw_notehead_and_ledger_lines(BaseName::G, 1, horizontal_pos, rect, ui);
    }
}

struct State<T: StackType> {
    active_notes: [KeyState; 128],
    pedal_hold: [bool; 16],
    tunings: [Stack<T>; 128],
    note_renderer: NoteRenderer<T>,
}

impl<T: FiveLimitStackType> State<T> {
    fn new(ctx: &egui::Context) -> Self {
        let now = Instant::now();
        Self {
            active_notes: core::array::from_fn(|_| KeyState::new(now)),
            pedal_hold: [false; 16],
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            note_renderer: NoteRenderer::new(ctx, 15.0),
        }
    }
}

impl<T: FiveLimitStackType> GUIState<T> for State<T> {
    fn handle_msg(
        &mut self,
        time: Instant,
        msg: &msg::AfterProcess<T>,
        to_process: &mpsc::Sender<(Instant, msg::ToProcess)>,
        ctx: &egui::Context,
        frame: &mut eframe::Frame,
    ) {
        match msg {
            msg::AfterProcess::ForwardMidi { msg } => match msg {
                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOn { note, .. },
                } => {
                    self.active_notes[*note as usize].note_on(*channel, time);
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg: ChannelVoiceMsg::NoteOff { note, .. },
                } => {
                    self.active_notes[*note as usize].note_off(
                        *channel,
                        self.pedal_hold[*channel as usize],
                        time,
                    );
                }

                MidiMsg::ChannelVoice {
                    channel,
                    msg:
                        ChannelVoiceMsg::ControlChange {
                            control: ControlChange::Hold(value),
                        },
                } => {
                    self.pedal_hold[*channel as usize] = *value != 0;
                    if *value == 0 {
                        for note in &mut self.active_notes {
                            note.pedal_off(*channel, time);
                        }
                    }
                }

                _ => {}
            },

            msg::AfterProcess::FromStrategy(msg) => {
                match msg {
                    msg::FromStrategy::Retune {
                        note, tuning_stack, ..
                    } => {
                        self.tunings[*note as usize].clone_from(tuning_stack);
                    }
                    //msg::fromstrategy::consider { stack } => todo!(),
                    _ => {} //msg::FromStrategy::SetReference { key, stack } => todo!(),
                            //msg::FromStrategy::NotifyFit { pattern_name, reference_stack } => todo!(),
                            //msg::FromStrategy::NotifyNoFit => todo!(),
                }
            }
            _ => {} //msg::AfterProcess::Start => todo!(),
                    //msg::AfterProcess::Stop => todo!(),
                    //msg::AfterProcess::Reset => todo!(),
                    //msg::AfterProcess::Notify { line } => todo!(),
                    //msg::AfterProcess::MidiParseErr(_) => todo!(),
                    //msg::AfterProcess::CrosstermEvent(event) => todo!(),
                    //msg::AfterProcess::BackendLatency { since_input } => todo!(),
                    //msg::AfterProcess::DetunedNote { note, should_be, actual, explanation } => todo!(),
        }
        self.update(ctx, frame);
    }
}

impl<T: FiveLimitStackType> eframe::App for State<T> {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        egui::TopBottomPanel::bottom("bottom panel").show(ctx, |ui| {
            egui::widgets::global_theme_preference_switch(ui);
            ui.add(
                egui::widgets::Slider::new(&mut self.note_renderer.x, 0.0..=2.0)
                    .smart_aim(false)
                    .text("x"),
            );
            if ui
                .add(
                    egui::widgets::Slider::new(&mut self.note_renderer.line_spacing, 5.0..=100.0)
                        .smart_aim(false)
                        .logarithmic(true)
                        .show_value(false)
                        .text("zoom"),
                )
                .drag_stopped()
            {
                self.note_renderer.reload_noteshapes(ctx);
            }
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::Frame::canvas(ui.style()).show(ui, |ui| {
                self.note_renderer
                    .draw(&self.active_notes, &self.tunings, ui);
            });
        });
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "adaptuner",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(State::<ConcreteFiveLimitStackType>::new(
                &cc.egui_ctx,
            )))
        }),
    )
}
