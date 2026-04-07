use crate::object::NiObject;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GcRef {
    pub index: u32,
    pub generation: u32,
}

struct GcSlot {
    object: NiObject,
    generation: u32,
    marked: bool,
}

pub struct GcHeap {
    slots: Vec<Option<GcSlot>>,
    generations: Vec<u32>,
    free_list: Vec<usize>,
    object_count: usize,
    bytes_allocated: usize,
    threshold: usize,
    growth_factor: f64,
}

impl Default for GcHeap {
    fn default() -> Self {
        Self::new()
    }
}

impl GcHeap {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            generations: Vec::new(),
            free_list: Vec::new(),
            object_count: 0,
            bytes_allocated: 0,
            threshold: 256,
            growth_factor: 2.0,
        }
    }

    pub fn alloc(&mut self, object: NiObject) -> GcRef {
        self.object_count += 1;
        let size = object.size_bytes();
        self.bytes_allocated += size;

        if let Some(index) = self.free_list.pop() {
            self.generations[index] += 1;
            let generation = self.generations[index];
            self.slots[index] = Some(GcSlot {
                object,
                generation,
                marked: false,
            });
            GcRef {
                index: index as u32,
                generation,
            }
        } else {
            let index = self.slots.len();
            self.generations.push(1);
            self.slots.push(Some(GcSlot {
                object,
                generation: 1,
                marked: false,
            }));
            GcRef {
                index: index as u32,
                generation: 1,
            }
        }
    }

    pub fn get(&self, gc_ref: GcRef) -> Option<&NiObject> {
        self.slots
            .get(gc_ref.index as usize)
            .and_then(|slot| slot.as_ref())
            .filter(|slot| slot.generation == gc_ref.generation)
            .map(|slot| &slot.object)
    }

    pub fn get_mut(&mut self, gc_ref: GcRef) -> Option<&mut NiObject> {
        self.slots
            .get_mut(gc_ref.index as usize)
            .and_then(|slot| slot.as_mut())
            .filter(|slot| slot.generation == gc_ref.generation)
            .map(|slot| &mut slot.object)
    }

    pub fn should_collect(&self) -> bool {
        self.object_count >= self.threshold
    }

    pub fn mark(&mut self, gc_ref: GcRef) {
        let mut worklist = vec![gc_ref];
        while let Some(r) = worklist.pop() {
            if let Some(Some(slot)) = self.slots.get_mut(r.index as usize) {
                if slot.generation == r.generation && !slot.marked {
                    slot.marked = true;
                    let refs = slot.object.references();
                    worklist.extend(refs);
                }
            }
        }
    }

    pub fn sweep(&mut self) {
        for i in 0..self.slots.len() {
            if let Some(slot) = &self.slots[i] {
                if !slot.marked {
                    self.bytes_allocated = self.bytes_allocated.saturating_sub(slot.object.size_bytes());
                    self.slots[i] = None;
                    self.free_list.push(i);
                    self.object_count -= 1;
                } else if let Some(slot) = &mut self.slots[i] {
                    slot.marked = false;
                }
            }
        }
        self.threshold = ((self.object_count as f64 * self.growth_factor) as usize).max(256);
    }

    pub fn object_count(&self) -> usize {
        self.object_count
    }

    pub fn bytes_allocated(&self) -> usize {
        self.bytes_allocated
    }

    pub fn set_threshold(&mut self, threshold: usize) {
        self.threshold = threshold;
    }

    pub fn threshold(&self) -> usize {
        self.threshold
    }

    pub fn set_growth_factor(&mut self, factor: f64) {
        self.growth_factor = factor;
    }
}
