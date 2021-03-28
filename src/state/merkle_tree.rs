// https://github1s.com/Fluidex/circuits/blob/HEAD/helper.ts/binary_merkle_tree.ts

use fnv::FnvHashMap;
use rayon::prelude::*;
use std::iter;

pub use ff::{Field, PrimeField};

use super::types::{hash, Fr};

type LeafIndex = u32;
type LeafType = Fr;
type ValueMap = FnvHashMap<LeafIndex, LeafType>;

pub struct MerkleProofN<const LENGTH: usize> {
    pub root: LeafType,
    pub leaf: LeafType,
    pub path_elements: Vec<[LeafType; LENGTH]>,
}
type MerkleProof = MerkleProofN<1>;
#[derive(Debug)]
struct HashCacheItemN<const LENGTH: usize> {
    inputs: [LeafType; LENGTH],
    result: LeafType,
}
type HashCacheItem = HashCacheItemN<2>;

// TODO: use leaf_index/leaf_type as generics
pub struct Tree {
    pub height: usize,
    // precalculate mid hashes, so we don't have to store the empty nodes
    default_nodes: Vec<LeafType>,

    // In `data`, we only store the nodes with non empty values
    // data[0] is leaf nodes, and data[-1] is root
    // the `logical size` of data[0] is of size 2**height
    data: Vec<ValueMap>,
}

