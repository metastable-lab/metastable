// use voda_common::CryptoHash;

// use super::User;

// pub trait Runtime {
//     const NAME: &'static str;
//     type Error: std::error::Error;

//     // pre-run
//     fn user_authentication(&mut self, user_id: CryptoHash) -> Result<User, Self::Error>;
//     fn prepare_system(&mut self) -> Result<(), Self::Error>;
//     fn load_history(&mut self, user: &User) -> Result<(), Self::Error>;

//     fn run(&mut self, user: &User, message: &HistoryMessage) -> Result<(), Self::Error>;

//     // post-run
//     fn save_history(&mut self, user: &User) -> Result<(), Self::Error>;
//     fn update_user(&mut self, user: &User) -> Result<(), Self::Error>;
// }