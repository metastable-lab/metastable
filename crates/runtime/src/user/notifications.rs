use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::types::Uuid;
use metastable_database::{SqlxObject, TextEnum};

use crate::{Character, CharacterPost, User};

#[derive(Debug, Clone, Default, TextEnum)]
pub enum NotificationType {
    #[default]
    SystemNotification,

    NewFollower,
    NewCharacterFavorite,
    NewPostComment,
    
    CharacterReviewOutcomePublished,
    CharacterReviewOutcomeRejected,

    PaymentProcessed,
    ReferralUsed,
}


#[derive(Debug, Serialize, Deserialize, Clone, Default, SqlxObject)]
#[table_name = "user_notifications"]
pub struct UserNotification {
    pub id: Uuid,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub from: Option<Uuid>,

    #[foreign_key(referenced_table = "users", related_rust_type = "User")]
    pub to: Option<Uuid>,

    pub notification_type: NotificationType,
    pub content: Option<String>,

    #[foreign_key(referenced_table = "roleplay_characters", related_rust_type = "Character")]
    pub related_characters: Option<Uuid>,
    #[foreign_key(referenced_table = "roleplay_character_posts", related_rust_type = "CharacterPost")]
    pub related_posts: Option<Uuid>,

    pub created_at: i64,
    pub updated_at: i64,
}

impl UserNotification {
    pub fn system_notification(content: String) -> Self {
        Self {
            id: Uuid::default(),
            from: None,
            to: None,
            notification_type: NotificationType::SystemNotification,
            content: Some(content),
            related_characters: None,
            related_posts: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn new_follower(from: Uuid, to: Uuid) -> Self {
        Self {
            id: Uuid::default(),
            from: Some(from),
            to: Some(to),
            notification_type: NotificationType::NewFollower,
            content: None,
            related_characters: None,
            related_posts: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn new_character_favorite(by: Uuid, character_id: Uuid) -> Self {
        Self {
            id: Uuid::default(),
            from: Some(by),
            to: None,
            notification_type: NotificationType::NewCharacterFavorite,
            content: None,
            related_characters: Some(character_id),
            related_posts: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn new_post_comment(by: Uuid, post_id: Uuid, comment: String) -> Self {
        Self {
            id: Uuid::default(),
            from: Some(by),
            to: None,
            notification_type: NotificationType::NewPostComment,
            content: Some(comment),
            related_characters: None,
            related_posts: Some(post_id),
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn character_review_outcome_published(user_id: Uuid, character_id: Uuid, message: String) -> Self {
        Self {
            id: Uuid::default(),
            from: None,
            to: Some(user_id),
            notification_type: NotificationType::CharacterReviewOutcomePublished,
            content: Some(message),
            related_characters: Some(character_id),
            related_posts: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn character_review_outcome_rejected(user_id: Uuid, character_id: Uuid, message: String) -> Self {
        Self {
            id: Uuid::default(),
            from: None,
            to: Some(user_id),
            notification_type: NotificationType::CharacterReviewOutcomeRejected,
            content: Some(message),
            related_characters: Some(character_id),
            related_posts: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn payment_processed(user_id: Uuid, message: String) -> Self {
        Self {
            id: Uuid::default(),
            from: None,
            to: Some(user_id),
            notification_type: NotificationType::PaymentProcessed,
            content: Some(message),
            related_characters: None,
            related_posts: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    pub fn referral_used(referred_by: Uuid, user_id: Uuid) -> Self {
        Self {
            id: Uuid::default(),
            from: Some(user_id),
            to: Some(referred_by),
            notification_type: NotificationType::ReferralUsed,
            content: None,
            related_characters: None,
            related_posts: None,
            created_at: 0,
            updated_at: 0,
        }
    }
}