use std::slice::{Iter, IterMut};

pub struct VecStack<T>(Vec<T>);

impl<T> VecStack<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn clear(&mut self) {
        self.0.clear()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn top(&self) -> Option<&T> {
        self.0.last()
    }

    pub fn push(&mut self, elem: T) {
        self.0.push(elem)
    }

    pub fn pop(&mut self) -> Option<T> {
        self.0.pop()
    }

    pub fn iter(&self) -> Iter<'_, T> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.0.iter_mut()
    }

    pub fn into_vec(self) -> Vec<T> {
        self.0
    }
}
