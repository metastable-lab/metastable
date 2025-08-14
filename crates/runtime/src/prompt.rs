use crate::MessageType;

pub trait Prompt {
    fn prompt_type() -> MessageType;
}