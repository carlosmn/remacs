use libc::{c_char, c_void, intptr_t, uintptr_t};
use std::ffi::CString;
use std::mem;

use crate::{build_string, Lisp_Bits, Lisp_Type, USE_LSB_TAG};
use crate::{EmacsInt, EmacsUint};

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
#[repr(transparent)]
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct LispObject(pub EmacsInt);

impl LispObject {
    pub fn from_C(n: EmacsInt) -> Self {
        LispObject(n)
    }

    pub fn from_C_unsigned(n: EmacsUint) -> Self {
        Self::from_C(n as EmacsInt)
    }

    pub fn to_C(self) -> EmacsInt {
        self.0
    }

    pub fn to_C_unsigned(self) -> EmacsUint {
        self.0 as EmacsUint
    }

    pub fn from_bool(v: bool) -> Self {
        if v {
            Qt
        } else {
            Qnil
        }
    }

    pub fn from_float(v: EmacsDouble) -> Self {
        unsafe { make_float(v) }
    }
}

impl<T> From<Option<T>> for LispObject
where
    LispObject: From<T>,
{
    fn from(v: Option<T>) -> Self {
        match v {
            None => Qnil,
            Some(v) => LispObject::from(v),
        }
    }
}

impl LispObject {
    pub fn is_misc(self) -> bool {
        self.get_type() == Lisp_Type::Lisp_Misc
    }

    pub fn as_misc(self) -> Option<LispMiscRef> {
        if self.is_misc() {
            unsafe { Some(self.to_misc_unchecked()) }
        } else {
            None
        }
    }

    unsafe fn to_misc_unchecked(self) -> LispMiscRef {
        LispMiscRef::new(self.get_untaggedptr() as *mut Lisp_Misc_Any)
    }
}

impl LispObject {
    pub fn is_subr(self) -> bool {
        self.as_vectorlike()
            .map_or(false, |v| v.is_pseudovector(pvec_type::PVEC_SUBR))
    }

    pub fn as_subr(self) -> Option<LispSubrRef> {
        self.as_vectorlike().and_then(|v| v.as_subr())
    }

    pub fn as_subr_or_error(self) -> LispSubrRef {
        self.as_subr().unwrap_or_else(|| wrong_type!(Qsubrp, self))
    }
}

// Other functions

impl From<()> for LispObject {
    fn from(_v: ()) -> Self {
        Qnil
    }
}

impl From<Vec<LispObject>> for LispObject {
    fn from(v: Vec<LispObject>) -> Self {
        list(&v)
    }
}

impl<T> From<Vec<T>> for LispObject
where
    LispObject: From<T>,
{
    default fn from(v: Vec<T>) -> LispObject {
        list(
            &v.into_iter()
                .map(LispObject::from)
                .collect::<Vec<LispObject>>(),
        )
    }
}

impl From<LispObject> for bool {
    fn from(o: LispObject) -> Self {
        o.is_not_nil()
    }
}

impl From<bool> for LispObject {
    fn from(v: bool) -> Self {
        if v {
            Qt
        } else {
            Qnil
        }
    }
}

impl From<LispObject> for u32 {
    fn from(o: LispObject) -> Self {
        o.as_fixnum_or_error() as u32
    }
}

impl From<LispObject> for Option<u32> {
    fn from(o: LispObject) -> Self {
        match o.as_fixnum() {
            None => None,
            Some(n) => Some(n as u32),
        }
    }
}

impl From<!> for LispObject {
    fn from(_v: !) -> Self {
        // I'm surprized that this works
        Qnil
    }
}

/// Copies a Rust str into a new Lisp string
impl<'a> From<&'a str> for LispObject {
    fn from(s: &str) -> Self {
        let cs = CString::new(s).unwrap();
        unsafe { build_string(cs.as_ptr()) }
    }
}

impl LispObject {
    pub fn get_type(self) -> Lisp_Type {
        let raw = self.to_C_unsigned();
        let res = (if USE_LSB_TAG {
            raw & (!VALMASK as EmacsUint)
        } else {
            raw >> Lisp_Bits::VALBITS
        }) as u32;
        unsafe { mem::transmute(res) }
    }

