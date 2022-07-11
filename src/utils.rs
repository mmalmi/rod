use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn random_string(len: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

/// When full, every insert pushes out the oldest entry in the set.
///
/// Used to record last seen message IDs.
pub struct BoundedHashSet {
    set: HashSet<String>,
    queue: VecDeque<String>,
    max_entries: usize,
}

impl BoundedHashSet {
    pub fn new(max_entries: usize) -> Self {
        BoundedHashSet {
            set: HashSet::new(),
            queue: VecDeque::new(),
            max_entries,
        }
    }

    pub fn insert(&mut self, s: String) {
        if self.set.contains(&s) {
            return;
        }
        if self.queue.len() >= self.max_entries {
            if let Some(removed) = self.queue.pop_back() {
                self.set.remove(&removed);
            }
        }
        self.queue.push_front(s.clone());
        self.set.insert(s);
    }

    pub fn contains(&self, s: &str) -> bool {
        return self.set.contains(s);
    }
}

/// When full, every insert pushes out the oldest entry in the set.
///
/// Used to record last seen message IDs.
pub struct BoundedHashMap<K, V> {
    map: HashMap<K, V>,
    queue: VecDeque<K>,
    max_entries: usize,
}

impl<K: Clone + std::hash::Hash + std::cmp::Eq, V> BoundedHashMap<K, V> {
    pub fn new(max_entries: usize) -> Self {
        BoundedHashMap {
            map: HashMap::new(),
            queue: VecDeque::new(),
            max_entries,
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.queue.len() >= self.max_entries {
            if let Some(removed) = self.queue.pop_back() {
                self.map.remove(&removed);
            }
        }
        if !self.map.contains_key(&key) {
            self.queue.push_front(key.clone());
        }
        self.map.insert(key, value);
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.map.get_mut(key)
    }
}
