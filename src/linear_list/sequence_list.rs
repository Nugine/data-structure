use std::alloc::Layout;
use std::iter::FusedIterator;
use std::mem::{align_of, size_of};
use std::ops::{Deref, DerefMut, Index, IndexMut};
use std::ptr::NonNull;

pub struct SequenceList<T> {
    arr: NonNull<T>,
    len: usize,
    cap: usize,
    // invariant: len <= cap <= isize::max_value()
}

unsafe impl<T: Send> Send for SequenceList<T> {}
unsafe impl<T: Sync> Sync for SequenceList<T> {}

impl<T> SequenceList<T> {
    unsafe fn offset(&self, offset: isize) -> *mut T {
        self.arr.as_ptr().offset(offset)
    }
}

impl<T> SequenceList<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(size_of::<T>() != 0);

        if capacity == 0 {
            return Self {
                arr: NonNull::dangling(),
                len: 0,
                cap: 0,
            };
        }

        let alloc_size = capacity
            .checked_mul(size_of::<T>())
            .and_then(|size| {
                if size > isize::max_value() as usize {
                    None
                } else {
                    Some(size)
                }
            })
            .expect("capacity overflow");
        let layout = Layout::from_size_align(alloc_size, align_of::<T>()).expect("layout error");

        let arr = unsafe {
            let ptr = std::alloc::alloc(layout.clone()) as *mut T;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            NonNull::new_unchecked(ptr)
        };

        Self {
            arr,
            len: 0,
            cap: capacity,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.cap
    }
    pub fn push(&mut self, elem: T) {
        if self.len == self.cap {
            panic!("sequence list is full")
        }

        unsafe { self.offset(self.len as isize).write(elem) };
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }

        self.len -= 1;
        Some(unsafe { self.offset(self.len as isize).read() })
    }

    pub fn clear(&mut self) {
        let len = self.len;
        self.len = 0;
        unsafe { std::ptr::drop_in_place(std::slice::from_raw_parts_mut(self.arr.as_ptr(), len)) };
    }

    pub fn insert(&mut self, index: usize, elem: T) {
        if index > self.len {
            panic!("index out of bounds")
        }
        let count = self.len - index;
        unsafe {
            let src = self.offset(index as isize);
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
            let dst = self.offset(index as isize);
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
            std::ptr::drop_in_place(self.deref_mut());
            let layout =
                Layout::from_size_align_unchecked(self.cap * size_of::<T>(), align_of::<T>());
            std::alloc::dealloc(self.arr.as_ptr() as *mut u8, layout)
        }
    }
}

impl<T> Deref for SequenceList<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.arr.as_ptr(), self.len) }
    }
}

impl<T> DerefMut for SequenceList<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.arr.as_ptr(), self.len) }
    }
}

impl<T> Index<usize> for SequenceList<T> {
    type Output = T;
    fn index(&self, idx: usize) -> &T {
        if idx >= self.len {
            panic!("index out of bounds")
        }

        unsafe { &*self.offset(idx as isize) }
    }
}

impl<T> IndexMut<usize> for SequenceList<T> {
    fn index_mut(&mut self, idx: usize) -> &mut T {
        if idx >= self.len {
            panic!("index out of bounds")
        }
        unsafe { &mut *self.offset(idx as isize) }
    }
}

// ----------------------------------------
// begin: IterOwned
pub struct IterOwned<T> {
    arr: NonNull<T>,
    head: NonNull<T>,
    tail: NonNull<T>,
    len: usize,
    cap: usize,
}

impl<T> Drop for IterOwned<T> {
    fn drop(&mut self) {
        unsafe {
            std::ptr::drop_in_place(std::slice::from_raw_parts_mut(self.head.as_ptr(), self.len));
            let layout =
                Layout::from_size_align_unchecked(self.cap * size_of::<T>(), align_of::<T>());
            std::alloc::dealloc(self.arr.as_ptr() as *mut u8, layout);
        }
    }
}

impl<T> IntoIterator for SequenceList<T> {
    type Item = T;
    type IntoIter = IterOwned<T>;
    fn into_iter(self) -> IterOwned<T> {
        IterOwned {
            arr: self.arr,
            head: self.arr,
            tail: unsafe { NonNull::new_unchecked(self.offset(self.len as isize)) },
            len: self.len,
            cap: self.cap,
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

        assert_eq!(list.remove(1).0, 3);
        assert_eq!(list.pop().unwrap().0, 2);

        list.clear();
        list.push(Foo(3));

        drop(list);
    }
}
