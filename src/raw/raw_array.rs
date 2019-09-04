use std::alloc::Layout;
use std::mem::{align_of, size_of};
use std::ptr::NonNull;

pub struct RawArray<T> {
    pub arr: NonNull<T>,
    pub cap: usize,
}


impl<T> RawArray<T> {
    pub unsafe fn alloc(capacity: usize) -> Self {
        assert!(size_of::<T>() != 0);

        if capacity == 0 {
            return Self {
                arr: NonNull::dangling(),
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

        let arr = {
            let ptr = std::alloc::alloc(layout) as *mut T;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            NonNull::new_unchecked(ptr)
        };

        Self { arr, cap: capacity }
    }

    pub unsafe fn dealloc(&mut self) {
        let alloc_size = self.cap * size_of::<T>();
        let layout = Layout::from_size_align_unchecked(alloc_size, align_of::<T>());
        let ptr = self.arr.as_ptr();
        std::alloc::dealloc(ptr as *mut u8, layout);
        self.cap = 0;
    }

    pub unsafe fn offset(&self, index: usize)->*mut T{
        self.arr.as_ptr().offset(index as isize)
    }
}
