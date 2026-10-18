use std::collections::{HashMap, VecDeque};
use std::hash::Hash;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CacheStats {
    pub bytes: usize,
    pub hits: u64,
    pub misses: u64,
}

#[derive(Debug)]
pub(crate) struct ByteLruCache<K, V> {
    max_bytes: usize,
    current_bytes: usize,
    map: HashMap<K, (V, usize)>,
    order: VecDeque<K>,
    hits: u64,
    misses: u64,
}

impl<K, V> ByteLruCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub(crate) fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            current_bytes: 0,
            map: HashMap::new(),
            order: VecDeque::new(),
            hits: 0,
            misses: 0,
        }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<V> {
        let value = self.map.get(key).map(|(value, _)| value.clone());
        if value.is_some() {
            self.hits += 1;
            self.touch(key);
        } else {
            self.misses += 1;
        }
        value
    }

    pub(crate) fn insert(&mut self, key: K, value: V, size: usize) {
        if self.max_bytes == 0 {
            return;
        }

        if let Some((_, old_size)) = self.map.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(old_size);
            self.order.retain(|item| item != &key);
        }
        if size > self.max_bytes {
            return;
        }
        while self.current_bytes + size > self.max_bytes {
            let Some(old_key) = self.order.pop_front() else {
                break;
            };
            if let Some((_, old_size)) = self.map.remove(&old_key) {
                self.current_bytes = self.current_bytes.saturating_sub(old_size);
            }
        }

        self.current_bytes += size;
        self.map.insert(key.clone(), (value, size));
        self.order.push_back(key);
    }

    pub(crate) fn clear(&mut self) {
        self.current_bytes = 0;
        self.map.clear();
        self.order.clear();
    }

    pub(crate) fn stats(&self) -> CacheStats {
        CacheStats {
            bytes: self.current_bytes,
            hits: self.hits,
            misses: self.misses,
        }
    }

    fn touch(&mut self, key: &K) {
        self.order.retain(|item| item != key);
        self.order.push_back(key.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_entries() {
        let mut cache = ByteLruCache::new(10);
        cache.insert("a", vec![1u8], 4);
        cache.insert("b", vec![2u8], 4);
        assert!(cache.get(&"a").is_some());
        cache.insert("c", vec![3u8], 4);
        assert!(cache.get(&"b").is_none());
        assert!(cache.get(&"a").is_some());
        assert!(cache.get(&"c").is_some());
    }

    #[test]
    fn disabled_cache_never_stores() {
        let mut cache = ByteLruCache::new(0);
        cache.insert("a", vec![1u8], 1);
        assert!(cache.get(&"a").is_none());
        assert_eq!(cache.stats().bytes, 0);
    }
}