impl Tree {
    pub fn new(height: usize, default_leaf_node_value: LeafType) -> Self {
        // check overflow
        let _ = 2u32.checked_pow(height as u32).expect("tree depth error, overflow");
        // 2**height leaves, and the total height of the tree is
        //self.height = height;
        let mut default_nodes = vec![default_leaf_node_value];
        for i in 0..height {
            default_nodes.push(hash(&[default_nodes[i], default_nodes[i]]));
        }
        let data = iter::repeat_with(ValueMap::default).take(height + 1).collect();
        Self {
            height,
            default_nodes,
            data,
        }
    }
    pub fn max_leaf_num(&self) -> u32 {
        2u32.checked_pow(self.height as u32).unwrap()
    }
    /*
    pub fn print(dense = true, empty_label = 'None') {
      console.log(`Tree(height: ${self.height}, leaf_num: ${Math.pow(2, self.height)}, non_empty_leaf_num: ${self.data[0].size})`);
      if (dense) {
        for (let i = 0; i < self.data.length; i++) {
          process.stdout.write(i == 0 ? 'Leaves\t' : `Mid(${i})\t`);
          for (let j = 0; j < Math.pow(2, self.height - i); j++) {
            process.stdout.write(self.data[i].has(big_int(j)) ? self.data[i].get(big_int(j)).to_string() : empty_label);
            process.stdout.write(',');
          }
          process.stdout.write('\n');
        }
      } else {
        for (let i = 0; i < self.data.length; i++) {
          process.stdout.write(i == 0 ? 'Leaves\t' : `Mid(${i})\t`);
          for (let [k, v] of self.data[i].entries()) {
            process.stdout.write(`${k}:${v},`);
          }
          process.stdout.write('\n');
        }
      }
    }
    */
    pub fn sibling_idx(&self, n: LeafIndex) -> LeafIndex {
        if n % 2 == 1 {
            n - 1
        } else {
            n + 1
        }
    }
    pub fn parent_idx(&self, n: LeafIndex) -> LeafIndex {
        n >> 1
    }
    pub fn get_value(&self, level: usize, idx: u32) -> LeafType {
        *self.data[level].get(&idx).unwrap_or(&self.default_nodes[level])
    }
    pub fn get_leaf(&self, idx: u32) -> LeafType {
        self.get_value(0, idx)
    }
    fn recalculate_parent(&mut self, level: usize, idx: u32) {
        let lhs = self.get_value(level - 1, idx * 2);
        let rhs = self.get_value(level - 1, idx * 2 + 1);
        let new_hash = hash(&[lhs, rhs]);
        self.data[level].insert(idx, new_hash);
    }
    pub fn set_value(&mut self, idx: u32, value: LeafType) {
        let mut idx = idx;
        if self.get_leaf(idx) == value {
            return;
        }
        if idx >= self.max_leaf_num() {
            panic!("invalid tree idx {}", idx);
        }
        self.data[0].insert(idx, value);
        for i in 1..=self.height {
            idx = self.parent_idx(idx);
            self.recalculate_parent(i, idx);
        }
    }
    // of course there is no such thing 'parallel' in Js
    // self function is only used as pseudo code for future Rust rewrite
    // TODO: change updates into something like Into<ParIter> ...
    pub fn set_value_parallel(&mut self, updates: &[(u32, LeafType)], parallel: usize) {
        let mut parallel = parallel;
        if parallel == 0 {
            parallel = 8; // TODO: a better default
        }
        for chunk in updates.chunks(parallel) {
            let diffs: Vec<Vec<HashCacheItem>> = chunk
                .par_iter() // iterating over i32
                .map(|(idx, value)| self.set_value_prepare_diff(*idx, *value))
                .collect();
            let chunk_vec: Vec<(u32, LeafType)> = chunk.to_vec();
            for ((idx, value), cache) in chunk_vec.into_iter().zip(diffs.into_iter()) {
                self.set_value_apply_diff(idx, value, cache)
            }
        }
    }
    fn set_value_prepare_diff(&self, idx: u32, value: LeafType) -> Vec<HashCacheItem> {
        // the precalculating can be done parallelly
        let mut precalculated = Vec::<HashCacheItem>::default();
        let mut cur_idx = idx;
        let mut cur_value = value;
        for i in 0..self.height {
            let pair = if cur_idx % 2 == 0 {
                [cur_value, self.get_value(i, cur_idx + 1)]
            } else {
                [self.get_value(i, cur_idx - 1), cur_value]
            };
            cur_value = hash(&pair);
            cur_idx = self.parent_idx(cur_idx);
            let cache_item = HashCacheItem {
                inputs: pair,
                result: cur_value,
            };
            precalculated.push(cache_item);
        }
        precalculated
    }
    fn set_value_apply_diff(&mut self, idx: u32, value: LeafType, precalculated: Vec<HashCacheItem>) {
        // apply the precalculated
        let mut cache_miss = false;
        let mut cur_idx = idx;
        //cur_value = value;
        self.data[0].insert(idx, value);
        //let cache_size = precalculated.len();
        //let mut cache_hit_count = 0;
        for i in 0..self.height {
            let pair = if cur_idx % 2 == 0 {
                [self.get_value(i, cur_idx), self.get_value(i, cur_idx + 1)]
            } else {
                [self.get_value(i, cur_idx - 1), self.get_value(i, cur_idx)]
            };
            cur_idx = self.parent_idx(cur_idx);
            if !cache_miss {
                // TODO: is the `cache_miss` shortcut really needed? comparing bigint is quite cheap compared to hash
                // `cache_miss` makes codes more difficult to read
                if precalculated[i].inputs[0] != pair[0] || precalculated[i].inputs[1] != pair[1] {
                    // Due to self is a merkle tree, future caches will all be missed.
                    // precalculated becomes totally useless now
                    cache_miss = true;
                    //precalculated.clear();
                }
            }
            if cache_miss {
                self.data[i + 1].insert(cur_idx, hash(&pair));
            } else {
                self.data[i + 1].insert(cur_idx, precalculated[i].result);
                //cache_hit_count += 1;
            }
        }
        //println!("cache hit {}/{}", cache_hit_count, cache_size);
    }
    pub fn fill_with_leaves_vec(&mut self, leaves: &[LeafType]) {
        if leaves.len() != self.max_leaf_num() as usize {
            panic!("invalid leaves size {}", leaves.len());
        }
        // TODO: optimize here
        for (i, item) in leaves.iter().enumerate() {
            self.set_value(i as u32, *item);
        }
    }
    pub fn fill_with_leaves_map(&mut self, leaves: std::collections::HashMap<LeafIndex, LeafType>) {
        for (k, v) in leaves.iter() {
            self.set_value(*k, *v);
        }
    }
    pub fn get_root(&self) -> LeafType {
        self.get_value(self.data.len() - 1, 0)
    }
    pub fn get_proof(&self, index: u32) -> MerkleProof {
        let mut index = index;
        let leaf = self.get_leaf(index);
        let mut path_elements = Vec::new();
        for i in 0..self.height {
            path_elements.push([self.get_value(i, self.sibling_idx(index))]);
            index = self.parent_idx(index);
        }
        MerkleProof {
            root: self.get_root(),
            path_elements,
            leaf,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::time::Instant;
    //use test::Bencher;
    #[test]
    #[ignore]
    fn bench_tree() {
        let h = 20;
        let mut tree = Tree::new(h, Fr::zero());
        for i in 0..100 {
            let start = Instant::now();
            let inner_count = 100;
            for j in 0..inner_count {
                if j % 100 == 0 {
                    println!("progress {} {}", i, j);
                }
                tree.set_value(j, Fr::from_str(&format!("{}", j + i)).unwrap());
            }
            // 2021.03.15(Apple M1): typescript: 100 ops takes 4934ms
            // 2021.03.26(Apple M1): rust:       100 ops takes 1160ms
            println!("{} ops takes {}ms", inner_count, start.elapsed().as_millis());
        }
    }

    #[test]
    fn test_parallel_update() {
        let h = 20;
        // RAYON_NUM_THREADS can change threads num used
        let mut tree1 = Tree::new(h, Fr::zero());

        let mut tree2 = Tree::new(h, Fr::zero());
        let count = 100;
        let mut updates = Vec::new();
        let rand_elem = || {
            let mut rng = rand::thread_rng();
            Fr::from_str(&format!("{}", rng.gen_range(0..123456789))).unwrap()
        };
        let rand_idx = || {
            let mut rng = rand::thread_rng();
            rng.gen_range(0..2u32.pow(20u32))
        };
        for _ in 0..count {
            updates.push((rand_idx(), rand_elem()));
        }
        for (idx, value) in updates.iter() {
            tree1.set_value(*idx, *value);
        }

        tree2.set_value_parallel(&updates, 4);
        assert_eq!(tree1.get_root(), tree2.get_root());
    }

    #[test]
    #[ignore]
    fn bench_tree_parallel() {
        let h = 20;
        // RAYON_NUM_THREADS can change threads num used
        let mut tree = Tree::new(h, Fr::zero());

        for i in 0..100 {
            let start = Instant::now();
            let inner_count = 100;
            let mut same_updates = Vec::new();
            let rand_elem = || {
                let mut rng = rand::thread_rng();
                Fr::from_str(&format!("{}", rng.gen_range(0..123456789))).unwrap()
            };
            let rand_idx = || {
                let mut rng = rand::thread_rng();
                rng.gen_range(0..2u32.pow(20u32))
            };
            for _ in 0..inner_count {
                same_updates.push((i, rand_elem()));
            }
            let mut dense_updates = Vec::new();
            for j in 0..inner_count {
                dense_updates.push((j, rand_elem()));
            }
            let mut sparse_updates = Vec::new();
            for _ in 0..inner_count {
                sparse_updates.push((rand_idx(), rand_elem()));
            }
            tree.set_value_parallel(&sparse_updates, 4);
            // Rescue
            // 2021.03.15(Apple M1): typescript:            100 ops takes 4934ms
            // 2021.03.26(Apple M1): rust:                  100 ops takes 1160ms
            // sparse update: 80-90% cache hit
            // 2021.03.26(Apple M1): rust parallel 1:       100 ops takes 1140ms
            // 2021.03.26(Apple M1): rust parallel 2:       100 ops takes 656ms
            // 2021.03.26(Apple M1): rust parallel 4:       100 ops takes 422ms
            // Poseidon
            // 2021.03.28(Apple M1): rust parallel 4:       100 ops takes 25ms
            println!("{} ops takes {}ms", inner_count, start.elapsed().as_millis());
        }
    }
}
