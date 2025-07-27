use std::{cmp, marker::PhantomData, sync::mpsc, time::Instant};

use eframe::{
    self,
    egui::{self, vec2},
    epaint::pos2,
};

use crate::{
    gui::r#trait::GuiShow,
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackCoeff, StackType},
    },
    keystate::KeyState,
    msg::{FromUi, HandleMsgRef, ToUi},
    notename::{Accidental, HasNoteNames, NoteName},
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
const TREBLECLEF_OFFSET: egui::Vec2 = vec2(0.0, -1.0896);
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
const PLUS_WIDTH: f32 = 0.8; // equal to the height
const PLUS_LINE_THICKNESS: f32 = 0.25;
const PLUS_VERTICAL_OFFSET: f32 = 0.3;
const MINUS_WIDTH: f32 = 0.8;
const MINUS_LINE_THICKNESS: f32 = 0.25;
const MINUS_VERTICAL_OFFSET: f32 = 0.3;
const LEDGER_LINE_LENGTH: f32 = 2.5;

struct SizedOffsetTexture {
    id: egui::TextureId,

    /// in units of [NoteRenderer::line_spacing]
    size: egui::Vec2,

    /// in units of [NoteRenderer::line_spacing]
    offset: egui::Vec2,
}

struct SVGNoteShapes {
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
    Plus,
    Minus,
}

struct NoteRenderer<T: IntervalBasis> {
    _phantom: PhantomData<T>,

    /// The distance between note lines. (middle of line to middle of line) This is the unit for
    /// all other distances.
    line_spacing: f32,

    /// Thickness of note lines
    line_thickness: f32,

    /// How far the clef's centers are offset from the left margin
    clef_offset: f32,

    svg_noteshapes: SVGNoteShapes,
}

