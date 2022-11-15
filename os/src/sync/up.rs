use core::cell::{RefCell, RefMut, UnsafeCell};
use core::ops::{Deref, DerefMut};
use lazy_static::*;
use riscv::register::sstatus;

/*
/// Wrap a static data structure inside it so that we are
/// able to access it without any `unsafe`.
///
/// We should only use it in uniprocessor.
///
/// In order to get mutable reference of inner data, call
/// `exclusive_access`.
pub struct UPSafeCell<T> {
    /// inner data
    inner: RefCell<T>,
}

unsafe impl<T> Sync for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    /// User is responsible to guarantee that inner struct is only used in
    /// uniprocessor.
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }
    /// Panic if the data has been borrowed.
    pub fn exclusive_access(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }
}
*/

/// UnsafeCell wrapper with `Sync`
pub struct UPSafeCellRaw<T> {
    inner: UnsafeCell<T>,
}

unsafe impl<T> Sync for UPSafeCellRaw<T> {}

impl<T> UPSafeCellRaw<T> {
    /// Constructs a new instance of `UnsafeCellRaw` which will wrap the specified
    /// value.
    ///
    /// All access to the inner value through methods is `unsafe`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sync::UnsafeCellRaw;
    ///
    /// let uc = UnsafeCellRaw::new(5);
    /// ```
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: UnsafeCell::new(value),
        }
    }

    #[allow(clippy::mut_from_ref)]
    /// Gets a mutable pointer to the wrapped value.
    ///
    /// This can be cast to a pointer of any kind.
    /// Ensure that the access is unique (no active references, mutable or not)
    /// when casting to `&mut T`, and ensure that there are no mutations
    /// or mutable aliases going on when casting to `&T`
    pub fn get_mut(&self) -> &mut T {
        unsafe { &mut (*self.inner.get()) }
    }
}

/// Used to enable/disable interrupts by setting the `sie` bit to 0/1 depending on the number of total
/// exclusive accesses during OS operation.
pub struct IntrMaskingInfo {
    /// exclusive access count
    nested_level: usize,
    sie_before_masking: bool,
}

lazy_static! {
    /// Used to enable/disable interrupts by setting the `sie` bit to 0/1 depending on the number of total
    /// exclusive accesses during OS operation.
    static ref INTR_MASKING_INFO: UPSafeCellRaw<IntrMaskingInfo> =
        unsafe { UPSafeCellRaw::new(IntrMaskingInfo::new()) };
}

impl IntrMaskingInfo {
    /// Create IntrMaskingInfo with all 0 fields.
    pub fn new() -> Self {
        Self {
            nested_level: 0,
            sie_before_masking: false,
        }
    }

    /// Increment nested level.
    ///
    /// Clear supervisor interrupt enable bit(sie).
    ///
    /// Store supervisor interrupt enable bit(sie) if nested level is 0.
    pub fn enter(&mut self) {
        let sie = sstatus::read().sie();
        unsafe {
            sstatus::clear_sie();
        }
        if self.nested_level == 0 {
            self.sie_before_masking = sie;
        }
        self.nested_level += 1;
    }

    /// Decrement nested_level.
    ///
    /// Set supervisor interrupt enable bit if nested_level is 0 and sie_before_masking is true.
    pub fn exit(&mut self) {
        self.nested_level -= 1;
        if self.nested_level == 0 && self.sie_before_masking {
            unsafe {
                sstatus::set_sie();
            }
        }
    }
}

/// `RefCell` wrapper with `Sync` to disable supervisor interrupts during exclusive access
pub struct UPIntrFreeCell<T> {
    /// inner data
    inner: RefCell<T>,
}

unsafe impl<T> Sync for UPIntrFreeCell<T> {}

/// `RefMut` wrapper with `Sync` to disable supervisor interrupts during exclusive access
///
/// - During exclusive access, the Supervisor interrupt bit is set to 0 to prevent interrupts.
/// - At the end of exclusive access (drop time), the interrupt bit is set to 1 again to enable interrupts.
///
/// ```rust
/// pub struct UPIntrRefMut<'a, T>(Option<RefMut<'a, T>>);
/// ```
pub struct UPIntrRefMut<'a, T>(Option<RefMut<'a, T>>);

impl<T> UPIntrFreeCell<T> {
    /// Creates a new `RefCell` containing `value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sync::UPIntrFreeCell;
    ///
    /// let c = UPIntrFreeCell::new(5);
    /// ```
    pub unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }

    /// Mutably borrows the wrapped value.
    ///
    /// Increment nested level.
    /// - Clear supervisor interrupt enable bit(sie).
    /// - Store supervisor interrupt enable bit(sie) if nested level is 0.
    ///
    /// The borrow lasts until the returned `RefMut` or all `RefMut`s derived
    /// from it exit scope. The value cannot be borrowed while this borrow is
    /// active.
    ///
    /// # Panics
    ///
    /// Panics if the value is currently borrowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use sync::UPIntrFreeCell;
    ///
    /// let c = UPInterFreeCell::new("hello".to_owned());
    ///
    /// *c.exclusive_access() = "bonjour".to_owned();
    ///
    /// assert_eq!(&*c.borrow(), "bonjour");
    /// ```
    ///
    /// An example of panic:
    ///
    /// ```should_panic
    /// use sync::UPIntrFreeCell;
    ///
    /// let c = UPInterFreeCell::new(5);
    /// let m = c.exclusive_access();
    ///
    /// let b = c.exclusive_access(); // this causes a panic
    /// ```
    pub fn exclusive_access(&self) -> UPIntrRefMut<'_, T> {
        INTR_MASKING_INFO.get_mut().enter();
        UPIntrRefMut(Some(self.inner.borrow_mut()))
    }

    /// Temporary exclusive access through callback functions
    ///
    /// - `f`: Function to affect exclusive access to a resource
    pub fn exclusive_session<F, V>(&self, f: F) -> V
    where
        F: FnOnce(&mut T) -> V,
    {
        let mut inner = self.exclusive_access();
        f(inner.deref_mut())
    }
}

impl<'a, T> Drop for UPIntrRefMut<'a, T> {
    fn drop(&mut self) {
        self.0 = None;
        INTR_MASKING_INFO.get_mut().exit();
    }
}

impl<'a, T> Deref for UPIntrRefMut<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().unwrap().deref()
    }
}
impl<'a, T> DerefMut for UPIntrRefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().unwrap().deref_mut()
    }
}
