use crate::linear_list::LinkedList;

pub struct LinkedQueue<T>(LinkedList<T>);

impl<T> LinkedQueue<T> {
    pub fn new() -> Self {
        Self(LinkedList::new())
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

    pub fn front(&self) -> Option<&T> {
        self.0.front()
    }

    pub fn push(&mut self, elem: T) {
        self.0.push_back(elem)
    }

    pub fn pop(&mut self) -> Option<T> {
        self.0.pop_front()
    }

    pub fn into_linked_list(self) -> LinkedList<T> {
        self.0
    }
}
