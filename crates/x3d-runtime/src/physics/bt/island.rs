//! btUnionFind and the unstable quick sort of btAlignedObjectArray (Bullet 2.82, zlib license).
//! The sort order of equal keys is observable (island and manifold order), so the algorithm is
//! reproduced exactly.

/// btAlignedObjectArray::quickSort.
pub fn quick_sort<T: Clone>(data: &mut [T], less: &impl Fn(&T, &T) -> bool) {
    // don't sort 0 or 1 elements
    if data.len() > 1 {
        quick_sort_internal(data, less, 0, data.len() as i32 - 1);
    }
}

fn quick_sort_internal<T: Clone>(data: &mut [T], less: &impl Fn(&T, &T) -> bool, lo: i32, hi: i32) {
    let (mut i, mut j) = (lo, hi);
    let x = data[((lo + hi) / 2) as usize].clone();

    // partition
    loop {
        while less(&data[i as usize], &x) {
            i += 1;
        }
        while less(&x, &data[j as usize]) {
            j -= 1;
        }
        if i <= j {
            data.swap(i as usize, j as usize);
            i += 1;
            j -= 1;
        }
        if i > j {
            break;
        }
    }

    // recursion
    if lo < j {
        quick_sort_internal(data, less, lo, j);
    }
    if i < hi {
        quick_sort_internal(data, less, i, hi);
    }
}

/// btElement.
#[derive(Clone, Copy, Debug, Default)]
pub struct Element {
    pub id: i32,
    pub sz: i32,
}

/// btUnionFind (USE_PATH_COMPRESSION, STATIC_SIMULATION_ISLAND_OPTIMIZATION).
#[derive(Clone, Debug, Default)]
pub struct UnionFind {
    pub elements: Vec<Element>,
}

impl UnionFind {
    pub fn reset(&mut self, n: usize) {
        self.elements = (0..n).map(|i| Element { id: i as i32, sz: 1 }).collect();
    }

    pub fn find(&mut self, mut x: i32) -> i32 {
        while x != self.elements[x as usize].id {
            // path halving
            let parent = self.elements[x as usize].id;
            let grand = self.elements[parent as usize].id;
            self.elements[x as usize].id = grand;
            x = grand;
        }
        x
    }

    pub fn unite(&mut self, p: i32, q: i32) {
        let i = self.find(p);
        let j = self.find(q);

        if i == j {
            return;
        }

        self.elements[i as usize].id = j;
        self.elements[j as usize].sz += self.elements[i as usize].sz;
    }

    /// sortIslands.
    pub fn sort_islands(&mut self) {
        for i in 0..self.elements.len() {
            let r = self.find(i as i32);
            self.elements[i].id = r;
        }
        quick_sort(&mut self.elements, &|a: &Element, b: &Element| a.id < b.id);
    }
}
