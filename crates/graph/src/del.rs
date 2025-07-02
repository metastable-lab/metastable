use anyhow::Result;
use neo4rs::{query, Row};
use crate::{relation::Relationship, GraphDatabase};

impl GraphDatabase {
    pub async fn delete(&self, relationships: &[Relationship]) -> Result<Vec<Row>> {
        let mut results = Vec::new();
        let mut tx = self.db.start_txn().await?;
        for relationship in relationships {
            // TODO: look into :Entity === __Entity__
            let cypher = format!(r#"
                MATCH (n:Entity {{name: $source_name, user_id: $user_id}})
                -[r:{}]->
                (m:Entity {{name: $dest_name, user_id: $user_id}})
                DELETE r
            "#, relationship.relationship);

            let query = query(&cypher)
                .param("source_name", relationship.source.clone())
                .param("dest_name", relationship.destination.clone())
                .param("user_id", relationship.user_id.clone());
            
            let mut result = tx.execute(query).await?;
            while let Some(row) = result.next(&mut tx.handle()).await? {
                results.push(row);
            }
        }

        tx.commit().await?;
        Ok(results)
    }
}