use crate::{consts, error::{Error, Result}};

use log::info;
use tokio::fs;

type RocksDbMultiThreaded = rocksdb::DBWithThreadMode<rocksdb::MultiThreaded>;

pub struct Db {
    primary: RocksDbMultiThreaded,
}

impl Db {
    pub async fn new() -> Result<Db> {
        let cfs;
        if Db::exists().await {
            // need to read CFs before loading
            cfs = RocksDbMultiThreaded::list_cf(&rocksdb::Options::default(), &*consts::DB_PATH)?;
        } else {
            cfs = vec![rocksdb::DEFAULT_COLUMN_FAMILY_NAME.to_owned()];
        }

        info!("cfs: {:?}", cfs);
        let primary = RocksDbMultiThreaded::open_cf(&rocksdb::Options::default(), &*consts::DB_PATH, &cfs)?;

        Ok(Db {
            primary,
        })
    }

    pub async fn create_cf(&self, name: &Cf) -> Result<()> {
        let opts = rocksdb::Options::default();
        Ok(self.primary.create_cf(&name.0, &opts)?)
    }

    pub async fn write(&self, cf: &Cf, record: &Record) -> Result<()> {
        let cfh = self.get_or_create_cf_handle(cf).await?;
        Ok(self.primary.put_cf(cfh, record.key.as_bytes(), record.value.as_bytes())?)
    }

    async fn exists() -> bool {
        fs::metadata(&*consts::DB_PATH).await.map_or(false, |m| m.is_dir())
    }

    async fn get_or_create_cf_handle(&self, cf: &Cf) -> Result<rocksdb::BoundColumnFamily<'_>> {
        match self.primary.cf_handle(&cf.0) {
            Some(cfh) => Ok(cfh),
            None => {
                self.create_cf(cf).await?;
                Ok(self.primary.cf_handle(&cf.0).ok_or(Error::Db("Could not create new CF".to_owned()))?)
            },
        }
    }
}

#[derive(Debug)]
pub struct Record {
    pub key: String,
    pub value: String,
}

/// External wrapper around column families
#[derive(Clone, Debug, derive_more::From, derive_more::Into, Hash, PartialEq, Eq)]
pub struct Cf(pub String);

#[cfg(test)]
mod tests {
    use super::*;

    use log::error;

    #[tokio::test]
    async fn test123() -> std::result::Result<(), Box<dyn std::error::Error>> {
        fctrl::util::testing::logger_init();

        let _ = Db::new().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_rocksdb() -> std::result::Result<(), Box<dyn std::error::Error>> {
        fctrl::util::testing::logger_init();

        fs::create_dir_all(&*consts::DB_DIR).await?;

        info!("Opening db");
        let db_path = consts::DB_DIR.join("testdb");
        let secondary_path = consts::DB_DIR.join("testdb_secondary");
        let db = rocksdb::DB::open_default(&db_path)?;

        info!("Opening secondary");
        let mut opts = rocksdb::Options::default();
        opts.set_max_open_files(-1);
        let db_r = rocksdb::DB::open_as_secondary(&opts, &db_path, &secondary_path)?;

        info!("Writing {{'key','hello this is value'}} to db");
        db.put(b"key", b"hello this is value")?;

        info!("Reading from secondary");
        db_r.try_catch_up_with_primary()?;

        match db_r.get(b"key") {
            Ok(Some(value)) => {
                info!(
                    "Retrieved written value from the db: {}",
                    String::from_utf8(value).unwrap()
                );
            }
            Ok(None) => {
                error!("Retrieved empty value from db");
            }
            Err(e) => {
                error!("Error retrieving value from db: {:?}", e)
            }
        }
        db.delete(b"key")?;
        Ok(())
    }
}
