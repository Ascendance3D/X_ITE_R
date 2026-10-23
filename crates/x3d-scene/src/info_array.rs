//! Ordered name → value map with the exact semantics of src/x_ite/Base/X3DInfoArray.js
//! (`add`, `update`, `remove`), which decide the output order of named nodes and protos.

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct InfoArray<T: Copy + PartialEq> {
    array: Vec<T>,
    index: HashMap<String, T>,
}

impl<T: Copy + PartialEq> Default for InfoArray<T> {
    fn default() -> Self {
        Self {
            array: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl<T: Copy + PartialEq> InfoArray<T> {
    pub fn has(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<T> {
        self.index.get(key).copied()
    }

    pub fn values(&self) -> &[T] {
        &self.array
    }

    /// Rewrite every value (used when scenes are merged and ids shift).
    pub fn map_values(&mut self, f: impl Fn(T) -> T) {
        for v in &mut self.array {
            *v = f(*v);
        }

        for v in self.index.values_mut() {
            *v = f(*v);
        }
    }

    pub fn len(&self) -> usize {
        self.array.len()
    }

    pub fn is_empty(&self) -> bool {
        self.array.is_empty()
    }

    /// X3DInfoArray.add: errors if the key exists.
    pub fn add(&mut self, key: &str, value: T) -> Result<(), String> {
        if self.index.contains_key(key) {
            return Err(format!("key '{key}' already exists"));
        }

        self.array.push(value);
        self.index.insert(key.to_string(), value);
        Ok(())
    }

    /// X3DInfoArray.update.
    pub fn update(&mut self, old_key: &str, new_key: &str, value: T) {
        let old_value = self.index.get(old_key).copied();

        if old_key != new_key {
            self.remove(new_key);
        }

        self.index.remove(old_key);
        self.index.insert(new_key.to_string(), value);

        match old_value {
            Some(old) => {
                if let Some(i) = self.array.iter().position(|v| *v == old) {
                    self.array[i] = value;
                }
            }
            None => self.array.push(value),
        }
    }

    /// X3DInfoArray.remove.
    pub fn remove(&mut self, key: &str) {
        let Some(value) = self.index.get(key).copied() else {
            return;
        };

        let i = self.array.iter().position(|v| *v == value);

        self.index.remove(key);

        if let Some(i) = i {
            self.array.remove(i);
        }
    }
}

/// src/x_ite/Execution/NamedNodesHandling.js: getUniqueName.
pub fn get_unique_name(has: impl Fn(&str) -> bool, name: &str) -> String {
    if !name.is_empty() && !has(name) {
        return name.to_string();
    }

    // name .replace (/_\d+$/, "")
    let mut base = name.to_string();
    let bytes = base.as_bytes();
    let mut end = bytes.len();

    while end > 0 && bytes[end - 1].is_ascii_digit() {
        end -= 1;
    }

    if end < bytes.len() && end > 0 && bytes[end - 1] == b'_' {
        base.truncate(end - 1);
    }

    if !base.is_empty() && !has(&base) {
        return base;
    }

    let mut new_name;
    let mut lo: u32;
    let mut hi: u32 = 1;

    loop {
        lo = hi;
        hi <<= 1;

        new_name = format!("{base}_{lo}");

        if !has(&new_name) {
            break;
        }
    }

    lo >>= 1;
    hi >>= 1;

    if lo != 0 && hi != 0 {
        while lo < hi {
            let m = (lo + hi) >> 1;

            if has(&format!("{base}_{m}")) {
                lo = m + 1;
            } else {
                hi = m;
            }
        }

        new_name = format!("{base}_{lo}");
    }

    new_name
}
