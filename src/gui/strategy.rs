use std::time::Instant;

use eframe::egui;

use crate::{
    bindable::BindableEvent,
    config::{HarmonyStrategyConfig, MelodyStrategyConfig, StrategyConfig},
    gui::{
        common::{
            show_list_edit, show_list_picker, ListEditOpts, ListEditResult, SmallFloatingWindow,
        },
        editor::{
            binding::BindingEditor,
            chordlist::{ChordListEditor, ChordListEditorResult},
            scale::{ScaleEditor, ScaleEditorResult},
        },
        r#trait::{GuiShow, ReceiveToUiRef, UiAdaptor},
    },
    interval::stacktype::r#trait::{OctavePeriodicStackType, StackType},
    msg::{
        FromUi, ToChordList, ToHarmony, ToMelody, ToStaticNeighbourhoods,
        ToStaticNeighbourhoodsAsMelody, ToStrategy, ToTwoStep, ToUi,
    },
    notename::HasNoteNames,
    strategy::{
        harmony::chordlist::ChordListConfig,
        melody::neighbourhoods::StaticNeighbourhoodsAsMelodyConfig,
        staticneighbourhoods::StaticNeighbourhoodsConfig,
    },
    util::list_action::ListAction,
};

struct StrategySelectorWidget {
    strategy_list_editor_window: SmallFloatingWindow,
}

impl StrategySelectorWidget {
    fn new() -> Self {
        Self {
            strategy_list_editor_window: SmallFloatingWindow::new(
                egui::Id::new("strategy_list_editor_window"),
                false,
            ),
        }
    }

    fn show_windows<T: StackType>(
        &mut self,
        ui: &mut egui::Ui,
        adaptor: &impl UiAdaptor<T>,
        disable: bool,
    ) {
        self.strategy_list_editor_window
            .show("edit strategies", ui.ctx(), |ui| {
                ui.vertical(|ui| {
                    if disable {
                        ui.disable();
                    }
                    // Don't handle the ListAction wrapped by `res` here, the process has to do
                    // that. It's a bit funny that we're working with a mut reference
                    // `strategy_config_mut`, but everything is all right, since the only thing
                    // we'll change in this thread are names and descriptions of strategies, and
                    // these aren't important in the process thread.
                    let res = show_list_edit(
                        ui,
                        "strategy_editor",
                        &mut *adaptor.strategy_config_mut(),
                        Some(adaptor.active_strategy_index()),
                        ListEditOpts {
                            empty_allowed: false,
                            select_allowed: true,
                            no_selection_allowed: false,
                            delete_allowed: true,
                            reorder_allowed: true,
                            show_one: Box::new(|ui, _i, elem: &mut StrategyConfig<T>, _| {
                                ui.add(egui::TextEdit::singleline(elem.name_mut()).min_size(
                                    egui::vec2(
                                        ui.style().spacing.text_edit_width / 2.0,
                                        ui.style().spacing.interact_size.y,
                                    ),
                                ));
                                ui.add(
                                    egui::TextEdit::multiline(elem.description_mut())
                                        .min_size(egui::vec2(
                                            ui.style().spacing.text_edit_width,
                                            ui.style().spacing.interact_size.y,
                                        ))
                                        .desired_rows(1),
                                );
                                None::<()>
                            }),
                            clone: Some(Box::new(|ui, _elems, selected, _| {
                                ui.separator();
                                if let Some(i) = selected {
                                    if ui.button("create copy of selected").clicked() {
                                        Some(i)
                                    } else {
                                        None {}
                                    }
                                } else {
                                    None {}
                                }
                            })),
                        },
                        &mut (),
                    );
                    match res {
                        ListEditResult::None => {}
                        ListEditResult::Action(action) => {
                            let _ = adaptor.send(FromUi::StrategyListAction {
                                action,
                                time: Instant::now(),
                            });
                        }
                        ListEditResult::Message(_) => unreachable!(),
                    }
                });
            });
    }
}

