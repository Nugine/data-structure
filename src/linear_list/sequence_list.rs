use crate::raw::RawArray;

use std::iter::FusedIterator;
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::ptr::{drop_in_place, NonNull};

pub struct SequenceList<T> {
    raw: RawArray<T>,
    len: usize,
    // invariant: len <= cap <= isize::max_value()
}

unsafe impl<T: Send> Send for SequenceList<T> {}
unsafe impl<T: Sync> Sync for SequenceList<T> {}

impl<T> SequenceList<T> {
    pub fn new(capacity: usize) -> Self {
        let raw = unsafe { RawArray::alloc(capacity) };
        Self { raw, len: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.raw.cap
    }

    pub fn push(&mut self, elem: T) {
        if self.len == self.raw.cap {
            panic!("sequence list is full")
        }

        unsafe { self.raw.offset(self.len).write(elem) };
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;
        Some(unsafe { self.raw.offset(self.len).read() })
    }

    pub fn clear(&mut self) {
        let len = self.len;
        self.len = 0;
        unsafe { drop_in_place(std::slice::from_raw_parts_mut(self.raw.arr.as_ptr(), len)) };
    }

    pub fn insert(&mut self, index: usize, elem: T) {
        if index > self.len {
            panic!("index out of bounds")
        }
        let count = self.len - index;
        unsafe {
            let src = self.raw.offset(index);
            let dst = src.offset(1);
            std::ptr::copy(src, dst, count);
            src.write(elem);
            self.len += 1;
        }
    }

    pub fn remove(&mut self, index: usize) -> T {
        if index >= self.len {
            panic!("index out of bounds")
        }
        let count = self.len - 1 - index;
        unsafe {
            let dst = self.raw.offset(index);
            let elem = dst.read();
            let src = dst.offset(1);
            std::ptr::copy(src, dst, count);
            self.len -= 1;
            elem
        }
    }
}

impl<T> Drop for SequenceList<T> {
    fn drop(&mut self) {
        unsafe {
            drop_in_place(self.raw.as_slice_mut());
            self.raw.dealloc();
        }
    }
}

impl<T> Deref for SequenceList<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe { self.raw.as_slice() }
    }
}

impl<T> DerefMut for SequenceList<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { self.raw.as_slice_mut() }
    }
}

impl<T> Index<usize> for SequenceList<T> {
    type Output = T;
    fn index(&self, idx: usize) -> &T {
        if idx >= self.len {
            panic!("index out of bounds")
        }

        unsafe { &*self.raw.offset(idx) }
    }
}

impl<T> IndexMut<usize> for SequenceList<T> {
    fn index_mut(&mut self, idx: usize) -> &mut T {
        if idx >= self.len {
            panic!("index out of bounds")
        }
        unsafe { &mut *self.raw.offset(idx) }
    }
}

// ----------------------------------------
// begin: IterOwned
pub struct IterOwned<T> {
    raw: RawArray<T>,
    head: NonNull<T>,
    tail: NonNull<T>,
    len: usize,
}

impl<T> Drop for IterOwned<T> {
    fn drop(&mut self) {
        unsafe {
            drop_in_place(std::slice::from_raw_parts_mut(self.head.as_ptr(), self.len));
            self.raw.dealloc()
        }
    }
}

impl<T> IntoIterator for SequenceList<T> {
    type Item = T;
    type IntoIter = IterOwned<T>;
    fn into_iter(self) -> IterOwned<T> {
        let raw = unsafe{ self.raw.shadow_clone() };
        let len = self.len;
        std::mem::forget(self);

        IterOwned {
            head: raw.arr,
            tail: unsafe { NonNull::new_unchecked(raw.arr.as_ptr().offset(len as isize)) },
            raw,
            len,
        }
    }
}

impl<T> Iterator for IterOwned<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                let ptr = self.head;
                self.head = NonNull::new_unchecked(ptr.as_ptr().offset(1));
                self.len -= 1;
                Some(ptr.as_ptr().read())
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T> DoubleEndedIterator for IterOwned<T> {
    fn next_back(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                let ptr = self.tail;
                self.tail = NonNull::new_unchecked(ptr.as_ptr().offset(-1));
                self.len -= 1;
                Some(ptr.as_ptr().read())
            }
        }
    }
}

impl<T> ExactSizeIterator for IterOwned<T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<T> FusedIterator for IterOwned<T> {}

// end: IterOwned
// ----------------------------------------

#[cfg(test)]
mod test {
    use super::SequenceList;

    #[test]
    fn test_sequence_list() {
        #[derive(Debug, PartialEq, Eq)]
        struct Foo(i32);

        impl Drop for Foo {
            fn drop(&mut self) {
                dbg!(format!("drop {:?}", self));
            }
        }

        let mut list = <SequenceList<Foo>>::new(10);
        assert!(list.is_empty());
        assert_eq!(list.capacity(), 10);

        list.push(Foo(1));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, 1);

        list.push(Foo(2));
        assert_eq!(list[1].0, 2);

        list.insert(1, Foo(3));
        assert_eq!(list[1].0, 3);
        assert_eq!(list[2].0, 2);
        // [1, 3, 2]

        assert_eq!(list.remove(1).0, 3); // drop 3
        assert_eq!(list.pop().unwrap().0, 2); // drop 2

        list.clear(); // drop [1]
        list.push(Foo(3));

        drop(list); // drop [3]
    }
}
