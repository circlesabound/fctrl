use std::{path::Path, sync::Arc};

use crate::{
    consts,
    error::{Error, Result},
};

use tokio::fs;

type RocksDbMultiThreaded = rocksdb::DBWithThreadMode<rocksdb::MultiThreaded>;

pub struct Db {
    primary: RocksDbMultiThreaded,
}

impl Db {
    pub async fn open_or_new(db_dir: impl AsRef<Path>) -> Result<Db> {
        fs::create_dir_all(&db_dir).await?;

        let db_path = db_dir.as_ref().join(consts::DB_NAME);

        let cfs;
        if Db::exists(&db_path).await {
            // need to read CFs before loading
            cfs = RocksDbMultiThreaded::list_cf(&rocksdb::Options::default(), &db_path)?;
        } else {
            cfs = vec![rocksdb::DEFAULT_COLUMN_FAMILY_NAME.to_owned()];
        }

        let mut open_options = rocksdb::Options::default();
        open_options.create_if_missing(true);
        open_options.create_missing_column_families(true);
        let primary = RocksDbMultiThreaded::open_cf(&open_options, &db_path, &cfs)?;

        Ok(Db { primary })
    }

    pub fn create_cf(&self, name: &Cf) -> Result<()> {
        let opts = rocksdb::Options::default();
        Ok(self.primary.create_cf(&name.0, &opts)?)
    }

    pub fn read(&self, cf: &Cf, key: String) -> Result<Option<Record>> {
        let cfh = self.get_or_create_cf_handle(cf)?;
        let key_bytes = key.as_bytes();
        let opt_value_bytes = self.primary.get_cf(&cfh, key_bytes)?;
        let opt_ret = opt_value_bytes.map(|v| Record {
            key,
            value: String::from_utf8_lossy(v.as_ref()).to_string(),
        });
        Ok(opt_ret)
    }

    pub fn read_range(
        &self,
        cf: &Cf,
        key: String,
        direction: RangeDirection,
        count: u32,
    ) -> Result<ReadRange> {
        let cfh = self.get_or_create_cf_handle(cf)?;
        let read_opts = rocksdb::ReadOptions::default();

        let key_bytes = key.as_bytes();
        let mode = match direction {
            RangeDirection::Forward => {
                rocksdb::IteratorMode::From(key_bytes, rocksdb::Direction::Forward)
            }
            RangeDirection::Backward => {
                rocksdb::IteratorMode::From(key_bytes, rocksdb::Direction::Reverse)
            }
        };

        self.read_range_internal(cfh, read_opts, mode, count)
    }

    pub fn read_range_head(&self, cf: &Cf, count: u32) -> Result<ReadRange> {
        let cfh = self.get_or_create_cf_handle(cf)?;
        let read_opts = rocksdb::ReadOptions::default();
        let mode = rocksdb::IteratorMode::Start;

        self.read_range_internal(cfh, read_opts, mode, count)
    }

    pub fn read_range_tail(&self, cf: &Cf, count: u32) -> Result<ReadRange> {
        let cfh = self.get_or_create_cf_handle(cf)?;
        let read_opts = rocksdb::ReadOptions::default();
        let mode = rocksdb::IteratorMode::End;

        self.read_range_internal(cfh, read_opts, mode, count)
    }

    pub fn write(&self, cf: &Cf, record: &Record) -> Result<()> {
        let cfh = self.get_or_create_cf_handle(cf)?;
        Ok(self
            .primary
            .put_cf(&cfh, record.key.as_bytes(), record.value.as_bytes())?)
    }

    async fn exists(db_path: impl AsRef<Path>) -> bool {
        fs::metadata(db_path).await.map_or(false, |m| m.is_dir())
    }

    fn flush(&self) -> Result<()> {
        Ok(self.primary.flush()?)
    }

