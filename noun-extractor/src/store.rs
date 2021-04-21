use anyhow::Result;
use serde::{Serialize, de::DeserializeOwned};
use std::hash::Hash;
use std::path::Path;
pub trait Store<K, V>: Sized
where
    K: Eq + Hash + PartialEq + PartialOrd + Serialize + DeserializeOwned,
    V: Serialize + DeserializeOwned + Copy,
{
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    fn get(&self, k: &K) -> Result<Option<V>>;
    fn put(&mut self, k: K, v: V) -> Result<()>;
    fn save(&self) -> Result<()>;
}

pub mod hashmap_store {
    use super::Store;
    use anyhow::Result;
    //use serde::{Deserialize, Serialize};
    use serde::{Serialize, de::DeserializeOwned};
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::path::{Path, PathBuf};

    pub struct StoreImpl<K, V> {
        inner: HashMap<K, V>,
        path: PathBuf,
    }

    impl<K, V> Store<K, V> for StoreImpl<K, V>
    where
        K: Eq + Hash + PartialEq + PartialOrd + Serialize + DeserializeOwned,
        V: Serialize + DeserializeOwned + Copy,
    {
        fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
            let inner = match std::fs::read(&(*path.as_ref())) {
                Ok(bytes) => bincode::deserialize(&bytes)?,
                Err(_) => HashMap::<K, V>::new(),
            };
            Ok(Self {
                inner,
                path: path.as_ref().to_path_buf(),
            })
        }
        fn get(&self, k: &K) -> Result<Option<V>> {
            Ok(self.inner.get(&k).copied())
        }
        fn put(&mut self, k: K, v: V) -> Result<()> {
            self.inner.insert(k, v);
            Ok(())
        }
        fn save(&self) -> Result<()> {
            if let Some(p) = self.path.parent() {
                std::fs::create_dir_all(p)?;
            }
            //std::fs::write(self.path.clone(), &bincode::serialize(&self.inner)?)?;
            std::fs::write(self.path.clone(), bincode::serialize(&self.inner)?)?;
            Ok(())
        }
    }
}

