#[allow(async_fn_in_trait)]
pub trait RuntimeClient<IN, OUT> {
    type Error;

    fn get_price(&self) -> u64;
    async fn run(&self, input: &IN) -> Result<OUT, Self::Error>;
    async fn regenerate(&self, input: &IN) -> Result<OUT, Self::Error>;
}