    pub fn tag_ptr<T>(external: ExternalPtr<T>, ty: Lisp_Type) -> LispObject {
        let raw = external.as_ptr() as intptr_t;
        let res = if USE_LSB_TAG {
            let ptr = raw as intptr_t;
            let tag = ty as intptr_t;
            (ptr + tag) as EmacsInt
        } else {
            let ptr = raw as EmacsUint as uintptr_t;
            let tag = ty as EmacsUint as uintptr_t;
            ((tag << Lisp_Bits::VALBITS) + ptr) as EmacsInt
        };

        LispObject::from_C(res)
    }

    pub fn get_untaggedptr(self) -> *mut c_void {
        (self.to_C() & VALMASK) as intptr_t as *mut c_void
    }
}

impl From<LispObject> for EmacsInt {
    fn from(o: LispObject) -> Self {
        o.as_fixnum_or_error()
    }
}

impl From<LispObject> for Option<EmacsInt> {
    fn from(o: LispObject) -> Self {
        if o.is_nil() {
            None
        } else {
            Some(o.as_fixnum_or_error())
        }
    }
}

impl From<LispObject> for EmacsUint {
    fn from(o: LispObject) -> Self {
        o.as_natnum_or_error()
    }
}

impl From<LispObject> for Option<EmacsUint> {
    fn from(o: LispObject) -> Self {
        if o.is_nil() {
            None
        } else {
            Some(o.as_natnum_or_error())
        }
    }
}

impl From<EmacsInt> for LispObject {
    fn from(v: EmacsInt) -> Self {
        LispObject::from_fixnum(v)
    }
}

impl From<isize> for LispObject {
    fn from(v: isize) -> Self {
        LispObject::from_fixnum(v as EmacsInt)
    }
}

impl From<i32> for LispObject {
    fn from(v: i32) -> Self {
        LispObject::from_fixnum(EmacsInt::from(v))
    }
}

impl From<i16> for LispObject {
    fn from(v: i16) -> Self {
        LispObject::from_fixnum(EmacsInt::from(v))
    }
}

impl From<i8> for LispObject {
    fn from(v: i8) -> Self {
        LispObject::from_fixnum(EmacsInt::from(v))
    }
}

impl From<EmacsUint> for LispObject {
    fn from(v: EmacsUint) -> Self {
        LispObject::from_natnum(v)
    }
}

impl From<usize> for LispObject {
    fn from(v: usize) -> Self {
        LispObject::from_natnum(v as EmacsUint)
    }
}

impl From<u32> for LispObject {
    fn from(v: u32) -> Self {
        LispObject::from_natnum(EmacsUint::from(v))
    }
}

impl From<u16> for LispObject {
    fn from(v: u16) -> Self {
        LispObject::from_natnum(EmacsUint::from(v))
    }
}

impl From<u8> for LispObject {
    fn from(v: u8) -> Self {
        LispObject::from_natnum(EmacsUint::from(v))
    }
}

impl LispObject {
    pub fn is_mutex(self) -> bool {
        self.as_vectorlike()
            .map_or(false, |v| v.is_pseudovector(pvec_type::PVEC_MUTEX))
    }

    pub fn is_condition_variable(self) -> bool {
        self.as_vectorlike()
            .map_or(false, |v| v.is_pseudovector(pvec_type::PVEC_CONDVAR))
    }

    pub fn is_byte_code_function(self) -> bool {
        self.as_vectorlike()
            .map_or(false, |v| v.is_pseudovector(pvec_type::PVEC_COMPILED))
    }

    pub fn is_module_function(self) -> bool {
        self.as_vectorlike().map_or(false, |v| {
            v.is_pseudovector(pvec_type::PVEC_MODULE_FUNCTION)
        })
    }

    pub fn is_array(self) -> bool {
        self.is_vector() || self.is_string() || self.is_char_table() || self.is_bool_vector()
    }

    pub fn is_sequence(self) -> bool {
        self.is_cons() || self.is_nil() || self.is_array()
    }

    pub fn is_record(self) -> bool {
        self.as_vectorlike()
            .map_or(false, |v| v.is_pseudovector(pvec_type::PVEC_RECORD))
    }
}
