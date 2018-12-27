//! This module contains Rust definitions whose C equivalents live in
//! lisp.h.

use libc::{c_char, c_void, intptr_t, uintptr_t};
use std::ffi::CString;

use std::convert::From;
use std::fmt::{Debug, Error, Formatter};
use std::mem;
use std::ops::{Deref, DerefMut};
use std::slice;

use crate::{
    buffers::LispBufferRef,
    eval::FUNCTIONP,
    lists::{list, CarIter, LispConsCircularChecks, LispConsEndChecks},
    process::LispProcessRef,
    remacs_sys::{build_string, internal_equal, make_float},
    remacs_sys::{
        equal_kind, pvec_type, EmacsDouble, EmacsInt, EmacsUint, Lisp_Bits, USE_LSB_TAG, VALMASK,
    },
    remacs_sys::{Lisp_Misc_Any, Lisp_Misc_Type, Lisp_Subr, Lisp_Type},
    remacs_sys::{Qautoload, Qnil, Qsubrp, Qt, Vbuffer_alist, Vprocess_alist},
};

// TODO: tweak Makefile to rebuild C files if this changes.

/// Emacs values are represented as tagged pointers. A few bits are
/// used to represent the type, and the remaining bits are either used
/// to store the value directly (e.g. integers) or the address of a
/// more complex data type (e.g. a cons cell).
///
/// TODO: example representations
///
/// `EmacsInt` represents an integer big enough to hold our tagged
/// pointer representation.
///
/// In Emacs C, this is `EMACS_INT`.
///
/// `EmacsUint` represents the unsigned equivalent of `EmacsInt`.
/// In Emacs C, this is `EMACS_UINT`.
///
/// Their definition are determined in a way consistent with Emacs C.
/// Under casual systems, they're the type isize and usize respectively.
// #[repr(transparent)]
// #[derive(PartialEq, Eq, Clone, Copy)]
// pub struct LispObject(pub EmacsInt);

pub type LispObject = crate::remacs_sys::LispObject;

// ExternalPtr

#[repr(transparent)]
#[derive(Debug)]
pub struct ExternalPtr<T>(*mut T);

impl<T> Copy for ExternalPtr<T> {}

// Derive fails for this type so do it manually
impl<T> Clone for ExternalPtr<T> {
    fn clone(&self) -> Self {
        ExternalPtr::new(self.0)
    }
}

impl<T> ExternalPtr<T> {
    pub fn new(p: *mut T) -> Self {
        ExternalPtr(p)
    }

    pub fn is_null(self) -> bool {
        self.0.is_null()
    }

    pub fn as_ptr(self) -> *const T {
        self.0
    }

    pub fn as_mut(&mut self) -> *mut T {
        self.0
    }

    pub fn from_ptr(ptr: *mut c_void) -> Option<Self> {
        unsafe { ptr.as_ref().map(|p| mem::transmute(p)) }
    }
}

impl<T> Deref for ExternalPtr<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<T> DerefMut for ExternalPtr<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

impl<T> PartialEq for ExternalPtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr()
    }
}

// Misc support (LispType == Lisp_Misc == 1)

// Lisp_Misc is a union. Now we don't really care about its variants except the
// super type layout. LispMisc is an unsized type for this, and LispMiscAny is
// only the header and a padding, which is consistent with the c version.
// directly creating and moving or copying this struct is simply wrong!
// If needed, we can calculate all variants size and allocate properly.

pub type LispMiscRef = ExternalPtr<Lisp_Misc_Any>;

impl LispMiscRef {
    pub fn get_type(self) -> Lisp_Misc_Type {
        self.type_()
    }

    pub fn equal(
        self,
        other: LispMiscRef,
        kind: equal_kind::Type,
        depth: i32,
        ht: LispObject,
    ) -> bool {
        if self.get_type() != other.get_type() {
            false
        } else if let (Some(ov1), Some(ov2)) = (self.as_overlay(), other.as_overlay()) {
            ov1.equal(ov2, kind, depth, ht)
        } else if let (Some(marker1), Some(marker2)) = (self.as_marker(), other.as_marker()) {
            marker1.equal(marker2, kind, depth, ht)
        } else {
            false
        }
    }
}

// Lisp_Subr support

pub type LispSubrRef = ExternalPtr<Lisp_Subr>;
unsafe impl Sync for LispSubrRef {}

impl LispSubrRef {
    pub fn is_many(self) -> bool {
        !self.0.is_null() && self.max_args() == -2
    }

    pub fn is_unevalled(self) -> bool {
        !self.0.is_null() && self.max_args() == -1
    }

    pub fn max_args(self) -> i16 {
        unsafe { (*self.0).max_args }
    }

    pub fn min_args(self) -> i16 {
        unsafe { (*self.0).min_args }
    }

    pub fn symbol_name(self) -> *const c_char {
        unsafe { (*self.0).symbol_name }
    }
}

impl From<LispObject> for LispSubrRef {
    fn from(o: LispObject) -> Self {
        o.as_subr_or_error()
    }
}

/// From `FOR_EACH_ALIST_VALUE` in `lisp.h`
/// Implement `Iterator` over all values of `$data` yielding `$iter_item` type.
/// `$data` should be an `alist` and `$iter_item` type should implement `From<LispObject>`
macro_rules! impl_alistval_iter {
    ($iter_name:ident, $iter_item:ty, $data: expr) => {
        pub struct $iter_name(CarIter);

        impl $iter_name {
            pub fn new() -> Self {
                $iter_name($data.iter_cars(LispConsEndChecks::on, LispConsCircularChecks::on))
            }
        }

        impl Iterator for $iter_name {
            type Item = $iter_item;

            fn next(&mut self) -> Option<Self::Item> {
                self.0
                    .next()
                    .and_then(|o| o.as_cons())
                    .map(|p| p.cdr())
                    .and_then(|q| q.into())
            }
        }
    };
}

