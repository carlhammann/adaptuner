use std::time::Instant;

use eframe::egui;

use crate::{
    adaptors::lock_levels::StrategyConfigLevel,
    bindable::BindableEvent,
    config::{HarmonyStrategyConfig, MelodyStrategyConfig, StrategyConfig},
    gui::{
        common::{
            show_list_edit, show_list_picker, ListEditOpts, ListEditResult, SmallFloatingWindow,
        },
        editor::{
            binding::BindingEditor,
            chordlist::{ChordListEditor, ChordListEditorResult},
            reference::ReferenceEditor,
            scale::{ScaleEditor, ScaleEditorResult},
            twostep::TwoStepEditor,
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
    util::ordered_locks::{AtMost, Zero},
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

    fn show_windows<T, L>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: UiAdaptor<T, L>,
        disable: bool,
    ) -> UiAdaptor<T, L>
    where
        T: StackType,
        L: AtMost<StrategyConfigLevel>,
    {
        (_, adaptor) = adaptor.strategy_config_mut(|strategy_configs, adaptor| {
            adaptor.active_strategy_index_mut(|active_strategy_index, adaptor| {
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
                                strategy_configs,
                                Some(*active_strategy_index),
                                ListEditOpts {
                                    empty_allowed: false,
                                    select_allowed: true,
                                    no_selection_allowed: false,
                                    delete_allowed: true,
                                    reorder_allowed: true,
                                    show_one: Box::new(
                                        |ui, _i, elem: &mut StrategyConfig<T>, _| {
                                            ui.add(
                                                egui::TextEdit::singleline(elem.name_mut())
                                                    .min_size(egui::vec2(
                                                        ui.style().spacing.text_edit_width / 2.0,
                                                        ui.style().spacing.interact_size.y,
                                                    )),
                                            );
                                            ui.add(
                                                egui::TextEdit::multiline(elem.description_mut())
                                                    .min_size(egui::vec2(
                                                        ui.style().spacing.text_edit_width,
                                                        ui.style().spacing.interact_size.y,
                                                    ))
                                                    .desired_rows(1),
                                            );
                                            None::<()>
                                        },
                                    ),
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
                                    action.apply_to(strategy_configs, active_strategy_index, |x| {
                                        x.clone()
                                    });
                                    let _ = adaptor.send(FromUi::RestartFromConfig {
                                        time: Instant::now(),
                                    });
                                }
                                ListEditResult::Message(_) => unreachable!(),
                            }
                        });
                    });
            });
        });

        adaptor
    }
}

impl<T: StackType> GuiShow<T> for StrategySelectorWidget {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        (_, adaptor) = adaptor.strategy_config(|strategy_configs, adaptor| {
            adaptor.active_strategy_index_mut(|active_strategy_index, adaptor| {
                egui::ComboBox::from_id_salt("strategy selector widget")
                    .selected_text(strategy_configs[*active_strategy_index].name())
                    .show_ui(ui, |ui| {
                        if let Some(i) = show_list_picker(
                            &strategy_configs,
                            *active_strategy_index,
                            ui,
                            |x| x.name(),
                            |x| x.description(),
                        ) {
                            *active_strategy_index = i;
                            let _ = adaptor.send(FromUi::RestartFromConfig {
                                time: Instant::now(),
                            });
                        }

                        ui.separator();

                        self.strategy_list_editor_window
                            .show_hide_button(ui, "edit strategies");

                        ui.shrink_width_to_current();
                    });
            });
        });
        adaptor
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

    fn react_to_bound_keys<T, L>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: UiAdaptor<T, L>,
        disable: bool,
    ) -> UiAdaptor<T, L>
    where
        T: StackType,
        L: AtMost<StrategyConfigLevel>,
    {
        if disable {
            return adaptor;
        }
        if ui.ui_contains_pointer() {
            adaptor = ui.input(|i| {
                for e in &i.events {
                    match e {
                        egui::Event::Key {
                            key,
                            pressed,
                            repeat,
                            ..
                        } => {
                            if !*pressed || *repeat {
                                return adaptor;
                            }
                            (_, adaptor) = adaptor.active_strategy(|strat, adaptor| {
                                if let Some(action) = strat
                                    .bindings()
                                    .get(&BindableEvent::KeyPress(*key))
                                    .map(|x| *x)
                                {
                                    let _ = adaptor.send(FromUi::BoundAction {
                                        action,
                                        time: Instant::now(),
                                    });
                                }
                            });
                        }
                        _ => {}
                    }
                }
                adaptor
            });
        }

        adaptor
    }
}