    fn get_or_create_cf_handle(&self, cf: &Cf) -> Result<Arc<rocksdb::BoundColumnFamily<'_>>> {
        self.primary.cf_handle(&cf.0).map_or_else(
            || {
                self.create_cf(cf)?;
                self.primary
                    .cf_handle(&cf.0)
                    .ok_or_else(|| Error::Db("Could not create new CF".to_owned()))
            },
            Ok,
        )
    }

    fn read_range_internal(
        &self,
        cfh: Arc<rocksdb::BoundColumnFamily>,
        read_opts: rocksdb::ReadOptions,
        mode: rocksdb::IteratorMode,
        count: u32,
    ) -> Result<ReadRange> {
        let mut iter = self.primary.iterator_cf_opt(&cfh, read_opts, mode);

        let mut continue_from = None;
        let mut records = vec![];
        for i in 0..count {
            if let Some((k, v)) = iter.next() {
                let record = Record {
                    key: String::from_utf8_lossy(&k).to_string(),
                    value: String::from_utf8_lossy(&v).to_string(),
                };
                records.push(record);
            } else {
                break;
            }

            // Read n+1 to get a continuation point
            if i == count - 1 {
                continue_from = iter
                    .next()
                    .map(|(k, _)| String::from_utf8_lossy(&k).to_string());
            }
        }

        Ok(ReadRange {
            records,
            continue_from,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub key: String,
    pub value: String,
}

/// External wrapper around column families
#[derive(Clone, Debug, derive_more::From, derive_more::Into, Hash, PartialEq, Eq)]
pub struct Cf(pub String);

#[derive(Debug)]
pub enum RangeDirection {
    Forward,
    Backward,
}

#[derive(Debug)]
pub struct ReadRange {
    pub records: Vec<Record>,
    pub continue_from: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    type GenericResult = std::result::Result<(), Box<dyn std::error::Error>>;

    #[tokio::test]
    async fn can_create_cf_then_reopen() -> GenericResult {
        fctrl::util::testing::logger_init();

        let db_dir = std::env::temp_dir().join("can_create_cf_then_reopen");
        if fs::metadata(&db_dir).await.is_ok() {
            let _ = fs::remove_dir_all(&db_dir).await;
        };

        // Db should be created with default cf
        let db = Db::open_or_new(&db_dir).await?;

        // Create new cf
        db.create_cf(&Cf("new_cf".to_owned()))?;

        // Close
        std::mem::drop(db);

        // Re-open - RocksDB will throw an error if opened in primary mode wthout all CFs
        let db = Db::open_or_new(&db_dir).await?;
        std::mem::drop(db);

        // Clean up
        let _ = fs::remove_dir_all(&db_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn can_write_then_read() -> GenericResult {
        fctrl::util::testing::logger_init();

        let db_dir = std::env::temp_dir().join("can_write_then_read");
        if fs::metadata(&db_dir).await.is_ok() {
            let _ = fs::remove_dir_all(&db_dir).await;
        };

        let cf = Cf("can_write_then_read".to_owned());
        let record = Record {
            key: "testkey".to_owned(),
            value: "testvalue".to_owned(),
        };

        let db = Db::open_or_new(&db_dir).await?;
        db.write(&cf, &record)?;

        let read = db.read(&cf, record.key.clone())?;
        assert!(read.is_some());
        assert_eq!(read.unwrap(), record);

        // Clean up
        let _ = fs::remove_dir_all(&db_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn can_read_range_forward_from_nonspecific_key() -> GenericResult {
        fctrl::util::testing::logger_init();

        let db_dir = std::env::temp_dir().join("can_read_range_forward_from_nonspecific_key");
        if fs::metadata(&db_dir).await.is_ok() {
            let _ = fs::remove_dir_all(&db_dir).await;
        };

        let cf = Cf("can_read_range_forward_from_nonspecific_key".to_owned());
        let db = Db::open_or_new(&db_dir).await?;

        db.write(
            &cf,
            &Record {
                key: "a1".to_owned(),
                value: "a1".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "a3".to_owned(),
                value: "a3".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "a4".to_owned(),
                value: "a4".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "b2".to_owned(),
                value: "b2".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "b4".to_owned(),
                value: "b4".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "b5".to_owned(),
                value: "b5".to_owned(),
            },
        )?;

        db.flush()?;

        let ret = db.read_range(&cf, "a2".to_owned(), RangeDirection::Forward, 2)?;
        assert_eq!(ret.records.len(), 2);
        let mut iter = ret.records.iter();
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "a3".to_owned(),
                value: "a3".to_owned(),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "a4".to_owned(),
                value: "a4".to_owned(),
            })
        );
        assert_eq!(ret.continue_from, Some("b2".to_owned()));

        // Clean up
        let _ = fs::remove_dir_all(&db_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn can_read_range_backward_from_nonspecific_key() -> GenericResult {
        fctrl::util::testing::logger_init();

        let db_dir = std::env::temp_dir().join("can_read_range_backward_from_nonspecific_key");
        if fs::metadata(&db_dir).await.is_ok() {
            let _ = fs::remove_dir_all(&db_dir).await;
        };

        let cf = Cf("can_read_range_backward_from_nonspecific_key".to_owned());
        let db = Db::open_or_new(&db_dir).await?;

        db.write(
            &cf,
            &Record {
                key: "a1".to_owned(),
                value: "a1".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "a3".to_owned(),
                value: "a3".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "a4".to_owned(),
                value: "a4".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "b2".to_owned(),
                value: "b2".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "b4".to_owned(),
                value: "b4".to_owned(),
            },
        )?;
        db.write(
            &cf,
            &Record {
                key: "b5".to_owned(),
                value: "b5".to_owned(),
            },
        )?;

        db.flush()?;

        let ret = db.read_range(&cf, "b3".to_owned(), RangeDirection::Backward, 2)?;
        assert_eq!(ret.records.len(), 2);
        let mut iter = ret.records.iter();
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "b2".to_owned(),
                value: "b2".to_owned(),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "a4".to_owned(),
                value: "a4".to_owned(),
            })
        );
        assert_eq!(ret.continue_from, Some("a3".to_owned()));

        // Clean up
        let _ = fs::remove_dir_all(&db_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn can_write_then_read_range_head() -> GenericResult {
        fctrl::util::testing::logger_init();

        let db_dir = std::env::temp_dir().join("can_write_then_read_range_head");
        if fs::metadata(&db_dir).await.is_ok() {
            let _ = fs::remove_dir_all(&db_dir).await;
        };

        let cf = Cf("can_write_then_read_range_head".to_owned());
        let db = Db::open_or_new(&db_dir).await?;

        for i in 0..10 {
            let record = Record {
                key: i.to_string(),
                value: i.to_string(),
            };
            db.write(&cf, &record)?;
        }

        db.flush()?;

        let ret = db.read_range_head(&cf, 3)?;
        assert_eq!(ret.records.len(), 3);
        let mut iter = ret.records.iter();
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "0".to_owned(),
                value: "0".to_owned(),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "1".to_owned(),
                value: "1".to_owned(),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "2".to_owned(),
                value: "2".to_owned(),
            })
        );
        assert_eq!(ret.continue_from, Some("3".to_owned()));

        // Clean up
        let _ = fs::remove_dir_all(&db_dir).await;

        Ok(())
    }

    #[tokio::test]
    async fn can_write_then_read_range_tail() -> GenericResult {
        fctrl::util::testing::logger_init();

        let db_dir = std::env::temp_dir().join("can_write_then_read_range_tail");
        if fs::metadata(&db_dir).await.is_ok() {
            let _ = fs::remove_dir_all(&db_dir).await;
        };

        let cf = Cf("can_write_then_read_range_tail".to_owned());
        let db = Db::open_or_new(&db_dir).await?;

        for i in 0..10 {
            let record = Record {
                key: i.to_string(),
                value: i.to_string(),
            };
            db.write(&cf, &record)?;
        }

        db.flush()?;

        let ret = db.read_range_tail(&cf, 3)?;
        assert_eq!(ret.records.len(), 3);
        let mut iter = ret.records.iter();
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "9".to_owned(),
                value: "9".to_owned(),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "8".to_owned(),
                value: "8".to_owned(),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&Record {
                key: "7".to_owned(),
                value: "7".to_owned(),
            })
        );
        assert_eq!(ret.continue_from, Some("6".to_owned()));

        // Clean up
        let _ = fs::remove_dir_all(&db_dir).await;

        Ok(())
    }
}
