use std::alloc::Layout;
use std::iter::FromIterator;
use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::ptr::NonNull;

struct Node<T> {
    elem: T,
    prev: NonNull<Node<T>>,
    next: NonNull<Node<T>>,
}

impl<T> Node<T> {
    unsafe fn alloc(elem: T, prev: NonNull<Node<T>>, next: NonNull<Node<T>>) -> NonNull<Self> {
        let layout = Layout::new::<Node<T>>();
        let ptr = std::alloc::alloc(layout) as *mut Node<T>;
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        ptr.write(Self { elem, prev, next });
        NonNull::new_unchecked(ptr)
    }

    unsafe fn dealloc(ptr: NonNull<Self>) {
        let layout = Layout::new::<Node<T>>();
        std::alloc::dealloc(ptr.as_ptr() as *mut u8, layout);
    }

    unsafe fn consume(ptr: NonNull<Self>) -> T {
        let elem = std::ptr::read(&ptr.as_ref().elem);
        Node::dealloc(ptr);
        elem
    }

    // cond: prev is valid and next is valid
    unsafe fn delete(ptr: NonNull<Self>) {
        let mut prev_ptr = ptr.as_ref().prev;
        let mut next_ptr = ptr.as_ref().next;
        prev_ptr.as_mut().next = next_ptr;
        next_ptr.as_mut().prev = prev_ptr;
    }

    unsafe fn insert(elem: T, mut prev: NonNull<Self>, mut next: NonNull<Self>) -> NonNull<Self> {
        let node_ptr = Node::alloc(elem, prev, next);
        prev.as_mut().next = node_ptr;
        next.as_mut().prev = node_ptr;
        node_ptr
    }
}

// Double-linked circular list
pub struct LinkedList<T> {
    head: NonNull<Node<T>>,
    len: usize,
}

unsafe impl<T: Send> Send for LinkedList<T> {}
unsafe impl<T: Sync> Sync for LinkedList<T> {}

impl<T> LinkedList<T> {
    // cond: self.len == 0
    unsafe fn init(&mut self, elem: T) {
        let mut node_ptr = Node::alloc(elem, NonNull::dangling(), NonNull::dangling());
        node_ptr.as_mut().prev = node_ptr;
        node_ptr.as_mut().next = node_ptr;
        self.head = node_ptr;
        self.len = 1;
    }

    // cond: self.len == 1
    unsafe fn deinit(&mut self) -> T {
        let head_ptr = self.head;
        self.len = 0;
        self.head = NonNull::dangling();
        Node::consume(head_ptr)
    }
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        Self {
            head: NonNull::dangling(),
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push_back(&mut self, elem: T) {
        if self.is_empty() {
            unsafe { self.init(elem) };
        } else {
            unsafe {
                let head_ptr = self.head;
                let tail_ptr = head_ptr.as_ref().prev;
                Node::insert(elem, tail_ptr, head_ptr);
            }
            self.len += 1;
        }
    }

    pub fn push_front(&mut self, elem: T) {
        self.push_back(elem);
        self.head = unsafe { self.head.as_ref().prev };
    }

    pub fn pop_back(&mut self) -> Option<T> {
        match self.len {
            0 => None,
            1 => Some(unsafe { self.deinit() }),
            _ => unsafe {
                self.len -= 1;
                let tail_ptr = self.head.as_ref().prev;
                Node::delete(tail_ptr);
                Some(Node::consume(tail_ptr))
            },
        }
    }

    pub fn pop_front(&mut self) -> Option<T> {
        match self.len {
            0 => None,
            1 => Some(unsafe { self.deinit() }),
            _ => unsafe {
                let ptr = self.head;
                self.len -= 1;
                self.head = ptr.as_ref().next;
                Node::delete(ptr);
                Some(Node::consume(ptr))
            },
        }
    }

    pub fn clear(&mut self) {
        let mut ptr = self.head;
        let len = self.len;
        self.head = NonNull::dangling();
        self.len = 0;
        for _ in 0..len {
            unsafe {
                let next = ptr.as_mut().next;
                std::ptr::drop_in_place(&mut ptr.as_mut().elem);
                Node::dealloc(ptr);
                ptr = next;
            }
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            iter: RawNodeIter {
                head: self.head,
                tail: self.head,
                len: self.len,
            },
            _marker: PhantomData,
        }
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            iter: RawNodeIter {
                head: self.head,
                tail: self.head,
                len: self.len,
            },
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        self.clear()
    }
}

impl<T> LinkedList<T>
where
    T: Ord,
{
    pub fn ordered_insert(&mut self, elem: T) {
        if self.is_empty() {
            unsafe { self.init(elem) };
        } else {
            let mut ptr = self.head;
            for _ in 0..self.len {
                let rhs = unsafe { &ptr.as_ref().elem };
                if &elem > rhs {
                    break;
                }
                ptr = unsafe { ptr.as_ref().next };
            }

            unsafe {
                let next_ptr = ptr.as_ref().next;
                let prev_ptr = ptr;
                Node::insert(elem, prev_ptr, next_ptr);
            }
        }
    }
}

impl<T> FromIterator<T> for LinkedList<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut list = Self::new();
        for elem in iter {
            list.push_back(elem);
        }
        list
    }
}