impl<T: StackType> GuiShow<T> for StrategySelectorWidget {
    fn show(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        let asi = adaptor.active_strategy_index();
        egui::ComboBox::from_id_salt("strategy selector widget")
            .selected_text(adaptor.strategy_config()[asi].name())
            .show_ui(ui, |ui| {
                if let Some(i) = show_list_picker(
                    &*adaptor.strategy_config(),
                    asi,
                    ui,
                    |x| x.name(),
                    |x| x.description(),
                ) {
                    let _ = adaptor.send(FromUi::StrategyListAction {
                        action: ListAction::Select(i),
                        time: Instant::now(),
                    });
                }

                ui.separator();

                self.strategy_list_editor_window
                    .show_hide_button(ui, "edit strategies");

                ui.shrink_width_to_current();
            });
    }
}

struct BindingEditorWidget {
    binding_editor: BindingEditor,
}

impl BindingEditorWidget {
    fn new() -> Self {
        Self {
            binding_editor: BindingEditor::new(),
        }
    }

    fn react_to_bound_keys<T: StackType>(
        &mut self,
        ui: &mut egui::Ui,
        adaptor: &impl UiAdaptor<T>,
        disable: bool,
    ) {
        if disable {
            return;
        }
        if ui.ui_contains_pointer() {
            ui.input(|i| {
                for e in &i.events {
                    match e {
                        egui::Event::Key {
                            key,
                            pressed,
                            repeat,
                            ..
                        } => {
                            if !*pressed || *repeat {
                                return;
                            }
                            if let Some(action) = adaptor.strategy_config()
                                [adaptor.active_strategy_index()]
                            .bindings()
                            .get(&BindableEvent::KeyPress(*key))
                            .map(|x| *x)
                            {
                                let _ = adaptor.send(FromUi::ToStrategy(ToStrategy::BoundAction {
                                    action,
                                    time: Instant::now(),
                                }));
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    }
}

impl<T: StackType> GuiShow<T> for BindingEditorWidget {
    #[inline]
    fn show(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        self.binding_editor.show(ui, adaptor)
    }
}

pub struct StrategyWidgets<T: StackType> {
    selector_widget: StrategySelectorWidget,
    binding_editor_widget: BindingEditorWidget,
    scale_editor: ScaleEditor,
    chord_list_editor: ChordListEditor<T>,
}

impl<T: OctavePeriodicStackType + HasNoteNames> StrategyWidgets<T> {
    pub fn new() -> Self {
        Self {
            selector_widget: StrategySelectorWidget::new(),
            binding_editor_widget: BindingEditorWidget::new(),
            scale_editor: ScaleEditor::new(),
            chord_list_editor: ChordListEditor::new(),
        }
    }

    pub fn show_windows(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>, disable: bool) {
        self.selector_widget.show_windows(ui, adaptor, disable);
        self.binding_editor_widget
            .react_to_bound_keys(ui, adaptor, disable);
    }

    #[inline]
    fn show_scale_editor(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        match &mut adaptor.strategy_config_mut()[adaptor.active_strategy_index()] {
            StrategyConfig::StaticNeighbourhoods {
                config: StaticNeighbourhoodsConfig { scales, .. },
                ..
            } => {
                ui.collapsing("scales", |ui| match self.scale_editor.show(ui, scales) {
                    ScaleEditorResult::NoChange => {}
                    ScaleEditorResult::Select(i) => {
                        let _ = adaptor.send(FromUi::ToStrategy(ToStrategy::StaticNeighbourhoods(
                            ToStaticNeighbourhoods::SelectScale {
                                index: i,
                                time: Instant::now(),
                            },
                        )));
                    }
                    ScaleEditorResult::ChangeScale(i) => {
                        let _ = adaptor.send(FromUi::ToStrategy(ToStrategy::StaticNeighbourhoods(
                            ToStaticNeighbourhoods::UpdateScales {
                                only_this_scale: Some(i),
                                time: Instant::now(),
                            },
                        )));
                    }
                    ScaleEditorResult::ChangeList => {
                        let _ = adaptor.send(FromUi::ToStrategy(ToStrategy::StaticNeighbourhoods(
                            ToStaticNeighbourhoods::UpdateScales {
                                only_this_scale: None {},
                                time: Instant::now(),
                            },
                        )));
                    }
                });
            }
            StrategyConfig::TwoStep {
                melody:
                    MelodyStrategyConfig::StaticNeighbourhoods(StaticNeighbourhoodsAsMelodyConfig {
                        scales,
                        ..
                    }),
                ..
            } => {
                ui.collapsing("scales", |ui| match self.scale_editor.show(ui, scales) {
                    ScaleEditorResult::NoChange => {}
                    ScaleEditorResult::Select(i) => {
                        let _ = adaptor.send(FromUi::ToStrategy(ToStrategy::TwoStep(
                            ToTwoStep::ToMelodyStrategy(ToMelody::StaticNeighbourhoods(
                                ToStaticNeighbourhoodsAsMelody::SelectScale {
                                    index: i,
                                    time: Instant::now(),
                                },
                            )),
                        )));
                    }
                    ScaleEditorResult::ChangeScale(i) => {
                        let _ = adaptor.send(FromUi::ToStrategy(ToStrategy::TwoStep(
                            ToTwoStep::ToMelodyStrategy(ToMelody::StaticNeighbourhoods(
                                ToStaticNeighbourhoodsAsMelody::UpdateScales {
                                    only_this_scale: Some(i),
                                    time: Instant::now(),
                                },
                            )),
                        )));
                    }
                    ScaleEditorResult::ChangeList => {
                        let _ = adaptor.send(FromUi::ToStrategy(ToStrategy::TwoStep(
                            ToTwoStep::ToMelodyStrategy(ToMelody::StaticNeighbourhoods(
                                ToStaticNeighbourhoodsAsMelody::UpdateScales {
                                    only_this_scale: None {},
                                    time: Instant::now(),
                                },
                            )),
                        )));
                    }
                });
            }
        }
    }

    #[inline]
    fn show_chord_list_editor(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        let wrap = |msg| {
            FromUi::ToStrategy(ToStrategy::TwoStep(ToTwoStep::ToHarmonyStrategy(
                ToHarmony::ChordList(msg),
            )))
        };
        match &mut adaptor.strategy_config_mut()[adaptor.active_strategy_index()] {
            StrategyConfig::TwoStep {
                harmony: HarmonyStrategyConfig::ChordList(ChordListConfig { enable, patterns }),
                ..
            } => match self.chord_list_editor.show(
                ui,
                enable,
                patterns,
                adaptor,
                adaptor.config().use_cent_values,
            ) {
                ChordListEditorResult::None => {}
                ChordListEditorResult::ToggleEnable => {
                    let _ = adaptor.send(wrap(ToChordList::ToggleEnable {
                        time: Instant::now(),
                    }));
                }
                ChordListEditorResult::UpdateChord(i) => {
                    let _ = adaptor.send(wrap(ToChordList::UpdateChord {
                        index: i,
                        time: Instant::now(),
                    }));
                }
                ChordListEditorResult::ListAction(list_action) => {
                    let _ = adaptor.send(wrap(ToChordList::ChordListAction {
                        list_action,
                        time: Instant::now(),
                    }));
                }
                ChordListEditorResult::PushNewChord => {
                    let _ = adaptor.send(wrap(ToChordList::PushNewChord {
                        time: Instant::now(),
                    }));
                }
            },

            _ => {}
        }
    }
}

impl<T: OctavePeriodicStackType + HasNoteNames> GuiShow<T> for StrategyWidgets<T> {
    fn show(&mut self, ui: &mut egui::Ui, adaptor: &impl UiAdaptor<T>) {
        self.selector_widget.show(ui, adaptor);
        self.binding_editor_widget.show(ui, adaptor);
        self.show_scale_editor(ui, adaptor);
        self.show_chord_list_editor(ui, adaptor);
    }
}

impl<T: StackType, A: UiAdaptor<T>> ReceiveToUiRef<T, A> for StrategyWidgets<T> {
    fn receive_to_ui_ref(&mut self, msg: &ToUi<T>, adaptor: &A) {
        self.scale_editor.receive_to_ui_ref(msg, adaptor);
        self.chord_list_editor.receive_to_ui_ref(msg, adaptor);
    }
}

// use std::{cell::RefCell, rc::Rc, sync::mpsc, time::Instant};
//
// use eframe::egui::{self, vec2};
//
// use crate::{
//     interval::stacktype::r#trait::{OctavePeriodicStackType, StackType},
//     msg::{FromUi, ReceiveMsgRef, ToUi},
//     notename::HasNoteNames,
//     util::list_action::ListAction,
// };
//
// use super::{
//     common::{
//         CorrectionSystemChooser, ListEdit, ListEditOpts, OwningListEdit, SmallFloatingWindow,
//     },
//     editor::{
//         // binding::BindingEditor,
//         // chordlist::ChordListEditor,
//         // neighbourhood::NeighbourhoodEditor,
//         // reference::{ReferenceEditor, ReferenceEditorConfig},
//         tuning::{TuningEditor, TuningEditorConfig},
//         // twostep::TwoStepEditor,
//     },
//     r#trait::GuiShow,
//     toplevel::KeysAndTunings,
// };
//
// pub struct StrategyWindows<T: StackType + 'static> {
//     strategy_list_editor_window: SmallFloatingWindow,
//     // strategies: OwningListEdit<(StrategyNames<T>, Bindings<Bindable>)>,
//
//     tuning_editor: TuningEditor<T>,
//     // reference_editor: ReferenceEditor<T>,
//     // neighbourhood_editor: NeighbourhoodEditor<T>,
//     // binding_editor: BindingEditor,
//     // chord_list_editor: ChordListEditor<T>,
//     // twostep_editor: TwoStepEditor,
// }
//
// /// [OctavePeriodicStackType] is needed for the [ChordListEditor]
// impl<T: OctavePeriodicStackType + HasNoteNames> StrategyWindows<T> {
//     pub fn strategies(&self) -> &[(StrategyNames<T>, Bindings<Bindable>)] {
//         self.strategies.elems()
//     }
//
//     pub fn currently_active(&self) -> Option<&(StrategyNames<T>, Bindings<Bindable>)> {
//         self.strategies.current_selected()
//     }
//
//     pub fn new(
//         strategies: Vec<(StrategyNames<T>, Bindings<Bindable>)>,
//         tuning_editor: TuningEditorConfig,
//         reference_editor: ReferenceEditorConfig,
//         correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
//     ) -> Self {
//         Self {
//             strategies: OwningListEdit::new(strategies),
//             strategy_list_editor_window: SmallFloatingWindow::new(
//                 egui::Id::new("strategy_list_editor_window"),
//                 false,
//             ),
//             tuning_editor: TuningEditor::new(tuning_editor, correction_system_chooser.clone()),
//             reference_editor: ReferenceEditor::new(
//                 reference_editor,
//                 correction_system_chooser.clone(),
//             ),
//             neighbourhood_editor: NeighbourhoodEditor::new(),
//             binding_editor: BindingEditor::new(),
//             chord_list_editor: ChordListEditor::new(correction_system_chooser),
//             twostep_editor: TwoStepEditor::new(),
//         }
//     }
//
//     pub fn restart_from_config(
//         &mut self,
//         strategies: Vec<(StrategyNames<T>, Bindings<Bindable>)>,
//         tuning_editor: TuningEditorConfig,
//         reference_editor: ReferenceEditorConfig,
//         correction_system_chooser: Rc<RefCell<CorrectionSystemChooser<T>>>,
//         _time: Instant,
//     ) {
//         *self = Self::new(
//             strategies,
//             tuning_editor,
//             reference_editor,
//             correction_system_chooser,
//         );
//     }
// }
//
// impl<T: StackType> ReceiveMsgRef<ToUi<T>> for StrategyWindows<T> {
//     fn receive_msg_ref(&mut self, msg: &ToUi<T>) {
//         match msg {
//             ToUi::CurrentStrategyIndex(index) => {
//                 if let Some(i) = index {
//                     self.strategies.apply(ListAction::Select(*i));
//                 } else {
//                     self.strategies.apply(ListAction::Deselect);
//                 }
//             }
//             ToUi::ReanchorOnMatch { reanchor } => {
//                 if let Some((
//                     StrategyNames::TwoStep {
//                         melody: MelodyStrategyNames::Neighbourhoods { fixed, .. },
//                         ..
//                     },
//                     _,
//                 )) = self.strategies.current_selected_mut()
//                 {
//                     *fixed = !reanchor;
//                 }
//             }
//             _ => {}
//         }
//         self.reference_editor.receive_msg_ref(msg);
//         self.tuning_editor.receive_msg_ref(msg);
//         self.neighbourhood_editor.receive_msg_ref(msg);
//         self.chord_list_editor.receive_msg_ref(msg);
//
//         // twostep_editor doesn't need to handle any messages, we handle ReanchorOnMatch here:
//         // self.twostep_editor.handle_msg_ref(msg, forward);
//     }
// }
//
// pub struct AsStrategyPicker<'a, T: StackType + 'static>(pub &'a mut StrategyWindows<T>);
//
// /// [OctavePeriodicStackType] is needed for the [ChordListEditor]
// impl<'a, T: OctavePeriodicStackType + HasNoteNames> AsStrategyPicker<'a, T> {
//     pub fn show(
//         &mut self,
//         ui: &mut egui::Ui,
//         state: &KeysAndTunings<T>,
//         forward: &mpsc::Sender<FromUi<T>>,
//     ) {
//         let AsStrategyPicker(x) = self;
//         egui::ComboBox::from_id_salt("strategy picker")
//             .selected_text(x.strategies.current_selected().map_or("", |x| x.0.name()))
//             .show_ui(ui, |ui| {
//                 if let Some((i, _)) = x.strategies.show_as_list_picker(
//                     ui,
//                     |x| x.0.name(),
//                     |x| Some(x.0.description()),
//                 ) {
//                     let _ = forward.send(FromUi::StrategyListAction {
//                         action: ListAction::Select(i),
//                         time: Instant::now(),
//                     });
//                 }
//
//                 ui.separator();
//
//                 x.strategy_list_editor_window
//                     .show_hide_button(ui, "edit strategies");
//
//                 ui.shrink_width_to_current();
//             });
//
//         ui.collapsing("global tuning", |ui| x.tuning_editor.show(ui, forward));
//         ui.collapsing("reference", |ui| x.reference_editor.show(ui, forward));
//         if let Some(strn) = x.strategies.current_selected_mut() {
//             ui.collapsing("bindings", |ui| {
//                 x.binding_editor
//                     .show(ui, strn.0.strategy_kind(), &mut strn.1, forward)
//             });
//
//             match &mut strn.0 {
//                 StrategyNames::StaticTuning {
//                     neighbourhood_names,
//                     ..
//                 } => {
//                     ui.collapsing("neighbourhoods", |ui| {
//                         x.neighbourhood_editor
//                             .show(ui, neighbourhood_names, forward)
//                     });
//                 }
//                 StrategyNames::TwoStep {
//                     harmony, melody, ..
//                 } => {
//                     match melody {
//                         MelodyStrategyNames::Neighbourhoods {
//                             neighbourhood_names,
//                             ..
//                         } => {
//                             ui.collapsing("neighbourhoods", |ui| {
//                                 x.neighbourhood_editor
//                                     .show(ui, neighbourhood_names, forward)
//                             });
//                         }
//                     }
//                     match harmony {
//                         HarmonyStrategyNames::ChordList { patterns } => {
//                             ui.collapsing("chord list", |ui| {
//                                 x.chord_list_editor.show(ui, state, patterns, forward);
//                             });
//                         }
//                     }
//                     ui.collapsing("melody/harmony", |ui| {
//                         x.twostep_editor.show(ui, harmony, melody, forward)
//                     });
//                 }
//             }
//         }
//     }
// }
//
// pub struct AsWindows<'a, T: StackType>(pub &'a mut StrategyWindows<T>);
