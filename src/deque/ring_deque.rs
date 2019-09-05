use crate::raw::RawArray;

use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::ptr::drop_in_place;
use std::ptr::NonNull;

pub struct RingDeque<T> {
    raw: RawArray<T>,
    head: usize,
    tail: usize,
    len: usize,
}

unsafe impl<T: Send> Send for RingDeque<T> {}
unsafe impl<T: Sync> Sync for RingDeque<T> {}

impl<T> RingDeque<T> {
    pub fn new(capacity: usize) -> Self {
        let raw = unsafe { RawArray::alloc(capacity) };
        Self {
            raw,
            head: 0,
            tail: 0,
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == self.raw.cap
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.raw.cap
    }

    pub fn clear(&mut self) {
        if self.is_empty() {
            return;
        }
        if self.head <= self.tail {
            unsafe {
                drop_in_place(std::slice::from_raw_parts_mut(
                    self.raw.offset(self.head),
                    self.len,
                ));
            }
        } else if self.head > self.tail {
            unsafe {
                drop_in_place(std::slice::from_raw_parts_mut(
                    self.raw.offset(self.head),
                    self.raw.cap - self.head,
                ));
                drop_in_place(std::slice::from_raw_parts_mut(
                    self.raw.arr.as_ptr(),
                    self.tail,
                ));
            }
        }
        self.head = 0;
        self.tail = 0;
        self.len = 0;
    }

    pub fn push_back(&mut self, elem: T) {
        if self.is_full() {
            panic!("ring deque is full")
        }

        unsafe {
            let ptr = self.raw.offset(self.tail);
            ptr.write(elem);
        };
        self.tail = (self.tail + 1) % self.raw.cap;
        self.len += 1;
    }

    pub fn push_front(&mut self, elem: T) {
        self.head = (self.head + self.raw.cap - 1) % self.raw.cap;
        unsafe {
            let ptr = self.raw.offset(self.head);
            ptr.write(elem);
        }
        self.len += 1;
    }

    pub fn pop_back(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            self.tail = (self.tail + self.raw.cap - 1) % self.raw.cap;
            self.len -= 1;
            unsafe {
                let ptr = self.raw.offset(self.tail);
                Some(ptr.read())
            }
        }
    }

    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            let elem = unsafe {
                let ptr = self.raw.offset(self.head);
                ptr.read()
            };
            self.head = (self.head + 1) % self.raw.cap;
            self.len -= 1;
            Some(elem)
        }
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            iter: RawPtrIter::from_ring_deque(self),
            _marker: PhantomData,
        }
    }

    pub fn iter_mut(&self) -> IterMut<'_, T> {
        IterMut {
            iter: RawPtrIter::from_ring_deque(self),
            _marker: PhantomData,
        }
    }
}

impl<T> Drop for RingDeque<T> {
    fn drop(&mut self) {
        self.clear();
        unsafe { self.raw.dealloc() }
    }
}

// ------------------------------------
// begin: IterOwned

pub struct IterOwned<T>(RingDeque<T>);

impl<T> Iterator for IterOwned<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.0.pop_front()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.0.len, Some(self.0.len))
    }
}

impl<T> IntoIterator for RingDeque<T> {
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
// ------------------------------------

pub struct RawPtrIter<T> {
    raw: RawArray<T>,
    head: usize,
    tail: usize,
    len: usize,
}

impl<T> RawPtrIter<T> {
    fn from_ring_deque(rd: &RingDeque<T>) -> Self {
        Self {
            raw: unsafe { rd.raw.shadow_clone() },
            head: rd.head,
            tail: rd.tail,
            len: rd.len,
        }
    }

    fn next_front(&mut self) -> Option<NonNull<T>> {
        if self.len == 0 {
            None
        } else {
            let ptr = unsafe { NonNull::new_unchecked(self.raw.offset(self.head)) };
            self.head = (self.head + 1) % self.raw.cap;
            self.len -= 1;
            Some(ptr)
        }
    }

    fn next_back(&mut self) -> Option<NonNull<T>> {
        if self.len == 0 {
            None
        } else {
            self.tail = (self.tail + self.raw.cap - 1) % self.raw.cap;
            let ptr = unsafe { NonNull::new_unchecked(self.raw.offset(self.tail)) };
            self.len -= 1;
            Some(ptr)
        }
    }
}

// ------------------------------------
// begin: Iter

pub struct Iter<'a, T> {
    iter: RawPtrIter<T>,
    _marker: PhantomData<&'a RingDeque<T>>,
}

unsafe impl<T: Sync> Send for Iter<'_, T> {}
unsafe impl<T: Sync> Sync for Iter<'_, T> {}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<&'a T> {
        self.iter.next_front().map(|ptr| unsafe { &*ptr.as_ptr() })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.iter.len, Some(self.iter.len))
    }
}

impl<'a, T> IntoIterator for &'a RingDeque<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<&'a T> {
        self.iter.next_back().map(|ptr| unsafe { &*ptr.as_ptr() })
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.iter.len
    }
}

impl<'a, T> FusedIterator for Iter<'a, T> {}

// end: Iter
// ------------------------------------

// ------------------------------------
// begin: IterMut

pub struct IterMut<'a, T> {
    iter: RawPtrIter<T>,
    _marker: PhantomData<&'a mut RingDeque<T>>,
}

unsafe impl<T: Send> Send for IterMut<'_, T> {}
unsafe impl<T: Sync> Sync for IterMut<'_, T> {}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<&'a mut T> {
        self.iter
            .next_front()
            .map(|ptr| unsafe { &mut *ptr.as_ptr() })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.iter.len, Some(self.iter.len))
    }
}

impl<'a, T> IntoIterator for &'a mut RingDeque<T> {
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
            .map(|ptr| unsafe { &mut *ptr.as_ptr() })
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.iter.len
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}
// end: IterMut
// ------------------------------------

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ring_deque() {
        #[derive(Debug, PartialEq, Eq)]
        struct Foo(i32);

        impl Drop for Foo {
            fn drop(&mut self) {
                dbg!(format!("drop {:?}", self));
            }
        }
        let mut dq = <RingDeque<Foo>>::new(3);
        assert!(dq.is_empty());
        dq.push_back(Foo(1));
        dq.pop_front();
        dq.push_back(Foo(2));
        dq.pop_front();

        dq.push_back(Foo(3));
        dq.clear();

        for i in 4..6 {
            dq.push_front(Foo(i));
        }

        drop(dq);
    }
}