impl<T: StackType> GuiShow<T> for BindingEditorWidget {
    #[inline]
    fn show(&mut self, ui: &mut egui::Ui, adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        self.binding_editor.show(ui, adaptor)
    }
}

pub struct StrategyWidgets<T: StackType> {
    selector_widget: StrategySelectorWidget,
    binding_editor_widget: BindingEditorWidget,
    initial_reference_editor: ReferenceEditor<T>,
    scale_editor: ScaleEditor,
    chord_list_editor: ChordListEditor<T>,
    twostep_editor: TwoStepEditor,
}

impl<T: OctavePeriodicStackType + HasNoteNames> StrategyWidgets<T> {
    pub fn new() -> Self {
        Self {
            selector_widget: StrategySelectorWidget::new(),
            initial_reference_editor: ReferenceEditor::new(),
            binding_editor_widget: BindingEditorWidget::new(),
            scale_editor: ScaleEditor::new(),
            chord_list_editor: ChordListEditor::new(),
            twostep_editor: TwoStepEditor::new(),
        }
    }

    pub fn show_windows<L>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: UiAdaptor<T, L>,
        disable: bool,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<StrategyConfigLevel>,
    {
        adaptor = self.selector_widget.show_windows(ui, adaptor, disable);
        self.binding_editor_widget
            .react_to_bound_keys(ui, adaptor, disable)
    }

    #[inline]
    fn show_scale_editor<L>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: UiAdaptor<T, L>,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<StrategyConfigLevel>,
    {
        (_, adaptor) = adaptor.active_strategy_mut(|strat, adaptor| match strat {
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
        });

        adaptor
    }

    #[inline]
    fn show_chord_list_editor<L>(
        &mut self,
        ui: &mut egui::Ui,
        mut adaptor: UiAdaptor<T, L>,
    ) -> UiAdaptor<T, L>
    where
        L: AtMost<StrategyConfigLevel>,
    {
        let wrap = |msg| {
            FromUi::ToStrategy(ToStrategy::TwoStep(ToTwoStep::ToHarmonyStrategy(
                ToHarmony::ChordList(msg),
            )))
        };
        (_, adaptor) = adaptor.active_strategy_mut(|strat, mut adaptor| match strat {
            StrategyConfig::TwoStep {
                harmony:
                    HarmonyStrategyConfig::ChordList(ChordListConfig {
                        ref mut enable,
                        patterns,
                    }),
                ..
            } => {
                let use_cent_values = adaptor.config().use_cent_values;
                let res;
                (res, adaptor) =
                    self.chord_list_editor
                        .show(ui, enable, patterns, adaptor, use_cent_values);
                match res {
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
                }
            }

            _ => {}
        });

        adaptor
    }
}

impl<T: OctavePeriodicStackType + HasNoteNames> GuiShow<T> for StrategyWidgets<T> {
    fn show(&mut self, ui: &mut egui::Ui, mut adaptor: UiAdaptor<T, Zero>) -> UiAdaptor<T, Zero> {
        adaptor = self.selector_widget.show(ui, adaptor);
        adaptor = self.initial_reference_editor.show(ui, adaptor);
        adaptor = self.binding_editor_widget.show(ui, adaptor);
        adaptor = self.show_scale_editor(ui, adaptor);
        adaptor = self.show_chord_list_editor(ui, adaptor);
        self.twostep_editor.show(ui, adaptor)
    }
}

impl<T: StackType> ReceiveToUiRef<T> for StrategyWidgets<T> {
    fn receive_to_ui_ref(
        &mut self,
        msg: &ToUi<T>,
        mut adaptor: UiAdaptor<T, Zero>,
    ) -> UiAdaptor<T, Zero> {
        adaptor = self.scale_editor.receive_to_ui_ref(msg, adaptor);
        adaptor = self.initial_reference_editor.receive_to_ui_ref(msg, adaptor);
        self.chord_list_editor.receive_to_ui_ref(msg, adaptor)
    }
}
