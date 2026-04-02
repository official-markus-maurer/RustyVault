use std::sync::Mutex;

pub struct ByteSortedList<TStore, TInput> {
    buckets: [Mutex<Vec<TStore>>; 256],
    get_bucket: fn(&TInput) -> u8,
    compare: fn(&TInput, &TStore) -> i32,
    new_item: fn(&TInput) -> TStore,
    merge: fn(&TInput, &mut TStore),
}

impl<TStore, TInput> ByteSortedList<TStore, TInput> {
    pub fn new(
        get_bucket: fn(&TInput) -> u8,
        compare: fn(&TInput, &TStore) -> i32,
        new_item: fn(&TInput) -> TStore,
        merge: fn(&TInput, &mut TStore),
    ) -> Self {
        Self {
            buckets: std::array::from_fn(|_| Mutex::new(Vec::new())),
            get_bucket,
            compare,
            new_item,
            merge,
        }
    }

    pub fn find(&self, value: &TInput) -> Option<TStore>
    where
        TStore: Clone,
    {
        let bucket = (self.get_bucket)(value) as usize;
        let list_guard = self.buckets[bucket].lock().unwrap_or_else(|e| e.into_inner());
        let (found, index) = self.search_on(value, &list_guard);
        if found == 0 {
            Some(list_guard[index].clone())
        } else {
            None
        }
    }

    pub fn add_find(&self, value: &TInput) {
        let bucket = (self.get_bucket)(value) as usize;
        let mut list_guard = self.buckets[bucket].lock().unwrap_or_else(|e| e.into_inner());
        let (found, index) = self.search_on(value, &list_guard);
        if found == 0 {
            (self.merge)(value, &mut list_guard[index]);
            return;
        }
        list_guard.insert(index, (self.new_item)(value));
    }

    pub fn add_find_with_exact(&self, value: &TInput, exact: fn(&TInput, &TStore) -> bool) {
        let bucket = (self.get_bucket)(value) as usize;
        let mut list_guard = self.buckets[bucket].lock().unwrap_or_else(|e| e.into_inner());
        let (found, mut index) = self.search_on(value, &list_guard);
        if found == 0 {
            let top = list_guard.len();
            while index < top {
                let int_res = (self.compare)(value, &list_guard[index]);
                if int_res != 0 {
                    break;
                }
                if exact(value, &list_guard[index]) {
                    (self.merge)(value, &mut list_guard[index]);
                    return;
                }
                index += 1;
            }
        }
        list_guard.insert(index, (self.new_item)(value));
    }

    pub fn count(&self) -> usize {
        self.buckets
            .iter()
            .map(|b| b.lock().unwrap_or_else(|e| e.into_inner()).len())
            .sum()
    }

    pub fn to_vec(&self) -> Vec<TStore>
    where
        TStore: Clone,
    {
        let mut out = Vec::with_capacity(self.count());
        for bucket in &self.buckets {
            let list_guard = bucket.lock().unwrap_or_else(|e| e.into_inner());
            out.extend(list_guard.iter().cloned());
        }
        out
    }

    fn search_on(&self, search_value: &TInput, search_list: &[TStore]) -> (i32, usize) {
        let mut bottom = 0usize;
        let mut top = search_list.len();
        let mut mid = 0usize;
        let mut res = -1i32;

        while bottom < top && res != 0 {
            mid = (bottom + top) / 2;
            res = (self.compare)(search_value, &search_list[mid]);
            if res < 0 {
                top = mid;
            } else if res > 0 {
                bottom = mid + 1;
            }
        }
        let mut index = mid;

        if res == 0 {
            while index > 0 {
                let res1 = (self.compare)(search_value, &search_list[index - 1]);
                if res1 == 0 {
                    index -= 1;
                } else {
                    break;
                }
            }
        } else if res > 0 {
            index += 1;
        }

        (res, index)
    }
}