impl SVGNoteShapes {
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
                    offset,
                },
                _ => panic!(
                    "{}",
                    format!("could not load image {:?} at size {}", image, actual_size)
                ),
            }
        };

        SVGNoteShapes {
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

fn vertical_index<T: IntervalBasis + HasNoteNames>(n: &T::NoteName) -> StackCoeff {
    (n.octave() - 4) * 7 + (n.base_name() as StackCoeff)
}

impl<T: IntervalBasis + HasNoteNames> NoteRenderer<T> {
    fn new(ctx: &egui::Context, line_spacing: f32) -> Self {
        Self {
            _phantom: PhantomData,
            line_spacing,
            line_thickness: MEASURED_LINE_THICKNESS / MEASURED_LINE_SPACING,
            clef_offset: 3.0,
            svg_noteshapes: SVGNoteShapes::new(ctx, line_spacing),
        }
    }

    fn reload_svg_noteshapes(&mut self, ctx: &egui::Context) {
        ctx.forget_image(BASSCLEF.uri().unwrap());
        ctx.forget_image(TREBLECLEF.uri().unwrap());
        ctx.forget_image(WHOLENOTE.uri().unwrap());
        ctx.forget_image(SHARP.uri().unwrap());
        ctx.forget_image(FLAT.uri().unwrap());
        ctx.forget_image(DOUBLESHARP.uri().unwrap());
        ctx.forget_image(DOUBLEFLAT.uri().unwrap());

        self.svg_noteshapes = SVGNoteShapes::new(ctx, self.line_spacing);
    }

    fn draw_noteshape(
        &self,
        shape: &NoteShape,
        vertical_index: StackCoeff,
        horizontal_center: f32,
        tint: egui::Color32,
        rect: egui::Rect,
        ui: &mut egui::Ui,
    ) {
        let vertical_center =
            rect.top() + (rect.height() - vertical_index as f32 * self.line_spacing) / 2.0;
        let center = pos2(horizontal_center + rect.left(), vertical_center);

        match shape {
            NoteShape::Plus => {
                let shift = vec2(
                    0.0,
                    if vertical_index % 2 == 0 {
                        PLUS_VERTICAL_OFFSET * self.line_spacing
                    } else {
                        0.0
                    },
                );
                ui.painter().with_clip_rect(rect).rect_filled(
                    egui::Rect::from_center_size(
                        center - shift,
                        self.line_spacing * vec2(PLUS_LINE_THICKNESS, PLUS_WIDTH),
                    ),
                    self.line_spacing * PLUS_LINE_THICKNESS / 3.0,
                    tint,
                );
                ui.painter().with_clip_rect(rect).rect_filled(
                    egui::Rect::from_center_size(
                        center - shift,
                        self.line_spacing * vec2(PLUS_WIDTH, PLUS_LINE_THICKNESS),
                    ),
                    self.line_spacing * PLUS_LINE_THICKNESS / 2.0,
                    tint,
                );
                return;
            }
            NoteShape::Minus => {
                let shift = vec2(
                    0.0,
                    if vertical_index % 2 == 0 {
                        MINUS_VERTICAL_OFFSET * self.line_spacing
                    } else {
                        0.0
                    },
                );
                ui.painter().with_clip_rect(rect).rect_filled(
                    egui::Rect::from_center_size(
                        center - shift,
                        self.line_spacing * vec2(MINUS_WIDTH, MINUS_LINE_THICKNESS),
                    ),
                    self.line_spacing * MINUS_LINE_THICKNESS / 3.0,
                    tint,
                );
                return;
            }
            _ => {}
        };

        let sot = match shape {
            NoteShape::WholeNote => &self.svg_noteshapes.wholenote,
            NoteShape::Sharp => &self.svg_noteshapes.sharp,
            NoteShape::Flat => &self.svg_noteshapes.flat,
            NoteShape::Natural => &self.svg_noteshapes.natural,
            NoteShape::BassClef => &self.svg_noteshapes.bassclef,
            NoteShape::TrebleClef => &self.svg_noteshapes.trebleclef,
            NoteShape::DoubleSharp => &self.svg_noteshapes.doublesharp,
            NoteShape::DoubleFlat => &self.svg_noteshapes.doubleflat,
            _ => unreachable!(),
        };
        ui.painter().with_clip_rect(rect).image(
            sot.id,
            egui::Rect::from_center_size(
                center + self.line_spacing * sot.offset,
                self.line_spacing * sot.size,
            ),
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            tint,
        );
    }

    /// returns results in units of [Self::line_spacing]
    fn noteshape_width(&self, shape: &NoteShape) -> f32 {
        match shape {
            NoteShape::WholeNote => self.svg_noteshapes.wholenote.size.x,
            NoteShape::Sharp => self.svg_noteshapes.sharp.size.x,
            NoteShape::Flat => self.svg_noteshapes.flat.size.x,
            NoteShape::Natural => self.svg_noteshapes.natural.size.x,
            NoteShape::BassClef => self.svg_noteshapes.bassclef.size.x,
            NoteShape::TrebleClef => self.svg_noteshapes.trebleclef.size.x,
            NoteShape::DoubleSharp => self.svg_noteshapes.doublesharp.size.x,
            NoteShape::DoubleFlat => self.svg_noteshapes.doubleflat.size.x,
            NoteShape::Plus => PLUS_WIDTH,
            NoteShape::Minus => MINUS_WIDTH,
        }
    }

    /// Like [Self::draw_noteshape], but aligning the right border of the shape.
    fn draw_noteshape_right_border(
        &self,
        shape: &NoteShape,
        vertical_index: StackCoeff,
        right_border: f32,
        tint: egui::Color32,
        rect: egui::Rect,
        ui: &mut egui::Ui,
    ) {
        let horizontal_center =
            right_border - self.noteshape_width(shape) * self.line_spacing / 2.0;

        self.draw_noteshape(shape, vertical_index, horizontal_center, tint, rect, ui);
    }

    fn draw_lines(&self, rect: egui::Rect, ui: &mut egui::Ui) {
        let mut y = rect.top() + rect.height() / 2.0;

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

        let mut y = rect.top() + rect.height() / 2.0;
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
    }

    fn draw_clefs(&self, rect: egui::Rect, ui: &mut egui::Ui) {
        self.draw_noteshape(
            &NoteShape::TrebleClef,
            4,
            self.line_spacing * self.clef_offset,
            ui.style().visuals.strong_text_color(),
            rect,
            ui,
        );

        self.draw_noteshape(
            &NoteShape::BassClef,
            -4,
            self.line_spacing * self.clef_offset,
            ui.style().visuals.strong_text_color(),
            rect,
            ui,
        );
    }

    /// returns the vertial posisition of the center of the note head
    fn draw_notehead_and_ledger_lines(
        &self,
        vertical_index: StackCoeff,
        horizontal_pos: f32,
        rect: egui::Rect,
        ui: &mut egui::Ui,
    ) {
        let scale = self.line_spacing / MEASURED_LINE_SPACING;
        self.draw_noteshape(
            &NoteShape::WholeNote,
            vertical_index,
            horizontal_pos,
            ui.style().visuals.strong_text_color(),
            rect,
            ui,
        );

        let ledger_line = |index| {
            let thickness = MEASURED_LEDGER_LINE_THICKNESS * scale;
            let length = LEDGER_LINE_LENGTH * self.line_spacing;
            let vpos = rect.top() + rect.height() / 2.0 - self.line_spacing * index as f32 / 2.0;
            ui.painter().with_clip_rect(rect).rect_filled(
                egui::Rect::from_center_size(
                    pos2(horizontal_pos + rect.left(), vpos),
                    vec2(length, thickness),
                ),
                thickness / 2.0,
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

    fn accidental_right_border(
        &self,
        index_offset: StackCoeff,
        new: &<T::NoteName as NoteName>::Accidental,
        neighbour: &<T::NoteName as NoteName>::Accidental,
        neighbour_right_border: f32,
        neighbour_left_border: f32,
    ) -> f32 {
        // if the notes are a at least a seventh apart, no offset is needed
        if index_offset.abs() >= 6 {
            return neighbour_right_border;
        }

        let has_sharps = |a: &<T::NoteName as NoteName>::Accidental| {
            (a.sharpflat() > 0) & (a.sharpflat() % 2 == 1)
        };

        // For a sixth, if the higher note doesn't have sharps (which descend), no noffset is needed
        if ((index_offset == 5) & !has_sharps(neighbour))
            | ((index_offset == -5) & !has_sharps(new))
        {
            return neighbour_right_border;
        }

        // for a fifth or less, we position the accidentals vertically next to each other (For
        // now... Vertical overlap to get a tighter spacing will be for later...)
        return neighbour_left_border;
    }

    /// helper function to apply [Self::accidental_right_border] for [Self::draw_chord].
    fn accidental_vertical_position(
        &self,
        new: &T::NoteName,
        one_ago: &T::NoteName,
        border_one_ago: f32,
        two_ago: &T::NoteName,
        border_two_ago: f32,
        border_three_ago: f32,
    ) -> f32 {
        let x = self.accidental_right_border(
            vertical_index::<T>(one_ago) - vertical_index::<T>(new),
            &new.accidental(),
            &one_ago.accidental(),
            border_two_ago,
            border_one_ago,
        );
        let y = self.accidental_right_border(
            vertical_index::<T>(two_ago) - vertical_index::<T>(new),
            &new.accidental(),
            &two_ago.accidental(),
            border_three_ago,
            border_two_ago,
        );

        x.min(y)
    }

    /// returns the left border of the accidental that was drawn.
    fn draw_accidental(
        &self,
        accidental: &<T::NoteName as NoteName>::Accidental,
        vertical_index: StackCoeff,
        right_border: f32,
        rect: egui::Rect,
        ui: &mut egui::Ui,
    ) -> f32 {
        let tint = ui.style().visuals.strong_text_color();

        let mut hpos = right_border;

        if accidental.plusminus() > 0 {
            for _ in 0..accidental.plusminus() {
                self.draw_noteshape_right_border(
                    &NoteShape::Plus,
                    vertical_index,
                    hpos,
                    tint,
                    rect,
                    ui,
                );
                hpos -= self.line_spacing
                    * (ACCIDENTAL_ACCIDENTAL_SPACE + self.noteshape_width(&NoteShape::Plus));
            }
        };

        if accidental.plusminus() < 0 {
            for _ in 0..(-accidental.plusminus()) {
                self.draw_noteshape_right_border(
                    &NoteShape::Minus,
                    vertical_index,
                    hpos,
                    tint,
                    rect,
                    ui,
                );
                hpos -= self.line_spacing
                    * (ACCIDENTAL_ACCIDENTAL_SPACE + self.noteshape_width(&NoteShape::Minus));
            }
        };

        if accidental.sharpflat() > 0 {
            for _ in 0..(accidental.sharpflat() / 2) {
                self.draw_noteshape_right_border(
                    &NoteShape::DoubleSharp,
                    vertical_index,
                    hpos,
                    tint,
                    rect,
                    ui,
                );
                hpos -= self.line_spacing
                    * (ACCIDENTAL_ACCIDENTAL_SPACE + self.noteshape_width(&NoteShape::DoubleSharp));
            }
            if accidental.sharpflat() % 2 == 1 {
                self.draw_noteshape_right_border(
                    &NoteShape::Sharp,
                    vertical_index,
                    hpos,
                    tint,
                    rect,
                    ui,
                );
                hpos -= self.line_spacing
                    * (ACCIDENTAL_ACCIDENTAL_SPACE + self.noteshape_width(&NoteShape::Sharp));
            }
        };

        if accidental.sharpflat() < 0 {
            for _ in 0..((-accidental.sharpflat()) / 2) {
                self.draw_noteshape_right_border(
                    &NoteShape::DoubleFlat,
                    vertical_index,
                    hpos,
                    tint,
                    rect,
                    ui,
                );
                hpos -= self.line_spacing
                    * (ACCIDENTAL_ACCIDENTAL_SPACE + self.noteshape_width(&NoteShape::DoubleFlat));
            }
            if (-accidental.sharpflat()) % 2 == 1 {
                self.draw_noteshape_right_border(
                    &NoteShape::Flat,
                    vertical_index,
                    hpos,
                    tint,
                    rect,
                    ui,
                );
                hpos -= self.line_spacing
                    * (ACCIDENTAL_ACCIDENTAL_SPACE + self.noteshape_width(&NoteShape::Flat));
            }
        }

        hpos + self.line_spacing * ACCIDENTAL_ACCIDENTAL_SPACE
    }

    /// returns the left border of the leftmost accidental drawn.
    ///
    /// Will change the order, but not the content of the `notenames` argument
    fn draw_chord(
        &self,
        notenames: &mut [T::NoteName],
        horizontal_axis: f32,
        rect: egui::Rect,
        ui: &mut egui::Ui,
    ) -> f32 {
        // sort hightest to lowest
        notenames.sort_by(|a, b| vertical_index::<T>(a).cmp(&vertical_index::<T>(b)));

        // notes without accidentals come first
        notenames.sort_by(|a, b| {
            if a.has_accidental() && !b.has_accidental() {
                cmp::Ordering::Greater
            } else {
                cmp::Ordering::Less
            }
        });

        let first_accidental = match notenames.iter().position(|a| a.has_accidental()) {
            None {} => notenames.len(),
            Some(i) => i,
        };

        // we want to alternate accidentals between high and low notes
        let n_accidentals = notenames.len() - first_accidental;
        let first_half =
            Vec::from(&notenames[first_accidental..(notenames.len() - n_accidentals / 2)]);
        let mut second_half = Vec::from(&notenames[(notenames.len() - n_accidentals / 2)..]);
        second_half.reverse();

        for i in 0..(n_accidentals / 2) {
            notenames[2 * i + first_accidental].clone_from(&first_half[i]);
            notenames[2 * i + 1 + first_accidental].clone_from(&second_half[i]);
        }
        if n_accidentals % 2 == 1 {
            notenames[notenames.len() - 1].clone_from(first_half.last().unwrap());
        }

        for i in 0..first_accidental {
            let n = &notenames[i];
            let ix = vertical_index::<T>(n);
            self.draw_notehead_and_ledger_lines(ix, horizontal_axis, rect, ui);
        }

        // there's nothing special about this note, but the fact that it has no accidental.
        let middlec = T::NoteName::middle_c();

        let mut one_ago = &middlec;
        let mut two_ago = &middlec;
        let mut border_one_ago = horizontal_axis
            - self.line_spacing
                * (0.5 * self.svg_noteshapes.wholenote.size.x + NOTE_ACCIDENTAL_SPACE);
        let mut border_two_ago = border_one_ago;
        let mut border_three_ago = border_one_ago;
        let mut furthest_left = border_one_ago;

        for i in first_accidental..notenames.len() {
            let n = &notenames[i];
            let ix = vertical_index::<T>(n);
            self.draw_notehead_and_ledger_lines(ix, horizontal_axis, rect, ui);
            let right_border = self.accidental_vertical_position(
                n,
                one_ago,
                border_one_ago,
                two_ago,
                border_two_ago,
                border_three_ago,
            );
            let left_border = self.draw_accidental(&n.accidental(), ix, right_border, rect, ui);
            border_three_ago = border_two_ago;
            two_ago = one_ago;
            border_two_ago = border_one_ago;
            one_ago = n;
            border_one_ago = left_border;
            furthest_left = furthest_left.min(left_border);
        }

        furthest_left
    }

    fn draw(&self, active_notes: &[KeyState; 128], tunings: &[Stack<T>], ui: &mut egui::Ui) {
        //let desired_size = ui.available_size();
        //let (_id, rect) = ui.allocate_space(desired_size);
        let rect = ui.clip_rect();

        self.draw_lines(rect, ui);
        self.draw_clefs(rect, ui);

        let horizontal_pos = self.line_spacing * 6.0 * self.clef_offset;

        let mut notes = vec![];

        for i in 0..128 {
            if active_notes[i].is_sounding() {
                notes.push(T::notename(&tunings[i]));
            }
        }

        let x = self.draw_chord(&mut notes, horizontal_pos, rect, ui);
    }
}

pub struct NoteWindow<T: IntervalBasis> {
    active_notes: [KeyState; 128],
    pedal_hold: [bool; 16],
    tunings: [Stack<T>; 128],
    note_renderer: NoteRenderer<T>,
}

impl<T: IntervalBasis + HasNoteNames> NoteWindow<T> {
    pub fn new(ctx: &egui::Context) -> Self {
        let now = Instant::now();
        ctx.set_theme(egui::ThemePreference::System);
        Self {
            active_notes: core::array::from_fn(|_| KeyState::new(now)),
            pedal_hold: [false; 16],
            tunings: core::array::from_fn(|_| Stack::new_zero()),
            note_renderer: NoteRenderer::new(ctx, 15.0),
        }
    }
}

impl<T: StackType> HandleMsgRef<ToUi<T>, FromUi<T>> for NoteWindow<T> {
    fn handle_msg_ref(&mut self, msg: &ToUi<T>, _forward: &mpsc::Sender<FromUi<T>>) {
        match msg {
            ToUi::NoteOn {
                time,
                channel,
                note,
            } => {
                self.active_notes[*note as usize].note_on(*channel, *time);
            }

            ToUi::NoteOff {
                time,
                channel,
                note,
            } => {
                self.active_notes[*note as usize].note_off(
                    *channel,
                    self.pedal_hold[*channel as usize],
                    *time,
                );
            }

            ToUi::PedalHold {
                channel,
                value,
                time,
            } => {
                self.pedal_hold[*channel as usize] = *value != 0;
                if *value == 0 {
                    for n in self.active_notes.iter_mut() {
                        n.pedal_off(*channel, *time);
                    }
                }
            }

            ToUi::Retune { note, tuning_stack } => {
                self.tunings[*note as usize].clone_from(tuning_stack);
            }

            ToUi::TunedNoteOn {
                time,
                channel,
                note,
                tuning_stack,
            } => {
                self.active_notes[*note as usize].note_on(*channel, *time);
                self.tunings[*note as usize].clone_from(tuning_stack);
            }

            _ => {}
        }
    }
}

impl<T: StackType + HasNoteNames> GuiShow<T> for NoteWindow<T> {
    fn show(&mut self, ui: &mut egui::Ui, _forward: &mpsc::Sender<FromUi<T>>) {
        egui::TopBottomPanel::bottom("note window bottom panel").show_inside(ui, |ui| {
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
                self.note_renderer.reload_svg_noteshapes(ui.ctx());
            }
        });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.note_renderer
                .draw(&self.active_notes, &self.tunings, ui);
        });
    }
}
