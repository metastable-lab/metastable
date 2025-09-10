use serde::{Deserialize, Serialize};
use metastable_database::{TextEnum, TextEnumCodec};

mod card;
mod draw_history;
mod card_pool;

pub use card::Card;
pub use draw_history::DrawHistory;
pub use card_pool::CardPool;

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Default, TextEnum)]
pub enum DrawType {
    #[default]
    Single,
    Ten,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrawProbability {
    pub ss_base_prob: i64,      
    pub s_base_prob: i64,

    pub ss_guaranteed_count: i64,
    pub s_guaranteed_count: i64,   

    pub ss_prob_increase_slope: i64,
    pub ss_prob_decrease_slope: i64,  
}

impl Default for DrawProbability {
    fn default() -> Self {
        Self {
            ss_base_prob: 150,
            s_base_prob: 1500,

            ss_guaranteed_count: 90,
            s_guaranteed_count: 10,

            ss_prob_increase_slope: 400,
            ss_prob_decrease_slope: 200, 
        }
    }
}