// --------------------------------
// begin: IterOwned

pub struct IterOwned<T>(LinkedList<T>);

impl<T> Iterator for IterOwned<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.0.pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len, Some(self.0.len))
    }
}

impl<T> IntoIterator for LinkedList<T> {
    type Item = T;
    type IntoIter = IterOwned<T>;
    fn into_iter(self) -> IterOwned<T> {
        IterOwned(self)
    }
}

impl<T> DoubleEndedIterator for IterOwned<T> {
    fn next_back(&mut self) -> Option<T> {
        self.0.pop_back()
    }
}

impl<T> ExactSizeIterator for IterOwned<T> {
    fn len(&self) -> usize {
        self.0.len
    }
}

impl<T> FusedIterator for IterOwned<T> {}

// end: IterOwned
// ------------------------------------------

struct RawNodeIter<T> {
    head: NonNull<Node<T>>,
    tail: NonNull<Node<T>>,
    len: usize,
}

impl<T> RawNodeIter<T> {
    fn next_front(&mut self) -> Option<NonNull<Node<T>>> {
        match self.len {
            0 => None,
            1 => {
                let ptr = self.head;
                self.len = 0;
                self.head = NonNull::dangling();
                self.tail = NonNull::dangling();
                Some(ptr)
            }
            _ => unsafe {
                let ptr = self.head;
                self.len -= 1;
                self.head = ptr.as_ref().next;
                Some(ptr)
            },
        }
    }

    fn next_back(&mut self) -> Option<NonNull<Node<T>>> {
        match self.len {
            0 => None,
            1 => {
                let ptr = self.head;
                self.len = 0;
                self.head = NonNull::dangling();
                self.tail = NonNull::dangling();
                Some(ptr)
            }
            _ => unsafe {
                let ptr = self.tail;
                self.len -= 1;
                self.tail = ptr.as_ref().prev;
                Some(ptr)
            },
        }
    }
}

// ------------------------------------------
// begin: IterMut

pub struct IterMut<'a, T> {
    iter: RawNodeIter<T>,
    _marker: PhantomData<&'a mut LinkedList<T>>,
}

unsafe impl<T: Send> Send for IterMut<'_, T> {}
unsafe impl<T: Sync> Sync for IterMut<'_, T> {}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        self.iter
            .next_front()
            .map(|ptr| unsafe { &mut (*ptr.as_ptr()).elem })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.iter.len, Some(self.iter.len))
    }
}

impl<'a, T> IntoIterator for &'a mut LinkedList<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<&'a mut T> {
        self.iter
            .next_back()
            .map(|ptr| unsafe { &mut (*ptr.as_ptr()).elem })
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.iter.len
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}

// end: IterMut
// ------------------------------------------

// ------------------------------------------
// begin: Iter

pub struct Iter<'a, T> {
    iter: RawNodeIter<T>,
    _marker: PhantomData<&'a LinkedList<T>>,
}

unsafe impl<T: Sync> Send for Iter<'_, T> {}
unsafe impl<T: Sync> Sync for Iter<'_, T> {}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        self.iter
            .next_front()
            .map(|ptr| unsafe { &(*ptr.as_ptr()).elem })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.iter.len, Some(self.iter.len))
    }
}

impl<'a, T> IntoIterator for &'a LinkedList<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<&'a T> {
        self.iter
            .next_back()
            .map(|ptr| unsafe { &(*ptr.as_ptr()).elem })
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.iter.len
    }
}

impl<'a, T> FusedIterator for Iter<'a, T> {}

// end: RefIter
// ------------------------------------------

#[cfg(test)]
mod test {
    use super::LinkedList;

    #[test]
    fn test_linked_list() {
        #[derive(Debug, PartialEq, Eq)]
        struct Foo(i32);

        impl Drop for Foo {
            fn drop(&mut self) {
                dbg!(format!("drop {:?}", self));
            }
        }
        let mut list = <LinkedList<Foo>>::new();
        assert!(list.is_empty());

        list.push_back(Foo(2));
        list.push_front(Foo(1));

        list.iter().for_each(|e| {
            dbg!(e);
        });

        list.iter().for_each(|e| {
            dbg!(e);
        });

        assert_eq!(list.pop_front().unwrap().0, 1);
        assert_eq!(list.pop_back().unwrap().0, 2);

        for e in list {
            dbg!(e);
        }

        let mut list = <LinkedList<Foo>>::new();

        for i in 3..=5 {
            list.push_front(Foo(i));
        }

        list.clear();

        for i in 6..=9 {
            list.push_back(Foo(i));
        }
        drop(list);
    }
}
