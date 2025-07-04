mod extract_facts;
mod extract_memory;

use async_openai::types::FunctionObject;

pub struct LlmConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: i32,
    pub system_prompt: String,
    pub tools: Vec<FunctionObject>
}
pub use crate::llm::extract_facts::{FactsToolcall, get_extract_facts_config};
pub use crate::llm::extract_memory::{MemoryToolcall, get_extract_memory_config};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PgVectorDatabase;
    use crate::{PgVector, EmbeddingData};

    #[tokio::test]
    async fn test_pgvector_extract_facts() {
        let user_id = "123".to_string();
        let conversation_data = "My name is Zhang san and I'm a software engineer. I like Sichuan cuisine and my favorite movie is Inception. I plan to go on a business trip to Shanghai next week。".to_string();

        let db = match PgVectorDatabase::new().await {
            Ok(db) => db,
            Err(e) => {
                println!("Failed to create database connection: {}", e);
                println!("Skipping test due to database connection failure");
                return;
            }
        };

        let (facts_config, facts_prompt) = get_extract_facts_config(user_id.clone(), conversation_data.clone());
        let facts_result = db.llm(&facts_config, &facts_prompt).await.unwrap();
        println!("LLM content: {}", facts_result);
    }

    #[tokio::test]
    async fn test_pgvector_extract_memory() {
        use crate::pgvector::Vector;
        use uuid::Uuid;
    
        // 1. 初始化环境
        dotenv::dotenv().ok();
        let user_id = Uuid::new_v4();
        let test_run_id = Uuid::new_v4(); // 用于标记本次测试的所有数据
    
        // 2. 初始化数据库（失败时 panic 以暴露环境问题）
        let db = PgVectorDatabase::new().await.expect("Failed to connect to database");
        db.init().await.expect("Failed to initialize database");
        let vector_store = PgVector::new(db.db.clone());
    
        // 3. 定义测试辅助函数
        async fn ask_question(
            db: &PgVectorDatabase,
            vector_store: &PgVector,
            user_id: Uuid,
            question: &str,
            expected_keywords: &[&str],
        ) {
            let embeddings = db.embed(vec![question.to_string()]).await.unwrap();
            let query_data = EmbeddingData::new(
                embeddings[0].clone(),
                user_id,
                Uuid::new_v4(),
                Some(question.to_string()),
            );
            
            let memories = vector_store
                .search_embeddings(&query_data, 5)
                .await
                .unwrap()
                .into_iter()
                .filter_map(|r| r.embedding_data.content)
                .collect::<Vec<_>>();
    
            let (config, prompt) = get_extract_memory_config(user_id.to_string(), question.to_string(), &memories);
            let answer = db.llm(&config, &prompt).await.unwrap();
    
            // 验证回答是否包含预期关键词
            for keyword in expected_keywords {
                assert!(
                    answer.contains(keyword),
                    "Answer '{}' should contain '{}'",
                    answer,
                    keyword
                );
            }
        }
    
        // 4. 测试流程
        // 步骤1: 初始状态无记忆
        ask_question(&db, &vector_store, user_id, "What is my favorite food?", &["don't know"]).await;
    
        // 步骤2: 添加食物记忆
        let foods = vec!["durian", "mango"];
        for food in &foods {
            let fact = format!("Likes {}", food);
            let embeddings = db.embed(vec![fact.clone()]).await.unwrap();
            let data = EmbeddingData::new(
                embeddings[0].clone(),
                user_id,
                test_run_id, // 使用测试标记ID便于清理
                Some(fact),
            );
            vector_store.add_embeddings(vec![data]).await.unwrap();
        }
    
        // 步骤3: 验证食物记忆
        ask_question(&db, &vector_store, user_id, "What is my favorite food?", &foods).await;
    
        // 步骤4: 删除食物记忆
        vector_store
            .delete_embeddings_by_content(&user_id.to_string(), "durian")
            .await
            .unwrap();
    
        // 步骤5: 验证删除结果
        ask_question(
            &db,
            &vector_store,
            user_id,
            "What is my favorite food?",
            &["mango"],
        )
        .await;
    }

}