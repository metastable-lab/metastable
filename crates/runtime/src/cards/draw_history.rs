use anyhow::{bail, Result};
use sqlx::types::Uuid;
use metastable_database::{SqlxObject};
use serde::{Deserialize, Serialize};
use rand::{random_range, random_ratio};

use crate::User;
use crate::cards::{Card, CardPool, DrawProbability};

#[derive(Clone, Default, Debug, Serialize, Deserialize, SqlxObject)]
#[table_name = "draw_history"]
pub struct DrawHistory {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub user: Uuid,
    #[foreign_key(referenced_table = "card_pool", related_rust_type = "CardPool")]
    pub card_pool_id: Uuid,
    #[foreign_key(referenced_table = "card", related_rust_type = "Card")]
    pub card_id: Uuid,
    
    pub last_s: i64,
    pub last_ss: i64,

    pub created_at: i64,
    pub updated_at: i64,
}

impl DrawHistory {
    fn select_card_from_pool(cards: &[Uuid], prob: i64) -> Result<Uuid> {
        if cards.is_empty() {
            bail!("DrawHistory::select_card_from_pool: cards is empty");
        }
        let prob = prob % 10000;
        if random_ratio(prob as u32, 10000) {
            let random_index = random_range(0..cards.len());
            Ok(cards[random_index].clone())
        } else {
            Err(anyhow::anyhow!("DrawHistory::select_card_from_pool: random_index is greater than cards.len(). Not selected"))
        }
    }

    fn ss_probability(last_ss_count: i64, probability: &DrawProbability) -> i64 {
        if last_ss_count + 1 >= probability.ss_guaranteed_count {
            return 10000;
        }
        
        let mut current_rate = probability.ss_base_prob;
        
        if last_ss_count >= 70 && last_ss_count <= 80 {
            let delta = last_ss_count - 70;
            current_rate += probability.ss_prob_increase_slope * delta;
        } else if last_ss_count >= 81 && last_ss_count <= 89 {
            let base_rate_at_80 = probability.ss_base_prob + probability.ss_prob_increase_slope * 10;
            let delta = last_ss_count - 80;
            current_rate = base_rate_at_80 - probability.ss_prob_decrease_slope * delta;
        }
        
        current_rate.max(0).min(10000)
    }

    fn s_probability(last_s_count: i64, probability: &DrawProbability) -> i64 {
        if last_s_count >= probability.s_guaranteed_count {
            return 10000;
        }
        probability.s_base_prob
    }

    pub fn execute_single_draw(
        last_draw_history: &Self,
        card_pool: &CardPool, cards: &[Card],
    ) -> Result<Self> {        
        let probability = card_pool.pool_settings.clone();
        
        let ss_prob = Self::ss_probability(last_draw_history.last_ss, &probability);
        let s_prob = Self::s_probability(last_draw_history.last_s, &probability);

        let ss_card_pool = cards.iter().filter(|c| c.rating == 0).map(|c| c.id).collect::<Vec<_>>();
        let s_card_pool = cards.iter().filter(|c| c.rating == 1).map(|c| c.id).collect::<Vec<_>>();
        let a_card_pool = cards.iter().filter(|c| c.rating == 2).map(|c| c.id).collect::<Vec<_>>();

        if a_card_pool.is_empty() {
            bail!("DrawHistory::execute_single_draw: a_card_pool is empty, pool misconfiguration");
        }

        let selected_card = Self::select_card_from_pool(&ss_card_pool, ss_prob)
            .or_else(|_| Self::select_card_from_pool(&s_card_pool, s_prob))
            .or_else(|_| Self::select_card_from_pool(&a_card_pool, 10000))
            .expect("DrawHistory::execute_single_draw: no card selected, pool misconfiguration");

        let is_ss = ss_card_pool.contains(&selected_card);
        let is_s = s_card_pool.contains(&selected_card);
        
        Ok(Self {
            id: Uuid::new_v4(),
            user: last_draw_history.user,
            card_pool_id: card_pool.id,
            card_id: selected_card,

            last_s: if is_s { 0 } else { last_draw_history.last_s + 1 },
            last_ss: if is_ss { 0 } else { last_draw_history.last_ss + 1 },

            created_at: 0,
            updated_at: 0,
        })
    }

    pub fn draw_ten_cards(
        last_draw_history: &Self,
        card_pool: &CardPool, cards: &[Card],
    ) -> Result<Vec<Self>> {
        
        let mut results = Vec::new();
        let mut last_draw = last_draw_history.clone();
        for _ in 0..10 {
            let result = Self::execute_single_draw(
                &last_draw, card_pool, cards,
            )?;

            last_draw = result.clone();
            results.push(result);
        }
        Ok(results)
    }
}