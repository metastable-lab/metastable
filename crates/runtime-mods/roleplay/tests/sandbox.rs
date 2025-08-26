#[cfg(test)]
mod test {
    use metastable_runtime_roleplay::agents::PrettierV0Agent;
    use metastable_runtime::Agent;
    use sqlx::types::Uuid;

    #[tokio::test]
    async fn test_prettier_v0() {
        let agent = PrettierV0Agent::new().await.unwrap();
        let output = agent.call(&Uuid::new_v4(), &()).await.unwrap();
        println!("{:?}", output);
    }

}