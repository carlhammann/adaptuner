use std::{collections::BTreeMap, marker::PhantomData, rc::Rc};

use serde_derive::{Deserialize, Serialize};

use crate::{
    config::{ExtractConfig, HarmonyStrategyConfig},
    interval::{
        stack::Stack,
        stacktype::r#trait::{IntervalBasis, StackType},
    },
    keystate::KeyState,
    neighbourhood::{Neighbourhood, PeriodicPartial, SomeNeighbourhood},
    strategy::twostep::{Harmony, HarmonyStrategy},
};

mod keyshape;
use keyshape::{Fit, HasActivationStatus, KeyShape};

#[derive(Debug, Clone, PartialEq)]
struct Pattern<T: StackType> {
    name: String,
    key_shape: KeyShape,
    neighbourhood: Rc<SomeNeighbourhood<T>>,
}

impl<T: StackType> Pattern<T> {
    fn new(conf: PatternConfig<T>) -> Self {
        Self {
            name: conf.name,
            key_shape: conf.key_shape,
            neighbourhood: match conf.neighbourhood_kind {
                NeighbourhoodKind::PeriodicPartial => {
                    let mut n = PeriodicPartial::new();
                    conf.neighbourhood.values().for_each(|stack| {
                        n.insert(stack);
                    });
                    Rc::new(SomeNeighbourhood::PeriodicPartial(n))
                }
            },
        }
    }

    fn fit(&self, notes: &[KeyState; 128]) -> Fit {
        self.key_shape.fit(notes, 0)
    }
}

impl HasActivationStatus for KeyState {
    fn active(&self) -> bool {
        self.is_sounding()
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub enum NeighbourhoodKind {
    // PeriodicComplete,
    PeriodicPartial,
    // Partial,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct PatternConfig<T: IntervalBasis> {
    #[serde(skip)]
    _phantom: PhantomData<T>,
    name: String,
    key_shape: KeyShape,
    neighbourhood_kind: NeighbourhoodKind,
    neighbourhood: BTreeMap<i8, Stack<T>>,
}

impl<T: StackType> ExtractConfig<PatternConfig<T>> for Pattern<T> {
    fn extract_config(&self) -> PatternConfig<T> {
        let Pattern {
            name,
            key_shape,
            neighbourhood,
        } = self;
        PatternConfig {
            _phantom: PhantomData,
            name: name.clone(),
            key_shape: key_shape.clone(),
            neighbourhood_kind: match **neighbourhood {
                SomeNeighbourhood::PeriodicPartial(_) => NeighbourhoodKind::PeriodicPartial,
                _ => todo!(),
            },
            neighbourhood: {
                let mut map = BTreeMap::new();
                neighbourhood.for_each_stack(|i, stack| {
                    map.insert(i, stack.clone());
                });
                map
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "kebab-case")]
pub struct ChordListConfig<T: IntervalBasis> {
    patterns: Vec<PatternConfig<T>>,
}

pub struct ChordList<T: StackType> {
    patterns: Vec<Pattern<T>>,
}

impl<T: StackType> ChordList<T> {
    pub fn new(mut conf: ChordListConfig<T>) -> Self {
        Self {
            patterns: conf.patterns.drain(..).map(|c| Pattern::new(c)).collect(),
        }
    }
}

impl<T: StackType> HarmonyStrategy<T> for ChordList<T> {
    fn solve(&mut self, keys: &[KeyState; 128]) -> (Option<String>, Option<Harmony<T>>) {
        let mut fit = Fit::new_worst();
        let mut index = 0;
        for (i, p) in self.patterns.iter().enumerate() {
            if fit.is_complete() {
                break;
            }
            let new_fit = p.fit(keys);
            if new_fit.is_better_than(&fit) {
                fit = new_fit;
                index = i;
            }
        }

        // only abort if we weren't able to match anything, otherwise wer're happy with matching
        // only the lowest few sounding notes
        if fit.matches_nothing() {
            return (None {}, None {});
        }

        (
            Some(self.patterns[index].name.clone()),
            Some(Harmony {
                neighbourhood: self.patterns[index].neighbourhood.clone(),
                reference: fit.reference,
            }),
        )
    }
}

impl<T: StackType> ExtractConfig<HarmonyStrategyConfig<T>> for ChordList<T> {
    fn extract_config(&self) -> HarmonyStrategyConfig<T> {
        HarmonyStrategyConfig::ChordList(ChordListConfig {
            patterns: self.patterns.iter().map(|p| p.extract_config()).collect(),
        })
    }
}
