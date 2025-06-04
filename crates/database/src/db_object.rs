
use mongodb::error::Error as MongoDbError;
use mongodb::bson::{self, doc, Document};
use mongodb::Database;
use mongodb::options::FindOptions;

use futures::StreamExt;
use serde::{de::DeserializeOwned, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use voda_common::CryptoHash;

#[allow(async_fn_in_trait)]
pub trait MongoDbObject: 
    Sized + Serialize + DeserializeOwned + Sync + Unpin + Send + Clone
{
    const COLLECTION_NAME: &'static str;
    type Error: From<MongoDbError> + From<bson::ser::Error> + From<bson::de::Error>;

    fn populate_id(&mut self);
    fn get_id(&self) -> CryptoHash;

    async fn save_many(db: &Database, mut objs: Vec<Self>) -> Result<(), Self::Error> {
        let col = db.collection::<Self>(Self::COLLECTION_NAME);
        objs.iter_mut().for_each(|obj| obj.populate_id());
        col.insert_many(objs, None).await?;
        Ok(())
    }

    async fn save(self, db: &Database) -> Result<(), Self::Error> {
        Self::save_many(db, vec![self]).await
    }

    async fn save_or_update(self, db: &Database) -> Result<(), Self::Error> {
        let maybe_exists = Self::select_one_by_index(db, &self.get_id()).await?;
        match maybe_exists {
            Some(_) => self.update(db).await,
            None => self.save(db).await,
        }
    }

    async fn update(&self, db: &Database) -> Result<(), Self::Error> {
        let col = db.collection(Self::COLLECTION_NAME);
        col.replace_one(
            doc! { "_id": self.get_id().to_hex_string() },
            bson::to_document(&self).map_err(Self::Error::from)?,
            None,
        )
        .await?;
        Ok(())
    }

    async fn delete(mut self, db: &Database) -> Result<(), Self::Error> {
        self.populate_id();
        let col = db.collection::<Document>(Self::COLLECTION_NAME);
        let _ = col.delete_one(doc! { "_id": self.get_id().to_hex_string() }, None).await?;
        Ok(())
    }

    async fn delete_many(db: &Database, query: Document) -> Result<(), Self::Error> {
        let col = db.collection::<Document>(Self::COLLECTION_NAME);
        let _ = col.delete_many(query, None).await?;
        Ok(())
    }


    async fn select_one_by_index(db: &Database, index: &CryptoHash) -> Result<Option<Self>, Self::Error> {
        let col = db.collection::<Document>(Self::COLLECTION_NAME);
        let doc = col.find_one(doc! { "_id": index.to_hex_string() }, None).await?;
        match doc  {
            Some(d) => Ok(Some(
                bson::from_document(d)
                    .map_err(Self::Error::from)?
            )), 
            None => Ok(None)
        }
    }

    async fn select_one_by_filter(db: &Database, filter: Document) -> Result<Option<Self>, Self::Error> {
        let col = db.collection::<Document>(Self::COLLECTION_NAME);
        let doc = col.find_one(filter, None).await?;
        match doc  {
            Some(d) => Ok(Some(
                bson::from_document(d)
                    .map_err(Self::Error::from)?
            )), 
            None => Ok(None)
        }
    }

    async fn select_many_simple(db: &Database, filter: Document) -> Result<Vec<Self>, Self::Error> {
        Self::select_many(db, filter, None, None).await
    }

    async fn select_many(
        db: &Database, filter: Document, 
        limit: Option<i64>, skip: Option<u64>
    ) -> Result<Vec<Self>, Self::Error> {
        let col = db.collection::<Document>(Self::COLLECTION_NAME);
        let options = FindOptions::builder()
            .limit(limit)
            .skip(skip)
            .build();

        let mut docs = col.find(filter, Some(options)).await?;
        let mut vec = Vec::new();
        while let Some(doc) = docs.next().await {
            vec.push(bson::from_document(doc?).map_err(Self::Error::from)?);
        }
        Ok(vec)
    }

    async fn select_many_stream(
        db: &Database, filter: Document, 
        limit: Option<i64>, skip: Option<u64>,
        tx: &UnboundedSender<Self>,
    ) -> Result<(), Self::Error> {
        let col = db.collection::<Document>(Self::COLLECTION_NAME);
        let options = FindOptions::builder()
            .limit(limit)
            .skip(skip)
            .build();

        let mut docs = col.find(filter, Some(options)).await?;
        while let Some(doc) = docs.next().await {
            tx.send(bson::from_document(doc?).map_err(Self::Error::from)?).unwrap();
        }
        Ok(())
    }

    async fn total_count(db: &Database, filter: Document) -> Result<u64, Self::Error> {
        let col = db.collection::<Document>(Self::COLLECTION_NAME);
        let total_count = col.count_documents(filter, None).await?;
        Ok(total_count)
    }
}