impl_alistval_iter! {LiveBufferIter, LispBufferRef, unsafe { Vbuffer_alist }}
impl_alistval_iter! {ProcessIter, LispProcessRef, unsafe { Vprocess_alist }}

pub fn is_autoload(function: LispObject) -> bool {
    function
        .as_cons()
        .map_or(false, |cell| cell.car().eq(Qautoload))
}

impl LispObject {
    pub fn is_nil(self) -> bool {
        self == Qnil
    }

    pub fn is_not_nil(self) -> bool {
        self != Qnil
    }

    pub fn is_t(self) -> bool {
        self == Qt
    }

    // The three Emacs Lisp comparison functions.

    pub fn eq(self, other: LispObject) -> bool {
        self == other
    }

    pub fn eql(self, other: LispObject) -> bool {
        if self.is_float() {
            self.equal_no_quit(other)
        } else {
            self.eq(other)
        }
    }

    pub fn equal(self, other: LispObject) -> bool {
        unsafe { internal_equal(self, other, equal_kind::EQUAL_PLAIN, 0, Qnil) }
    }

    pub fn equal_no_quit(self, other: LispObject) -> bool {
        unsafe { internal_equal(self, other, equal_kind::EQUAL_NO_QUIT, 0, Qnil) }
    }

    pub fn is_function(self) -> bool {
        FUNCTIONP(self)
    }

    pub fn map_or<T, F: FnOnce(LispObject) -> T>(self, default: T, action: F) -> T {
        if self.is_nil() {
            default
        } else {
            action(self)
        }
    }

    pub fn map_or_else<T, F: FnOnce() -> T, F1: FnOnce(LispObject) -> T>(
        self,
        default: F,
        action: F1,
    ) -> T {
        if self.is_nil() {
            default()
        } else {
            action(self)
        }
    }
}

/// Used to denote functions that have no limit on the maximum number
/// of arguments.
pub const MANY: i16 = -2;

/// Internal function to get a displayable string out of a Lisp string.
fn display_string(obj: LispObject) -> String {
    let s = obj.as_string().unwrap();
    let slice = unsafe { slice::from_raw_parts(s.const_data_ptr(), s.len_bytes() as usize) };
    String::from_utf8_lossy(slice).into_owned()
}

impl Debug for LispObject {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        let ty = self.get_type();
        let self_ptr = &self as *const _ as usize;
        if ty as u8 >= 8 {
            write!(
                f,
                "#<INVALID-OBJECT @ {:#X}: VAL({:#X})>",
                self_ptr,
                self.to_C()
            )?;
            return Ok(());
        }
        if self.is_nil() {
            return write!(f, "nil");
        }
        match ty {
            Lisp_Type::Lisp_Symbol => {
                let name = self.as_symbol_or_error().symbol_name();
                write!(f, "'{}", display_string(name))?;
            }
            Lisp_Type::Lisp_Cons => {
                let mut cdr = *self;
                write!(f, "'(")?;
                while let Some(cons) = cdr.as_cons() {
                    write!(f, "{:?} ", cons.car())?;
                    cdr = cons.cdr();
                }
                if cdr.is_nil() {
                    write!(f, ")")?;
                } else {
                    write!(f, ". {:?}", cdr)?;
                }
            }
            Lisp_Type::Lisp_Float => {
                write!(f, "{}", self.as_float().unwrap())?;
            }
            Lisp_Type::Lisp_Vectorlike => {
                let vl = self.as_vectorlike().unwrap();
                if vl.is_vector() {
                    write!(f, "[")?;
                    for el in vl.as_vector().unwrap().as_slice() {
                        write!(f, "{:?} ", el)?;
                    }
                    write!(f, "]")?;
                } else {
                    write!(
                        f,
                        "#<VECTOR-LIKE @ {:#X}: VAL({:#X})>",
                        self_ptr,
                        self.to_C()
                    )?;
                }
            }
            Lisp_Type::Lisp_Int0 | Lisp_Type::Lisp_Int1 => {
                write!(f, "{}", self.as_fixnum().unwrap())?;
            }
            Lisp_Type::Lisp_Misc => {
                write!(f, "#<MISC @ {:#X}: VAL({:#X})>", self_ptr, self.to_C())?;
            }
            Lisp_Type::Lisp_String => {
                write!(f, "{:?}", display_string(*self))?;
            }
        }
        Ok(())
    }
}

extern "C" {
    pub fn defsubr(sname: *const Lisp_Subr);
}

macro_rules! export_lisp_fns {
    ($($f:ident),+) => {
        pub fn rust_init_syms() {
            unsafe {
                $(
                    defsubr(concat_idents!(S, $f).as_ptr());
                )+
            }
        }
    }
}

#[allow(unused_macros)]
macro_rules! protect_statics_from_GC {
    ($($f:ident),+) => {
        pub fn rust_static_syms() {
            unsafe {
                $(
                    crate::remacs_sys::staticpro(&$f as *const LispObject as *mut LispObject);
                )+
            }
        }
    }
}

#[test]
fn test_lisp_misc_any_size() {
    // Should be 32 bits, which is 4 bytes.
    assert!(mem::size_of::<Lisp_Misc_Any>() == 4);
}