/*
#[cfg(feature = "faster-rs")]
mod inner {
    use super::*;
    use anyhow::Result;
    use faster_rs::{status, FasterKv, FasterKvBuilder};
    use serde::{Deserialize, Serialize};
    use std::path::Path;
    use std::sync::mpsc::Receiver;

    pub struct StoreImpl<K, V> {
        inner: FasterKv,
        serial: u64,
        _marker: std::marker::PhantomData<fn() -> (K, V)>,
    }

    impl<K, V> Store<K, V>
    where
        K: Eq + Serialize + for<'a> Deserialize<'a>,
        V: Serialize + for<'a> Deserialize<'a>,
    {
        pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
            if let Some(p) = path.as_ref().parent() {
                std::fs::create_dir_all(p)?;
            }
            Ok(Self {
                inner: FasterKvBuilder::new(1 << 15, 1024 * 1024 * 1024)
                    .with_disk(path.as_ref().to_str().unwrap())
                    .set_pre_allocate_log(true)
                    .build()?,
                serial: 1u64,
                _marker: std::marker::PhantomData,
            })
        }
        pub fn get(&self, k: &K) -> Result<Option<V>> {
            let (read, recv): (u8, Receiver<V>) = self.inner.read(k, 1);
            match read {
                status::OK | status::PENDING => Ok(recv.recv().ok()),
                status::NOT_FOUND => Ok(None),
                s => return Err(anyhow::Error::msg(format!("{:?}", s))),
            }
        }
        pub fn put(&mut self, k: &K, v: &V) -> Result<()> {
            self.serial += 1;
            let upsert = self.inner.upsert(k, v, self.serial);
            match upsert {
                status::OK | status::PENDING => Ok(()),
                s => Err(anyhow::Error::msg(format!("{:?}", s))),
            }
        }
        pub fn save(&self) -> Result<()> {
            self.inner.checkpoint().unwrap();
            Ok(())
        }
    }
}

#[cfg(feature = "rocksdb")]
mod inner {
    use super::Store;
    use anyhow::Result;
    use borsh::{BorshDeserialize, BorshSerialize};
    use rocksdb::{BlockBasedOptions, Options, DB};
    use std::hash::Hash;
    use std::path::Path;

    fn rocksdb_default_opts() -> Options {
        let mut opts = Options::default();
        // https://github.com/facebook/rocksdb/wiki/Setup-Options-and-Basic-Tuning
        #[allow(deprecated)]
        opts.set_max_background_compactions(4);
        #[allow(deprecated)]
        opts.set_max_background_flushes(2);
        opts.set_level_compaction_dynamic_level_bytes(true);
        opts.set_bytes_per_sync(1048576);
        opts.create_if_missing(true);

        let mut table_opts = BlockBasedOptions::default();
        table_opts.set_pin_l0_filter_and_index_blocks_in_cache(true);
        table_opts.set_cache_index_and_filter_blocks(true);
        table_opts.set_cache_index_and_filter_blocks(true);
        table_opts.set_block_size(16 * 1024);
        table_opts.set_format_version(5);

        // options.compaction_pri = kMinOverlappingRatio;
        opts.set_block_based_table_factory(&table_opts);
        opts
    }

    pub struct StoreImpl<K, V> {
        inner: DB,
        _marker: std::marker::PhantomData<fn() -> (K, V)>,
    }

    impl<K, V> Store<K, V> for StoreImpl<K, V>
    where
        K: Eq + Hash + PartialEq + PartialOrd + BorshSerialize + BorshDeserialize,
        V: BorshSerialize + BorshDeserialize + Copy,
    {
        fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
            Ok(Self {
                inner: DB::open(&rocksdb_default_opts(), path)?,
                _marker: std::marker::PhantomData,
            })
        }
        fn get(&self, k: &K) -> Result<Option<V>> {
            Ok(match self.inner.get(k.try_to_vec()?)? {
                Some(bytes) => Some(V::try_from_slice(&bytes)?),
                None => None,
            })
        }
        fn put(&mut self, k: K, v: V) -> Result<()> {
            Ok(self.inner.put(k.try_to_vec()?, v.try_to_vec()?)?)
        }
        fn save(&self) -> Result<()> {
            Ok(())
        }
    }
}

#[cfg(feature = "hashmap")]
mod inner {
    use super::Store;
    use anyhow::Result;
    //use serde::{Deserialize, Serialize};
    use borsh::{BorshDeserialize, BorshSerialize};
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::path::{Path, PathBuf};

    pub struct StoreImpl<K, V>
    where
        K: Eq + Hash,
    {
        inner: HashMap<K, V>,
        path: PathBuf,
    }

    impl<K, V> Store<K, V> for StoreImpl<K, V>
    where
        K: Eq + Hash + PartialEq + PartialOrd + BorshSerialize + BorshDeserialize,
        V: BorshSerialize + BorshDeserialize + Copy,
    {
        fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
            let inner = match std::fs::read(&(*path.as_ref())) {
                Ok(bytes) => HashMap::<K, V>::try_from_slice(&bytes)?,
                Err(_) => HashMap::<K, V>::new(),
            };
            Ok(Self {
                inner,
                path: path.as_ref().to_path_buf(),
            })
        }
        fn get(&self, k: &K) -> Result<Option<V>> {
            Ok(self.inner.get(&k).copied())
        }
        fn put(&mut self, k: K, v: V) -> Result<()> {
            self.inner.insert(k, v);
            Ok(())
        }
        fn save(&self) -> Result<()> {
            if let Some(p) = self.path.parent() {
                std::fs::create_dir_all(p)?;
            }
            //std::fs::write(self.path.clone(), &bincode::serialize(&self.inner)?)?;
            std::fs::write(self.path.clone(), self.inner.try_to_vec()?)?;
            Ok(())
        }
    }
}

pub use inner::StoreImpl;
*/
