use serde_derive::{Deserialize, Serialize};

use crate::interval::{stacktype::r#trait::IntervalBasis, temperament::TemperamentDefinition};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config<T: IntervalBasis> {
    pub temperaments: Vec<TemperamentDefinition<T>>,
}
